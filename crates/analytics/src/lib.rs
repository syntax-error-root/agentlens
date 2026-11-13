#![forbid(unsafe_code)]

//! `agentlens-analytics` crate.
//! High-level analytics aggregations, metric service, and truthful reporting.

use agentlens_database::{AnalyticsRepository, DatabaseError, DbPool};
use agentlens_domain::{
    AgentAnalyticsItem, AnalyticsFilters, AnalyticsOverview, CostAnalyticsResult,
    ErrorAnalyticsResult, LatencyAnalyticsResult, ModelAnalyticsItem, TokenAnalyticsResult,
    ToolAnalyticsItem,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnalyticsError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
    #[error("Invalid filter: {0}")]
    InvalidFilter(String),
}

#[derive(Debug, Clone)]
pub struct AnalyticsService {
    pool: DbPool,
}

impl AnalyticsService {
    #[must_use]
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn get_overview(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<AnalyticsOverview, AnalyticsError> {
        let repo = AnalyticsRepository::new(&self.pool);
        let res = repo.get_overview(filters).await?;
        Ok(res)
    }

    pub async fn get_tokens(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<TokenAnalyticsResult, AnalyticsError> {
        let repo = AnalyticsRepository::new(&self.pool);
        let res = repo.get_tokens(filters).await?;
        Ok(res)
    }

    pub async fn get_costs(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<CostAnalyticsResult, AnalyticsError> {
        let repo = AnalyticsRepository::new(&self.pool);
        let res = repo.get_costs(filters).await?;
        Ok(res)
    }

    pub async fn get_models(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<Vec<ModelAnalyticsItem>, AnalyticsError> {
        let repo = AnalyticsRepository::new(&self.pool);
        let res = repo.get_models(filters).await?;
        Ok(res)
    }

    pub async fn get_agents(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<Vec<AgentAnalyticsItem>, AnalyticsError> {
        let repo = AnalyticsRepository::new(&self.pool);
        let res = repo.get_agents(filters).await?;
        Ok(res)
    }

    pub async fn get_tools(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<Vec<ToolAnalyticsItem>, AnalyticsError> {
        let repo = AnalyticsRepository::new(&self.pool);
        let res = repo.get_tools(filters).await?;
        Ok(res)
    }

    pub async fn get_latency(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<LatencyAnalyticsResult, AnalyticsError> {
        let repo = AnalyticsRepository::new(&self.pool);
        let res = repo.get_latency(filters).await?;
        Ok(res)
    }

    pub async fn get_errors(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<ErrorAnalyticsResult, AnalyticsError> {
        let repo = AnalyticsRepository::new(&self.pool);
        let res = repo.get_errors(filters).await?;
        Ok(res)
    }
}
