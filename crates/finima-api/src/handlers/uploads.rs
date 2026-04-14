use std::collections::HashMap;

use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use finima_auth::middleware::AuthUser;
use finima_core::traits::{AccountRepo, PortfolioRepo};
use finima_core::types::UploadStatus;
use finima_core::{AppError, FileFormat};
use finima_db::{LlmCategorizationUpdate, NewTransaction};
use finima_ingest::{
    compute_dedup_hash, detect_format, generate_preview, ColumnMapping, FileParser,
};
use finima_llm::{Categorizer, OverridePattern, TransactionInput};

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
    // Gate: the LLM backend must be loaded before we accept uploads that
    // require categorization. Return 503 so the UI can show "still loading".
    if !state.is_llm_ready() {
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

    // Compute dedup hashes and build NewTransaction records
    let new_transactions: Vec<NewTransaction> = raw_transactions
        .iter()
        .map(|raw| {
            let hash = compute_dedup_hash(&raw.date, &raw.amount, &raw.original_description);
            NewTransaction {
                account_id: upload.account_id,
                date: raw.date,
                amount: raw.amount,
                description: raw.description.clone(),
                original_description: raw.original_description.clone(),
                category: raw.category.clone(),
                subcategory: None,
                merchant_name: None,
                memo: raw.memo.clone(),
                dedup_hash: hash,
            }
        })
        .collect();

    // Bulk insert
    let imported = state
        .transaction_repo()
        .bulk_insert(upload.account_id, &new_transactions)
        .await?;

    let duplicates = total - imported;

    // Update upload status
    state
        .upload_repo()
        .update_status(
            upload.id,
            UploadStatus::Complete,
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

    // Spawn async categorization pipeline
    let categorization_state = state.clone();
    let user_id = user.user_id;
    let upload_id = upload.id;
    let account_id = upload.account_id;

    tokio::spawn(async move {
        if let Err(e) =
            run_categorization_pipeline(categorization_state, user_id, upload_id, account_id).await
        {
            tracing::error!(
                upload_id = %upload_id,
                error = %e,
                "Async categorization pipeline failed"
            );
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

    Ok(Json(UploadStatusResponse {
        id: upload.id,
        status: upload.status,
        row_count: upload.row_count,
        imported_count: upload.imported_count,
        duplicate_count: upload.duplicate_count,
        error_message: upload.error_message,
    }))
}

// ---------------------------------------------------------------------------
// Async categorization pipeline
// ---------------------------------------------------------------------------

/// Background task that categorizes uncategorized transactions after an upload.
///
/// Steps:
/// 1. Fetch uncategorized transactions for the account
/// 2. Load user overrides
/// 3. Create a Categorizer and call categorize_transactions
/// 4. Update transaction records with LLM results
/// 5. Send WebSocket progress events throughout
/// 6. Trigger recurring detection for the portfolio
/// 7. Send final WebSocket event
async fn run_categorization_pipeline(
    state: AppState,
    user_id: Uuid,
    upload_id: Uuid,
    account_id: Uuid,
) -> Result<(), AppError> {
    // Step 1: Fetch uncategorized transactions
    let uncategorized = state
        .transaction_repo()
        .find_uncategorized(account_id)
        .await?;

    if uncategorized.is_empty() {
        tracing::info!(upload_id = %upload_id, "No uncategorized transactions to process");
        return Ok(());
    }

    let total = uncategorized.len();

    // Step 2: Load user overrides
    let user_overrides = state.override_repo().list_by_user(user_id).await?;
    let override_patterns: Vec<OverridePattern> = user_overrides
        .iter()
        .map(|o| OverridePattern {
            pattern: o.description_pattern.clone(),
            category: o.category.clone(),
            subcategory: o.subcategory.clone(),
        })
        .collect();

    // Step 3: Build transaction inputs
    let transaction_inputs: Vec<TransactionInput> = uncategorized
        .iter()
        .map(|t| TransactionInput {
            id: t.id,
            date: t.date,
            amount: t.amount,
            description: t.description.clone(),
        })
        .collect();

    // Step 4: Categorize using the LLM client
    let llm_client = state
        .llm_client()
        .ok_or_else(|| AppError::ServiceUnavailable("LLM backend not available".to_string()))?;
    let categorizer = Categorizer::new(llm_client);

    // Send initial progress
    state
        .ws_manager()
        .send_to_user(
            user_id,
            WsMessage::CategorizationProgress {
                upload_id,
                categorized: 0,
                total,
                flagged: 0,
            },
        )
        .await;

    let report = categorizer
        .categorize_transactions(transaction_inputs, override_patterns)
        .await
        .map_err(|e| AppError::InternalError(format!("Categorization failed: {}", e)))?;

    // Step 5: Update transaction records with results
    let updates: Vec<LlmCategorizationUpdate> = report
        .results
        .iter()
        .map(|r| LlmCategorizationUpdate {
            transaction_id: r.transaction_id,
            category: r.category.clone(),
            subcategory: r.subcategory.clone(),
            merchant_name: r.merchant_name.clone(),
            llm_confidence: r.confidence,
        })
        .collect();

    state
        .transaction_repo()
        .update_llm_results(&updates)
        .await?;

    let flagged = report.flagged;

    // Send completion progress
    state
        .ws_manager()
        .send_to_user(
            user_id,
            WsMessage::CategorizationComplete {
                upload_id,
                total: report.results.len(),
                flagged,
            },
        )
        .await;

    // Step 6: Trigger recurring detection for the portfolio
    let account = state.account_repo().find_by_id(account_id).await?;
    let portfolio_id = account.portfolio_id;

    // Fetch all transactions for the portfolio to detect recurring patterns
    let (all_txns, _) = state
        .transaction_repo()
        .list(
            &finima_db::TransactionFilters {
                portfolio_id: Some(portfolio_id),
                ..Default::default()
            },
            &finima_db::Pagination {
                page: 1,
                per_page: 10_000,
            },
            &finima_db::Sort::default(),
        )
        .await?;

    let analysis_txns: Vec<finima_analysis::TransactionForAnalysis> = all_txns
        .iter()
        .map(|t| finima_analysis::TransactionForAnalysis {
            id: t.id,
            date: t.date,
            amount: t.amount,
            description: t.description.clone(),
            merchant_name: t.merchant_name.clone(),
            category: t.category.clone(),
            account_id: Some(t.account_id),
        })
        .collect();

    let recurring_candidates = finima_analysis::detect_recurring(&analysis_txns);

    // Upsert recurring groups
    for candidate in &recurring_candidates {
        let insert = finima_db::RecurringGroupInsert {
            merchant_name: candidate.merchant_name.clone(),
            category: candidate
                .category
                .clone()
                .unwrap_or_else(|| "other".to_string()),
            frequency: candidate.frequency,
            avg_amount: candidate.avg_amount,
            next_expected_date: candidate.next_expected_date,
            metadata: serde_json::json!({
                "transaction_count": candidate.transaction_count,
                "annual_cost": candidate.annual_cost.to_string(),
            }),
        };
        let _ = state.recurring_repo().upsert(portfolio_id, insert).await;
    }

    // Step 7: Send recurring detection event
    if !recurring_candidates.is_empty() {
        state
            .ws_manager()
            .send_to_user(
                user_id,
                WsMessage::RecurringDetected {
                    count: recurring_candidates.len(),
                },
            )
            .await;
    }

    tracing::info!(
        upload_id = %upload_id,
        categorized = report.results.len(),
        flagged,
        recurring = recurring_candidates.len(),
        "Categorization pipeline complete"
    );

    Ok(())
}
