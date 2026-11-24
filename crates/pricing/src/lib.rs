#![forbid(unsafe_code)]

//! `agentlens-pricing` crate.
//!
//! Provides versioned model rate card evaluation and exact decimal cost estimation
//! adhering to the truthful accounting principles in ADR-0004 and ADR-0013.

use agentlens_domain::ModelPricing;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// Result of evaluated token pricing snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedCost {
    pub cost_estimate: Decimal,
    pub currency: String,
    pub pricing_version: String,
    pub input_rate_per_million: Decimal,
    pub output_rate_per_million: Decimal,
    pub cached_rate_per_million: Decimal,
}

/// In-memory engine for versioned model pricing lookups and cost evaluations.
#[derive(Debug, Clone, Default)]
pub struct ModelPricingEngine {
    pricing_cards: Vec<ModelPricing>,
}

impl ModelPricingEngine {
    /// Create a new pricing engine populated with rate cards.
    #[must_use]
    pub fn new(pricing_cards: Vec<ModelPricing>) -> Self {
        Self { pricing_cards }
    }

    /// Add or update rate cards in the engine.
    pub fn update_cards(&mut self, pricing_cards: Vec<ModelPricing>) {
        self.pricing_cards = pricing_cards;
    }

    /// Match a pricing card for `(provider, model)` valid at `recorded_at`.
    ///
    /// Precedence rules:
    /// 1. Exact match (`match_type = 'exact'`), case-insensitive provider, case-sensitive model pattern.
    /// 2. Prefix match (`match_type = 'prefix'`), case-insensitive provider, longest model pattern match.
    /// 3. Bounded strictly by `effective_from <= recorded_at` and `(effective_until IS NULL OR effective_until > recorded_at)`.
    #[must_use]
    pub fn match_pricing(
        &self,
        provider: &str,
        model: &str,
        recorded_at: DateTime<Utc>,
    ) -> Option<&ModelPricing> {
        let provider_norm = provider.trim().to_lowercase();
        let model_norm = model.trim();

        // 1. Try exact match first
        let exact_match = self.pricing_cards.iter().find(|card| {
            card.provider.trim().to_lowercase() == provider_norm
                && card.match_type == "exact"
                && card.model_pattern.trim() == model_norm
                && card.effective_from <= recorded_at
                && card
                    .effective_until
                    .map_or(true, |until| until > recorded_at)
        });

        if exact_match.is_some() {
            return exact_match;
        }

        // 2. Fall back to longest prefix match
        let prefix_matches = self.pricing_cards.iter().filter(|card| {
            card.provider.trim().to_lowercase() == provider_norm
                && card.match_type == "prefix"
                && model_norm.starts_with(card.model_pattern.trim())
                && card.effective_from <= recorded_at
                && card
                    .effective_until
                    .map_or(true, |until| until > recorded_at)
        });

        // Pick the prefix match with the longest pattern length for most specific specificity
        prefix_matches.max_by_key(|card| card.model_pattern.trim().len())
    }

