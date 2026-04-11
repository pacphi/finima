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
    pub upload_id: Uuid,
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
pub struct ConfirmRequest {
    pub column_mapping: Option<ColumnMapping>,
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

    // Store only the S3 key and preview in the database (not the file content).
    let storage = serde_json::json!({
        "s3_key": s3_key,
        "preview": preview,
    });
    state
        .upload_repo()
        .update_column_mapping(upload.id, storage)
        .await?;

    let preview_json = serde_json::to_value(&preview).unwrap_or(serde_json::Value::Null);

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
            upload_id: upload.id,
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

    // Parse all rows
    let parser = parser_for_format(upload.format);
    let raw_transactions = parser
        .parse_all(&file_data, body.column_mapping.as_ref())
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
    let llm_client = state.llm_client();
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
