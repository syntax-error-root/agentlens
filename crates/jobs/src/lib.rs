#![forbid(unsafe_code)]

//! `agentlens-jobs` crate.
//! Durable transactional outbox processor, exponential backoff, and trace finalization.

use agentlens_alerts::AlertEngine;
use agentlens_database::{
    AlertRulesRepository, DailyMetricsRepository, DatabaseError, DbPool, ModelPricingRepository,
    NotificationDeliveriesRepository, OutboxRepository, TokenUsagesRepository, TracesRepository,
};
use agentlens_domain::{OutboxJob, TraceId, TraceStatus};
use agentlens_notifications::{deliver_notification, NotificationPayload};
use agentlens_observability::ObservabilityRegistry;
use agentlens_pricing::ModelPricingEngine;
use chrono::Utc;
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
    pub project_id: Option<Uuid>,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            worker_id: format!("worker-{}", Uuid::new_v4()),
            batch_size: 10,
            lease_duration_seconds: 30,
            poll_interval: Duration::from_millis(500),
            project_id: None,
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
                self.config.project_id,
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
        if let Ok(pending) = outbox_repo.get_pending_count(self.config.project_id).await {
            self.registry.set_outbox_pending_jobs(pending);
        }
        if let Ok(age) = outbox_repo
            .get_oldest_pending_age_seconds(self.config.project_id)
            .await
        {
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

                // 1. Evaluate pricing on token usages for this trace
                let pricing_repo = ModelPricingRepository::new(&self.pool);
                let rate_cards = pricing_repo.list_all().await?;
                let pricing_engine = ModelPricingEngine::new(rate_cards);

                let token_repo = TokenUsagesRepository::new(&self.pool);
                let usages = token_repo
                    .list_by_trace(payload.project_id, &trace_id)
                    .await?;

                for usage in usages {
                    let evaluated = pricing_engine.calculate_cost(
                        &usage.provider,
                        &usage.model,
                        usage.input_tokens,
                        usage.output_tokens,
                        usage.cached_input_tokens,
                        usage.reasoning_tokens,
                        usage.recorded_at,
                    );

                    if let Some(cost) = evaluated {
                        token_repo
                            .update_pricing_evaluation(
                                usage.id,
                                Some(cost.cost_estimate),
                                Some(&cost.pricing_version),
                                Some(cost.input_rate_per_million),
                                Some(cost.output_rate_per_million),
                                Some(cost.cached_rate_per_million),
                            )
                            .await?;
                    } else {
                        // Unpriced model: keep cost_estimate NULL, do not substitute $0.00
                        token_repo
                            .update_pricing_evaluation(usage.id, None, None, None, None, None)
                            .await?;
                    }
                }

                // 2. Finalize trace aggregations
                let traces_repo = TracesRepository::new(&self.pool);
                let finalized = traces_repo
                    .finalize_trace(payload.project_id, &trace_id)
                    .await?;

                let trace = match finalized {
                    Some(t) => t,
                    None => return Err(JobError::TraceNotFound(payload.trace_id)),
                };

                // 3. Roll up into daily_metrics
                let daily_repo = DailyMetricsRepository::new(&self.pool);
                let metric_date = trace.started_at.date_naive();
                let is_error = trace.status == TraceStatus::Error;
                daily_repo
                    .record_trace_metric(
                        trace.organization_id,
                        trace.project_id,
                        trace.environment_id,
                        metric_date,
                        trace.duration_ms,
                        trace.estimated_cost,
                        trace.total_input_tokens,
                        trace.total_output_tokens,
                        is_error,
                    )
                    .await?;

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

pub struct AlertEvaluatorRunner {
    pool: DbPool,
}

impl AlertEvaluatorRunner {
    #[must_use]
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn run_evaluation_pass(&self) -> Result<usize, JobError> {
        let rules_repo = AlertRulesRepository::new(&self.pool);
        let rules = rules_repo.list_all_enabled().await?;
        let engine = AlertEngine::new(&self.pool);
        let mut processed = 0;

        for rule in rules {
            match engine.evaluate_rule(&rule).await {
                Ok(Some(outcome)) => {
                    if let Err(e) = engine.process_evaluation(&rule, &outcome).await {
                        error!(rule_id = %rule.id, err = %e, "Failed to process alert evaluation outcome");
                    } else {
                        processed += 1;
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    error!(rule_id = %rule.id, err = %e, "Failed to evaluate alert rule");
                }
            }
        }

        Ok(processed)
    }
}

pub struct NotificationDeliveryRunner {
    pool: DbPool,
}

impl NotificationDeliveryRunner {
    #[must_use]
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn run_delivery_pass(
        &self,
        encryption_key: &[u8],
        allow_loopback: bool,
    ) -> Result<usize, JobError> {
        let deliveries_repo = NotificationDeliveriesRepository::new(&self.pool);
        let pending = deliveries_repo.list_pending(10).await?;
        let count = pending.len();

        for (delivery, channel, event) in pending {
            let payload = NotificationPayload {
                event_id: event.id,
                rule_id: event.rule_id,
                rule_name: format!("Alert {}", event.severity.as_str()),
                severity: event.severity.as_str().to_string(),
                status: event.status,
                summary: event.summary,
                details: event.details,
                triggered_at: event.last_triggered_at,
                project_name: None,
            };

            match deliver_notification(&channel, encryption_key, &payload, allow_loopback).await {
                Ok(msg_id) => {
                    let _ = deliveries_repo
                        .mark_delivered(delivery.id, msg_id.as_deref())
                        .await;
                }
                Err(err) => {
                    let attempts = delivery.attempts + 1;
                    let backoff = compute_backoff(attempts);
                    let next_attempt = Utc::now()
                        + chrono::Duration::from_std(backoff)
                            .unwrap_or_else(|_| chrono::Duration::seconds(60));
                    let _ = deliveries_repo
                        .mark_failed(delivery.id, &err.to_string(), next_attempt, attempts)
                        .await;
                }
            }
        }

        Ok(count)
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
