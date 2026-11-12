# ADR-0013: Versioned Pricing Matcher and Analytics Aggregations

## Status
Accepted (System-1 Architect Verified)

## Context
AI model cost accounting demands exact financial determinism without floating-point inaccuracy.
Provider rate changes (e.g. price drops or new models) must not retroactively mutate past billing calculations.
Token reporting varies across providers: some bundle cached tokens within `input_tokens` while others report them separately.
When telemetry reports a model without a configured pricing card, misreporting cost as `$0.00` violates observational integrity and skews financial projections.
Furthermore, multi-tenant latency analytics across millions of spans requires statistically robust continuous percentiles ($P_{50}$, $P_{95}$, $P_{99}$) computed efficiently without memory-intensive client-side sortation.

## Decision
1. **Financial Precision:** All token rates and cost estimates are represented via `rust_decimal::Decimal` mapping to PostgreSQL `NUMERIC(24, 12)` rate columns and `NUMERIC(18, 6)` cost columns.
2. **Deterministic Matching Strategy:**
   - Match `(provider, model)` against `model_pricing` by exact match first (`match_type = 'exact'`).
   - If no exact match is found, match by longest prefix match (`match_type = 'prefix'`).
   - Pricing entries are bounded by time: `effective_from <= recorded_at AND (effective_until IS NULL OR effective_until > recorded_at)`.
3. **Historical Rate Snapshotting:**
   - Evaluated rates (`pricing_version`, `input_rate_per_million`, `output_rate_per_million`, `cached_rate_per_million`, `cost_estimate`) are snapshotted directly onto `token_usages`.
   - Past traces retain immutable historical costs even when rates are updated or retired.
4. **Cached Token Accounting Invariant:**
   - To prevent negative values and double-counting:
     $$\text{non\_cached\_input} = \max(0, \text{input\_tokens} - \text{cached\_input\_tokens})$$
   - Total cost estimate:
     $$\text{cost\_estimate} = \frac{\text{non\_cached\_input} \times R_{\text{in}} + \text{cached\_input} \times R_{\text{cache}} + (\text{output\_tokens} + \text{reasoning\_tokens}) \times R_{\text{out}}}{1\,000\,000}$$
5. **Truthful Unknown Model Accounting:**
   - If no rate matches `(provider, model)`:
     - `token_usages.cost_estimate = NULL`
     - `traces.has_unpriced_usage = TRUE`
     - Never substitute `$0.00` or arbitrary defaults.
6. **Continuous Percentile & Rollup Aggregation:**
   - Latency percentiles use PostgreSQL `percentile_cont(ARRAY[0.50, 0.95, 0.99]) WITHIN GROUP (ORDER BY duration_ms)`.
   - Trace completion worker maintains daily rollup records in `daily_metrics` for high-throughput dashboard querying.

## Consequences
- Guaranteed zero rounding drift across all token cost computations.
- Provable historical immutability across pricing rate lifecycle changes.
- Explicit and observable indication of unpriced usage.
- High-performance, statistically valid latency metric calculations.
