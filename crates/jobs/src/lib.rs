#![forbid(unsafe_code)]

//! `agentlens-jobs` crate.
//! Durable transactional outbox processor, exponential backoff, and trace finalization.

use agentlens_database::{DatabaseError, DbPool, OutboxRepository, TracesRepository};
use agentlens_domain::{OutboxJob, TraceId};
use agentlens_observability::ObservabilityRegistry;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum JobError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid job payload: {0}")]
    InvalidPayload(String),
    #[error("Unknown job type: {0}")]
    UnknownJobType(String),
    #[error("Failed to parse trace id: {0}")]
    InvalidTraceId(String),
    #[error("Trace not found for finalization: {0}")]
    TraceNotFound(String),
}

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub worker_id: String,
    pub batch_size: i64,
    pub lease_duration_seconds: i32,
    pub poll_interval: Duration,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            worker_id: format!("worker-{}", Uuid::new_v4()),
            batch_size: 10,
            lease_duration_seconds: 30,
            poll_interval: Duration::from_millis(500),
        }
    }
}

/// Payload format for `trace_ingested` outbox jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceIngestedPayload {
    pub organization_id: Option<Uuid>,
    pub project_id: Uuid,
    pub trace_id: String,
}

/// Computes backoff duration using exponential curve with jitter bounded to 300s:
/// backoff_seconds = min(300, 2^attempts) + jitter
#[must_use]
pub fn compute_backoff(attempts: i32) -> Duration {
    let exp_power = attempts.clamp(0, 30) as u32;
    let base = 2_u64.saturating_pow(exp_power);
    let capped = base.min(300);

    let mut rng = rand::thread_rng();
    let jitter: f64 = rng.gen_range(0.0..1.0);
    Duration::from_secs_f64(capped as f64 + jitter)
}

pub struct OutboxWorker {
    pool: DbPool,
    registry: Arc<ObservabilityRegistry>,
    config: WorkerConfig,
}

impl OutboxWorker {
    #[must_use]
    pub fn new(pool: DbPool, registry: Arc<ObservabilityRegistry>, config: WorkerConfig) -> Self {
        Self {
            pool,
            registry,
            config,
        }
    }

    /// Executes a single polling and execution pass for claimable outbox jobs.
    /// Returns the number of jobs processed in this pass.
    pub async fn run_once(&self) -> Result<usize, JobError> {
        let outbox_repo = OutboxRepository::new(&self.pool);
        let jobs = outbox_repo
            .claim_jobs(
                &self.config.worker_id,
                self.config.batch_size,
                self.config.lease_duration_seconds,
            )
            .await?;

        if jobs.is_empty() {
            return Ok(0);
        }

        let job_count = jobs.len();
        debug!(
            worker = %self.config.worker_id,
            count = job_count,
            "Claimed outbox job batch"
        );

        for job in jobs {
            self.process_job(&outbox_repo, job).await;
        }

        // Refresh outbox queue gauge metrics
        if let Ok(pending) = outbox_repo.get_pending_count().await {
            self.registry.set_outbox_pending_jobs(pending);
        }
        if let Ok(age) = outbox_repo.get_oldest_pending_age_seconds().await {
            self.registry.set_outbox_oldest_pending_age_seconds(age);
        }

        Ok(job_count)
    }

    async fn process_job(&self, outbox_repo: &OutboxRepository<'_>, job: OutboxJob) {
        let job_id = job.id;
        let attempts = job.attempts;
        let max_attempts = job.max_attempts;

        match self.dispatch_job(&job).await {
            Ok(()) => {
                if let Err(e) = outbox_repo.mark_completed(job_id).await {
                    error!(%job_id, err = %e, "Failed to mark outbox job completed");
                } else {
                    self.registry.record_job_processed();
                    debug!(%job_id, "Outbox job successfully completed");
                }
            }
            Err(err) => {
                let err_str = err.to_string();
                warn!(
                    %job_id,
                    attempts,
                    max_attempts,
                    error = %err_str,
                    "Outbox job processing failed"
                );

                let backoff = compute_backoff(attempts);
                if let Err(e) = outbox_repo
                    .mark_failed(
                        job_id,
                        attempts,
                        max_attempts,
                        &err_str,
                        backoff.as_secs() as i64,
                    )
                    .await
                {
                    error!(%job_id, err = %e, "Failed to record outbox job failure");
                }

                if attempts >= max_attempts {
                    self.registry.record_job_dead();
                    error!(%job_id, "Outbox job exhausted all retries and transitioned to dead");
                }
            }
        }
    }

    async fn dispatch_job(&self, job: &OutboxJob) -> Result<(), JobError> {
        match job.job_type.as_str() {
            "trace_ingested" => {
                let payload: TraceIngestedPayload = serde_json::from_value(job.payload.clone())?;
                let trace_id = TraceId::new(&payload.trace_id)
                    .map_err(|e| JobError::InvalidTraceId(e.to_string()))?;

                let traces_repo = TracesRepository::new(&self.pool);
                let finalized = traces_repo
                    .finalize_trace(payload.project_id, &trace_id)
                    .await?;

                if finalized.is_none() {
                    return Err(JobError::TraceNotFound(payload.trace_id));
                }

                Ok(())
            }
            "retention_cleanup" => {
                // Retention cleanup scaffolding
                debug!(job_id = %job.id, "Processed retention cleanup job");
                Ok(())
            }
            unknown => Err(JobError::UnknownJobType(unknown.to_string())),
        }
    }

    /// Runs the worker loop until a shutdown signal is received.
    pub async fn run(&self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        info!(
            worker = %self.config.worker_id,
            poll_interval_ms = self.config.poll_interval.as_millis(),
            "Starting outbox worker loop"
        );

        loop {
            if *shutdown.borrow() {
                info!(worker = %self.config.worker_id, "Shutdown signal received; exiting worker loop");
                break;
            }

            match self.run_once().await {
                Ok(count) => {
                    if count == 0 {
                        tokio::select! {
                            _ = tokio::time::sleep(self.config.poll_interval) => {}
                            _ = shutdown.changed() => {
                                if *shutdown.borrow() {
                                    break;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(err = %e, "Worker encountered error in run_once pass");
                    tokio::select! {
                        _ = tokio::time::sleep(self.config.poll_interval) => {}
                        _ = shutdown.changed() => {
                            if *shutdown.borrow() {
                                break;
                            }
                        }
                    }
                }
            }
        }

        info!(worker = %self.config.worker_id, "Outbox worker loop terminated cleanly");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_backoff_bounds() {
        for attempt in 0..10 {
            let dur = compute_backoff(attempt);
            let secs = dur.as_secs_f64();
            let base = 2_u64.pow(attempt as u32) as f64;
            assert!(secs >= base.min(300.0));
            assert!(secs <= (base.min(300.0) + 1.0));
        }

        // High attempt capped at 300s + jitter
        let capped_dur = compute_backoff(25);
        assert!(capped_dur.as_secs_f64() >= 300.0);
        assert!(capped_dur.as_secs_f64() <= 301.0);
    }
}
