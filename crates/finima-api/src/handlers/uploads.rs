use std::collections::HashMap;

use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use finima_auth::middleware::AuthUser;
use finima_core::services::sign_normalizer::SignNormalizer;
use finima_core::traits::{AccountRepo, PortfolioRepo};
use finima_core::types::UploadStatus;
use finima_core::{AppError, FileFormat};
use finima_db::NewTransaction;
use finima_ingest::{
    compute_dedup_hash, detect_format, generate_preview, normalize_batch, ColumnMapping, FileParser,
};

use crate::state::AppState;
use crate::ws::WsMessage;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub id: Uuid,
    pub preview: serde_json::Value,
    pub format: FileFormat,
}

#[derive(Debug, Serialize)]
pub struct ConfirmResponse {
    pub imported: usize,
    pub duplicates: usize,
    pub total: usize,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ConfirmRequest {
    /// Header-name → target-name mapping from the frontend (e.g. `{"Date": "Date", "Debit": "Debit"}`).
    pub mapping: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub skip_duplicates: bool,
    pub date_format: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct UploadStatusResponse {
    pub id: Uuid,
    pub status: UploadStatus,
    pub row_count: i32,
    pub imported_count: i32,
    pub duplicate_count: i32,
    pub error_message: Option<String>,
    /// Number of transactions categorized so far (only set during `categorizing` status).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categorized_count: Option<usize>,
    /// Total transactions to categorize (only set during `categorizing` status).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categorized_total: Option<usize>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create the correct parser for a given format.
fn parser_for_format(format: FileFormat) -> Box<dyn FileParser> {
    match format {
        FileFormat::Csv => Box::new(finima_ingest::CsvParser::new(b',')),
        FileFormat::Tsv => Box::new(finima_ingest::CsvParser::new(b'\t')),
        FileFormat::Ofx | FileFormat::Qfx | FileFormat::Qbo => {
            Box::new(finima_ingest::OfxParser::new())
        }
        FileFormat::Qif => Box::new(finima_ingest::QifParser::new()),
        FileFormat::Xls | FileFormat::Xlsx => Box::new(finima_ingest::XlsxParser::default()),
    }
}

/// Convert a name-based mapping from the frontend into an index-based [`ColumnMapping`].
///
/// The frontend sends `{ "Date": "Date", "Description": "Description", "Debit": "Debit", ... }`
/// where keys are file column headers and values are target field names.
fn resolve_mapping(
    mapping: &HashMap<String, String>,
    headers: &[String],
) -> Result<ColumnMapping, AppError> {
    let mut date_col = None;
    let mut amount_col = None;
    let mut debit_col = None;
    let mut credit_col = None;
    let mut description_col = None;
    let mut memo_col = None;
    let mut category_col = None;

    for (header_name, target) in mapping {
        if target == "-- Skip --" {
            continue;
        }
        let idx = headers
            .iter()
            .position(|h| h == header_name)
            .ok_or_else(|| {
                AppError::BadRequest(format!("Header '{}' not found in file", header_name))
            })?;
        match target.as_str() {
            "Date" => date_col = Some(idx),
            "Amount" => amount_col = Some(idx),
            "Debit" => debit_col = Some(idx),
            "Credit" => credit_col = Some(idx),
            "Description" => description_col = Some(idx),
            "Memo" => memo_col = Some(idx),
            "Category" => category_col = Some(idx),
            _ => {}
        }
    }

    let date_col =
        date_col.ok_or_else(|| AppError::BadRequest("Date column is required".into()))?;
    let description_col = description_col
        .ok_or_else(|| AppError::BadRequest("Description column is required".into()))?;

    let cm = ColumnMapping {
        date_col,
        amount_col,
        debit_col,
        credit_col,
        description_col,
        memo_col,
        category_col,
    };
    cm.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    Ok(cm)
}

/// Convert an index-based [`ColumnMapping`] to a header-name → target-name map
/// for the frontend preview response.
fn inferred_mapping_to_names(
    mapping: &ColumnMapping,
    headers: &[String],
) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    let set = |m: &mut serde_json::Map<String, serde_json::Value>,
               idx: usize,
               target: &str,
               headers: &[String]| {
        if let Some(h) = headers.get(idx) {
            m.insert(h.clone(), serde_json::Value::String(target.into()));
        }
    };

    set(&mut map, mapping.date_col, "Date", headers);
    if let Some(idx) = mapping.amount_col {
        set(&mut map, idx, "Amount", headers);
    }
    if let Some(idx) = mapping.debit_col {
        set(&mut map, idx, "Debit", headers);
    }
    if let Some(idx) = mapping.credit_col {
        set(&mut map, idx, "Credit", headers);
    }
    set(&mut map, mapping.description_col, "Description", headers);
    if let Some(idx) = mapping.memo_col {
        set(&mut map, idx, "Memo", headers);
    }
    if let Some(idx) = mapping.category_col {
        set(&mut map, idx, "Category", headers);
    }

    // Fill unmapped headers with "-- Skip --"
    for h in headers {
        if !map.contains_key(h) {
            map.insert(h.clone(), serde_json::Value::String("-- Skip --".into()));
        }
    }

    map
}

/// Resolve the user_id that owns a given account, for WebSocket message routing.
async fn user_id_for_account(state: &AppState, account_id: Uuid) -> Option<Uuid> {
    let account: finima_core::models::Account =
        state.account_repo().find_by_id(account_id).await.ok()?;
    let portfolio: finima_core::models::Portfolio = state
        .portfolio_repo()
        .find_by_id(account.portfolio_id)
        .await
        .ok()?;
    Some(portfolio.user_id)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/uploads
///
/// Multipart form: `file` + `account_id`.
/// Detects file format, parses a preview, creates an Upload record.
pub async fn create_upload(
    user: AuthUser,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut account_id: Option<Uuid> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Failed to read file: {}", e)))?;
                file_data = Some(bytes.to_vec());
            }
            "account_id" => {
                let text = field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read account_id: {}", e))
                })?;
                account_id =
                    Some(Uuid::parse_str(&text).map_err(|_| {
                        AppError::BadRequest("Invalid account_id UUID".to_string())
                    })?);
            }
            _ => {}
        }
    }

    let file_data = file_data.ok_or(AppError::BadRequest("Missing file field".to_string()))?;
    let filename = filename.unwrap_or_else(|| "unknown".to_string());
    let account_id =
        account_id.ok_or(AppError::BadRequest("Missing account_id field".to_string()))?;

    // Verify ownership: account -> portfolio -> user
    let account = state.account_repo().find_by_id(account_id).await?;
    state
        .portfolio_repo()
        .verify_ownership(account.portfolio_id, user.user_id)
        .await?;

    // Detect format
    let first_bytes = &file_data[..file_data.len().min(4096)];
    let format = detect_format(&filename, first_bytes)
        .map_err(|e| AppError::BadRequest(format!("Cannot detect file format: {}", e)))?;

    // Generate preview
    let preview = generate_preview(&file_data, format)
        .map_err(|e| AppError::ParseError(format!("Preview generation failed: {}", e)))?;

    // Create upload record
    let upload = state
        .upload_repo()
        .create(account_id, &filename, format)
        .await?;

    // Store the file in S3-compatible object storage.
    let s3_key = format!("uploads/{}/{}/{}", user.user_id, upload.id, filename);

    let content_type = match format {
        FileFormat::Csv | FileFormat::Tsv => "text/csv",
        FileFormat::Ofx | FileFormat::Qfx | FileFormat::Qbo => "application/xml",
        FileFormat::Qif => "application/qif",
        FileFormat::Xls | FileFormat::Xlsx => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        }
    };

    state
        .object_storage()
        .put_object(&s3_key, file_data, content_type)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to store file in S3: {}", e)))?;

    // Convert the index-based inferred_mapping to name-based for the frontend,
    // then store both the S3 key and the frontend-friendly preview in the database.
    let mut preview_json = serde_json::to_value(&preview).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = preview_json.as_object_mut() {
        let name_mapping = inferred_mapping_to_names(&preview.inferred_mapping, &preview.headers);
        obj.insert(
            "inferred_mapping".into(),
            serde_json::Value::Object(name_mapping),
        );
    }

    let storage = serde_json::json!({
        "s3_key": s3_key,
        "preview": preview_json,
    });
    state
        .upload_repo()
        .update_column_mapping(upload.id, storage)
        .await?;

    // Notify the owning user via WebSocket that a new upload has been received.
    if let Some(owner_id) = user_id_for_account(&state, account_id).await {
        state
            .ws_manager()
            .send_to_user(
                owner_id,
                WsMessage::UploadProgress {
                    upload_id: upload.id,
                    parsed: 0,
                    total: 0,
                },
            )
            .await;
    }

    Ok((
        StatusCode::CREATED,
        Json(UploadResponse {
            id: upload.id,
            preview: preview_json,
            format,
        }),
    ))
}