    /// Calculate exact decimal cost for given token quantities using the matched rate card.
    ///
    /// Accounting Invariant:
    /// non_cached_input = max(0, input_tokens - cached_input_tokens)
    /// total_cost = (non_cached_input * R_in + cached_input * R_cache + (output_tokens + reasoning_tokens) * R_out) / 1,000,000
    ///
    /// Returns `None` if no pricing card matches. Never returns `$0.00` for unpriced models.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_cost(
        &self,
        provider: &str,
        model: &str,
        input_tokens: i64,
        output_tokens: i64,
        cached_input_tokens: i64,
        reasoning_tokens: i64,
        recorded_at: DateTime<Utc>,
    ) -> Option<EvaluatedCost> {
        let pricing = self.match_pricing(provider, model, recorded_at)?;
        Some(Self::compute_cost_from_card(
            pricing,
            input_tokens,
            output_tokens,
            cached_input_tokens,
            reasoning_tokens,
        ))
    }

    /// Compute cost snapshot from an already matched card.
    #[must_use]
    pub fn compute_cost_from_card(
        card: &ModelPricing,
        input_tokens: i64,
        output_tokens: i64,
        cached_input_tokens: i64,
        reasoning_tokens: i64,
    ) -> EvaluatedCost {
        let one_million = Decimal::from(1_000_000);

        let input_rate = card.input_per_million.unwrap_or(Decimal::ZERO);
        let output_rate = card.output_per_million.unwrap_or(Decimal::ZERO);
        // If cached rate is not explicitly configured, fall back to standard input rate
        let cached_rate = card.cached_input_per_million.unwrap_or(input_rate);

        let safe_input = input_tokens.max(0);
        let safe_cached = cached_input_tokens.max(0);
        let non_cached_input = (safe_input - safe_cached).max(0);
        let safe_output = output_tokens.max(0);
        let safe_reasoning = reasoning_tokens.max(0);
        let total_billable_output = safe_output + safe_reasoning;

        let input_cost = Decimal::from(non_cached_input) * input_rate;
        let cached_cost = Decimal::from(safe_cached) * cached_rate;
        let output_cost = Decimal::from(total_billable_output) * output_rate;

        let total_numerator = input_cost + cached_cost + output_cost;
        let cost_estimate = total_numerator / one_million;

        EvaluatedCost {
            cost_estimate,
            currency: card.currency.clone(),
            pricing_version: card.version.clone(),
            input_rate_per_million: input_rate,
            output_rate_per_million: output_rate,
            cached_rate_per_million: cached_rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::str::FromStr;
    use uuid::Uuid;

    fn sample_pricing_cards() -> Vec<ModelPricing> {
        let epoch = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        vec![
            ModelPricing {
                id: Uuid::new_v4(),
                provider: "openai".to_string(),
                model_pattern: "gpt-4o".to_string(),
                match_type: "exact".to_string(),
                currency: "USD".to_string(),
                input_per_million: Some(Decimal::from_str("2.50").unwrap()),
                output_per_million: Some(Decimal::from_str("10.00").unwrap()),
                cached_input_per_million: Some(Decimal::from_str("1.25").unwrap()),
                effective_from: epoch,
                effective_until: None,
                source_url: None,
                source_checked_at: None,
                version: "2026-01-v1".to_string(),
                created_at: epoch,
            },
            ModelPricing {
                id: Uuid::new_v4(),
                provider: "openai".to_string(),
                model_pattern: "gpt-4o".to_string(),
                match_type: "prefix".to_string(),
                currency: "USD".to_string(),
                input_per_million: Some(Decimal::from_str("2.50").unwrap()),
                output_per_million: Some(Decimal::from_str("10.00").unwrap()),
                cached_input_per_million: Some(Decimal::from_str("1.25").unwrap()),
                effective_from: epoch,
                effective_until: None,
                source_url: None,
                source_checked_at: None,
                version: "2026-01-v1".to_string(),
                created_at: epoch,
            },
            ModelPricing {
                id: Uuid::new_v4(),
                provider: "openai".to_string(),
                model_pattern: "gpt-4o-mini".to_string(),
                match_type: "prefix".to_string(),
                currency: "USD".to_string(),
                input_per_million: Some(Decimal::from_str("0.15").unwrap()),
                output_per_million: Some(Decimal::from_str("0.60").unwrap()),
                cached_input_per_million: Some(Decimal::from_str("0.075").unwrap()),
                effective_from: epoch,
                effective_until: None,
                source_url: None,
                source_checked_at: None,
                version: "2026-01-v1".to_string(),
                created_at: epoch,
            },
        ]
    }

    #[test]
    fn test_exact_match_precedence_and_calculation() {
        let engine = ModelPricingEngine::new(sample_pricing_cards());
        let now = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();

        // 1,000 input (200 cached), 500 output, 100 reasoning
        // non_cached = 800
        // cost = (800 * 2.50 + 200 * 1.25 + (500 + 100) * 10.00) / 1_000_000
        // cost = (2000 + 250 + 6000) / 1_000_000 = 8250 / 1_000_000 = 0.008250
        let evaluated = engine
            .calculate_cost("openai", "gpt-4o", 1000, 500, 200, 100, now)
            .expect("should match gpt-4o");

        assert_eq!(
            evaluated.cost_estimate,
            Decimal::from_str("0.00825").unwrap()
        );
        assert_eq!(evaluated.currency, "USD");
        assert_eq!(evaluated.pricing_version, "2026-01-v1");
        assert_eq!(
            evaluated.input_rate_per_million,
            Decimal::from_str("2.50").unwrap()
        );
        assert_eq!(
            evaluated.output_rate_per_million,
            Decimal::from_str("10.00").unwrap()
        );
        assert_eq!(
            evaluated.cached_rate_per_million,
            Decimal::from_str("1.25").unwrap()
        );
    }

    #[test]
    fn test_longest_prefix_match() {
        let engine = ModelPricingEngine::new(sample_pricing_cards());
        let now = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();

        // "gpt-4o-mini-2024-07-18" matches both "gpt-4o" prefix and "gpt-4o-mini" prefix.
        // It must choose "gpt-4o-mini" because it's longer.
        let evaluated = engine
            .calculate_cost("OpenAI", "gpt-4o-mini-2024-07-18", 10000, 1000, 0, 0, now)
            .expect("should match prefix");

        assert_eq!(
            evaluated.input_rate_per_million,
            Decimal::from_str("0.15").unwrap()
        );
        assert_eq!(
            evaluated.output_rate_per_million,
            Decimal::from_str("0.60").unwrap()
        );
    }

    #[test]
    fn test_cached_token_accounting_invariant() {
        let engine = ModelPricingEngine::new(sample_pricing_cards());
        let now = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();

        // If cached_input_tokens > input_tokens (buggy client telemetry),
        // non_cached_input must clamp to 0 and not produce negative costs.
        let evaluated = engine
            .calculate_cost("openai", "gpt-4o", 100, 0, 500, 0, now)
            .expect("should match");

        // non_cached = max(0, 100 - 500) = 0
        // cached_cost = 500 * 1.25 = 625
        // total = 625 / 1_000_000 = 0.000625
        assert_eq!(
            evaluated.cost_estimate,
            Decimal::from_str("0.000625").unwrap()
        );
    }

    #[test]
    fn test_unpriced_model_returns_none() {
        let engine = ModelPricingEngine::new(sample_pricing_cards());
        let now = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();

        let evaluated =
            engine.calculate_cost("unknown_provider", "llama-custom-99b", 1000, 500, 0, 0, now);
        assert!(
            evaluated.is_none(),
            "Unpriced models must return None, never $0.00"
        );
    }

    #[test]
    fn test_expired_pricing_card_ignored() {
        let epoch = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let until = Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap();
        let query_time = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();

        let cards = vec![ModelPricing {
            id: Uuid::new_v4(),
            provider: "openai".to_string(),
            model_pattern: "legacy-model".to_string(),
            match_type: "exact".to_string(),
            currency: "USD".to_string(),
            input_per_million: Some(Decimal::from_str("10.00").unwrap()),
            output_per_million: Some(Decimal::from_str("30.00").unwrap()),
            cached_input_per_million: None,
            effective_from: epoch,
            effective_until: Some(until),
            source_url: None,
            source_checked_at: None,
            version: "2025-v1".to_string(),
            created_at: epoch,
        }];

        let engine = ModelPricingEngine::new(cards);
        assert!(engine
            .calculate_cost("openai", "legacy-model", 100, 100, 0, 0, query_time)
            .is_none());
    }
}
