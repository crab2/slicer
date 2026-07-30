use crate::artifacts::workspace_layout::WorkspaceLayout;
use crate::domain::job::JobDto;
use crate::errors::{AppError, AppResult};
use crate::repositories::ledger_repository::LedgerRepository;

pub struct JobOrchestrator {
    ledger: LedgerRepository,
}

impl JobOrchestrator {
    pub fn new(layout: WorkspaceLayout) -> Self {
        Self {
            ledger: LedgerRepository::new(layout),
        }
    }

    pub fn enqueue_placeholder(&self, job_type: &str) -> AppResult<JobDto> {
        self.create_job(job_type)
    }

    pub fn create_job(&self, job_type: &str) -> AppResult<JobDto> {
        self.ledger.append_job(job_type)
    }

    pub fn list_jobs(&self) -> AppResult<Vec<JobDto>> {
        self.ledger.list_jobs()
    }

    pub fn update_progress(
        &self,
        job_id: &str,
        progress: u8,
        message: Option<&str>,
    ) -> AppResult<JobDto> {
        self.ledger.update_job_progress(job_id, progress, message)
    }

    pub fn mark_failed(&self, job_id: &str, error: &AppError, summary: &str) -> AppResult<JobDto> {
        self.ledger.mark_job_failed(job_id, error, summary)
    }

    pub fn complete_import(
        &self,
        connection: &mut sqlx::SqliteConnection,
        document_id: &str,
        job_id: &str,
        page_count: i64,
        message: &str,
    ) -> AppResult<()> {
        LedgerRepository::complete_import_transaction(
            connection,
            document_id,
            job_id,
            page_count,
            message,
        )
    }

    pub fn fail_import_if_active(
        &self,
        document_id: &str,
        job_id: &str,
        error: &AppError,
        summary: &str,
    ) -> AppResult<bool> {
        self.ledger
            .fail_import_if_active(document_id, job_id, error, summary)
    }

    pub fn recover_interrupted_jobs(&self) -> AppResult<Vec<JobDto>> {
        self.ledger.recover_interrupted_jobs()
    }

    pub fn record_error(&self, error: &AppError) -> AppResult<String> {
        self.ledger.record_error_with_id(error)
    }
}