/// GET /api/uploads/:id/preview
///
/// Return the stored preview for an upload.
pub async fn get_preview(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let upload = state.upload_repo().find_by_id(id).await?;

    // Verify ownership: upload -> account -> portfolio -> user
    let account = state.account_repo().find_by_id(upload.account_id).await?;
    state
        .portfolio_repo()
        .verify_ownership(account.portfolio_id, user.user_id)
        .await?;

    let preview = upload
        .column_mapping
        .as_ref()
        .and_then(|v| v.get("preview"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    Ok(Json(preview))
}

/// POST /api/uploads/:id/confirm
///
/// Accept optional column_mapping. Parse all rows, compute dedup hashes,
/// bulk insert, update upload status, spawn async categorization.
pub async fn confirm_upload(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ConfirmRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Only block uploads while the LLM is actively loading. If the LLM is
    // disabled or failed, allow the upload — Tiers 0-2 handle categorization.
    if state.llm_status() == "loading" {
        return Err(AppError::ServiceUnavailable(
            "LLM backend is still loading — please try again shortly".to_string(),
        ));
    }

    let upload = state.upload_repo().find_by_id(id).await?;

    // Verify ownership: upload -> account -> portfolio -> user
    let account = state.account_repo().find_by_id(upload.account_id).await?;
    state
        .portfolio_repo()
        .verify_ownership(account.portfolio_id, user.user_id)
        .await?;

    if upload.status != UploadStatus::Pending {
        return Err(AppError::Conflict(format!(
            "Upload is already in status: {}",
            upload.status
        )));
    }

    // Update status to processing
    state
        .upload_repo()
        .update_status(upload.id, UploadStatus::Processing, None, None, None, None)
        .await?;

    // Retrieve the S3 key and fetch the file from object storage.
    let stored = upload
        .column_mapping
        .as_ref()
        .ok_or(AppError::InternalError(
            "No stored file metadata".to_string(),
        ))?;

    let s3_key = stored
        .get("s3_key")
        .and_then(|v| v.as_str())
        .ok_or(AppError::InternalError(
            "Missing S3 key in upload metadata".to_string(),
        ))?;

    let file_data = state
        .object_storage()
        .get_object(s3_key)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to retrieve file from S3: {}", e)))?;

    // Resolve the name-based mapping from the frontend into an index-based ColumnMapping.
    // For auto-mapped formats (OFX, QFX, QBO, QIF) the mapping is ignored by the parser,
    // but we still resolve it to keep the API contract consistent.
    let column_mapping = if !body.mapping.is_empty() {
        // Extract headers from stored preview to resolve name→index mapping.
        let headers: Vec<String> = stored
            .get("preview")
            .and_then(|v| v.get("headers"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .ok_or_else(|| AppError::InternalError("No headers found in stored preview".into()))?;
        Some(resolve_mapping(&body.mapping, &headers)?)
    } else {
        None
    };

    // Parse all rows
    let parser = parser_for_format(upload.format);
    let raw_transactions = parser
        .parse_all(&file_data, column_mapping.as_ref())
        .map_err(|e| AppError::ParseError(format!("Parse error: {}", e)))?;

    let total = raw_transactions.len();

    // Normalize per-row direction (inflow/outflow) using the configured
    // SignNormalizer. See ADR-018. The result includes any autodetection
    // outcome we can surface back to the user as a post-import banner.
    //
    // Account-level overrides (set via the UI Flip-this-account button)
    // take precedence over institution YAML rules. We merge the
    // override (if any) into the normalizer's by_account_id map.
    let mut rules = state.config().sign_conventions.clone().into_service_rules();
    if let Some(override_convention) = account.sign_convention_override {
        rules.by_account_id.insert(account.id, override_convention);
    }
    let normalizer = SignNormalizer::new(rules);
    let normalization = normalize_batch(
        &raw_transactions,
        upload.account_id,
        account.account_type,
        account.institution.as_deref(),
        &normalizer,
    );
    if let Some(detection) = &normalization.autodetection {
        tracing::info!(
            account_id = %upload.account_id,
            account_type = %account.account_type,
            verdict = ?detection.verdict,
            confidence = detection.confidence,
            reason = %detection.reason,
            "sign-convention autodetection completed"
        );
    }

    // Compute dedup hashes and build NewTransaction records.
    //
    // Note on the dedup hash: it is computed from the *raw* amount
    // (as it appeared in the source file) so re-uploading the same
    // file under a different institution rule still deduplicates
    // against previously imported rows. The *canonical* amount is
    // what we persist to `transactions.amount`. See ADR-018.
    let new_transactions: Vec<NewTransaction> = raw_transactions
        .iter()
        .zip(normalization.directions.iter())
        .zip(normalization.amounts.iter())
        .map(|((raw, &direction), &canonical_amount)| {
            let hash = compute_dedup_hash(&raw.date, &raw.amount, &raw.original_description);
            NewTransaction {
                account_id: upload.account_id,
                date: raw.date,
                amount: canonical_amount,
                description: raw.description.clone(),
                original_description: raw.original_description.clone(),
                category: raw.category.clone(),
                subcategory: None,
                merchant_name: None,
                memo: raw.memo.clone(),
                dedup_hash: hash,
                direction,
            }
        })
        .collect();

    // Bulk insert
    let imported = state
        .transaction_repo()
        .bulk_insert(upload.account_id, &new_transactions)
        .await?;

    let duplicates = total - imported;

    // Set status to Categorizing so the frontend knows work is still in progress.
    state
        .upload_repo()
        .update_status(
            upload.id,
            UploadStatus::Categorizing,
            Some(total as i32),
            Some(imported as i32),
            Some(duplicates as i32),
            None,
        )
        .await?;

    // Clean up the S3 object now that the file has been fully parsed and imported.
    if let Err(e) = state.object_storage().delete_object(s3_key).await {
        tracing::warn!(
            upload_id = %upload.id,
            s3_key = %s3_key,
            error = %e,
            "Failed to delete S3 object after successful import"
        );
    }

    // Spawn async categorization pipeline — it will set status to Complete when done.
    let categorization_state = state.clone();
    let user_id = user.user_id;
    let upload_id = upload.id;
    let account_id = upload.account_id;

    tokio::spawn(async move {
        if let Err(e) = run_categorization_pipeline(
            categorization_state.clone(),
            user_id,
            upload_id,
            account_id,
        )
        .await
        {
            tracing::error!(
                upload_id = %upload_id,
                error = %e,
                "Async categorization pipeline failed"
            );
            // Still mark upload as complete — the import succeeded, categorization
            // can be retried later via the on-demand endpoint.
            let _ = categorization_state
                .upload_repo()
                .update_status(upload_id, UploadStatus::Complete, None, None, None, None)
                .await;
        }
    });

    Ok(Json(ConfirmResponse {
        imported,
        duplicates,
        total,
    }))
}

/// GET /api/uploads/:id/status
///
/// Return the current status of an upload.
pub async fn get_upload_status(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let upload = state.upload_repo().find_by_id(id).await?;

    // Verify ownership: upload -> account -> portfolio -> user
    let account = state.account_repo().find_by_id(upload.account_id).await?;
    state
        .portfolio_repo()
        .verify_ownership(account.portfolio_id, user.user_id)
        .await?;

    // Include categorization progress if the upload is in the categorizing phase.
    let (categorized_count, categorized_total) = state
        .get_upload_categorization_progress(upload.id)
        .map(|(c, t)| (Some(c), Some(t)))
        .unwrap_or((None, None));

    Ok(Json(UploadStatusResponse {
        id: upload.id,
        status: upload.status,
        row_count: upload.row_count,
        imported_count: upload.imported_count,
        duplicate_count: upload.duplicate_count,
        error_message: upload.error_message,
        categorized_count,
        categorized_total,
    }))
}

// ---------------------------------------------------------------------------
// Async categorization pipeline
// ---------------------------------------------------------------------------

/// Background task that categorizes uncategorized transactions after an upload.
///
/// Delegates to the shared categorization pipeline and wraps it with
/// upload-specific WebSocket progress events.
async fn run_categorization_pipeline(
    state: AppState,
    user_id: Uuid,
    upload_id: Uuid,
    account_id: Uuid,
) -> Result<(), AppError> {
    // Send initial progress (upload-specific: includes upload_id)
    let uncategorized_count = state
        .transaction_repo()
        .find_uncategorized(account_id)
        .await?
        .len();

    if uncategorized_count == 0 {
        tracing::info!(upload_id = %upload_id, "No uncategorized transactions to process");
        state
            .upload_repo()
            .update_status(upload_id, UploadStatus::Complete, None, None, None, None)
            .await?;
        return Ok(());
    }

    // Seed the in-memory progress so the polling endpoint can return it immediately.
    state.set_upload_categorization_progress(upload_id, 0, uncategorized_count);

    state
        .ws_manager()
        .send_to_user(
            user_id,
            WsMessage::CategorizationProgress {
                upload_id,
                categorized: 0,
                total: uncategorized_count,
                flagged: 0,
            },
        )
        .await;

    // Run the shared pipeline with upload_id for per-batch progress events.
    let outcome = super::categorization::run_categorization_for_account_with_upload(
        &state,
        user_id,
        account_id,
        Some(upload_id),
    )
    .await?;

    if let Some(outcome) = outcome {
        // Send upload-specific completion event
        state
            .ws_manager()
            .send_to_user(
                user_id,
                WsMessage::CategorizationComplete {
                    upload_id,
                    total: outcome.total,
                    flagged: outcome.flagged,
                },
            )
            .await;

        tracing::info!(
            upload_id = %upload_id,
            categorized = outcome.total,
            flagged = outcome.flagged,
            "Categorization pipeline complete"
        );
    }

    // Mark the upload as fully complete now that categorization has finished.
    state
        .upload_repo()
        .update_status(upload_id, UploadStatus::Complete, None, None, None, None)
        .await?;

    Ok(())
}
