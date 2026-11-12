#![forbid(unsafe_code)]

//! `agentlens-observability` crate.
//! Structured logging and internal Prometheus metrics registry.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct ObservabilityRegistry {
    outbox_pending_jobs: AtomicI64,
    outbox_oldest_pending_age_ms: AtomicU64,
    worker_jobs_processed_total: AtomicU64,
    worker_jobs_dead_total: AtomicU64,
    otlp_spans_ingested_total: AtomicU64,
}

impl ObservabilityRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_spans_ingested(&self, count: u64) {
        self.otlp_spans_ingested_total
            .fetch_add(count, Ordering::Relaxed);
    }

    pub fn record_job_processed(&self) {
        self.worker_jobs_processed_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_job_dead(&self) {
        self.worker_jobs_dead_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_outbox_pending_jobs(&self, count: i64) {
        self.outbox_pending_jobs.store(count, Ordering::Relaxed);
    }

    pub fn set_outbox_oldest_pending_age_seconds(&self, seconds: f64) {
        let ms = (seconds.max(0.0) * 1000.0) as u64;
        self.outbox_oldest_pending_age_ms
            .store(ms, Ordering::Relaxed);
    }

    #[must_use]
    pub fn get_outbox_pending_jobs(&self) -> i64 {
        self.outbox_pending_jobs.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn get_worker_jobs_processed(&self) -> u64 {
        self.worker_jobs_processed_total.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn get_worker_jobs_dead(&self) -> u64 {
        self.worker_jobs_dead_total.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn get_spans_ingested(&self) -> u64 {
        self.otlp_spans_ingested_total.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn render_prometheus(&self) -> String {
        let pending = self.outbox_pending_jobs.load(Ordering::Relaxed);
        let oldest_age_s =
            (self.outbox_oldest_pending_age_ms.load(Ordering::Relaxed) as f64) / 1000.0;
        let processed = self.worker_jobs_processed_total.load(Ordering::Relaxed);
        let dead = self.worker_jobs_dead_total.load(Ordering::Relaxed);
        let spans = self.otlp_spans_ingested_total.load(Ordering::Relaxed);

        format!(
            "# HELP agentlens_outbox_pending_jobs Number of pending outbox jobs waiting for processing\n\
             # TYPE agentlens_outbox_pending_jobs gauge\n\
             agentlens_outbox_pending_jobs {}\n\n\
             # HELP agentlens_outbox_oldest_pending_age_seconds Age in seconds of the oldest pending outbox job\n\
             # TYPE agentlens_outbox_oldest_pending_age_seconds gauge\n\
             agentlens_outbox_oldest_pending_age_seconds {:.3}\n\n\
             # HELP agentlens_worker_jobs_processed_total Total count of background jobs successfully processed\n\
             # TYPE agentlens_worker_jobs_processed_total counter\n\
             agentlens_worker_jobs_processed_total {}\n\n\
             # HELP agentlens_worker_jobs_dead_total Total count of background jobs that exceeded max attempts and died\n\
             # TYPE agentlens_worker_jobs_dead_total counter\n\
             agentlens_worker_jobs_dead_total {}\n\n\
             # HELP agentlens_otlp_spans_ingested_total Total count of OTLP spans ingested into AgentLens\n\
             # TYPE agentlens_otlp_spans_ingested_total counter\n\
             agentlens_otlp_spans_ingested_total {}\n",
            pending, oldest_age_s, processed, dead, spans
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observability_registry_metrics() {
        let reg = ObservabilityRegistry::new();
        reg.record_spans_ingested(42);
        reg.record_job_processed();
        reg.record_job_dead();
        reg.set_outbox_pending_jobs(5);
        reg.set_outbox_oldest_pending_age_seconds(12.345);

        assert_eq!(reg.get_spans_ingested(), 42);
        assert_eq!(reg.get_worker_jobs_processed(), 1);
        assert_eq!(reg.get_worker_jobs_dead(), 1);
        assert_eq!(reg.get_outbox_pending_jobs(), 5);

        let rendered = reg.render_prometheus();
        assert!(rendered.contains("agentlens_outbox_pending_jobs 5"));
        assert!(rendered.contains("agentlens_outbox_oldest_pending_age_seconds 12.345"));
        assert!(rendered.contains("agentlens_worker_jobs_processed_total 1"));
        assert!(rendered.contains("agentlens_worker_jobs_dead_total 1"));
        assert!(rendered.contains("agentlens_otlp_spans_ingested_total 42"));
    }
}
