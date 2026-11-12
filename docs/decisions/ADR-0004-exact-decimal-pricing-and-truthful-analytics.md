# ADR-0004: Exact Decimal Pricing and Truthful Analytics

## Status
Accepted

## Context
Floating point calculations introduce rounding drift in monetary calculations. Unknown model pricing must not be misreported as free.

## Decision
1. All monetary rates, token pricing, and cost estimates use `rust_decimal::Decimal` and PostgreSQL `NUMERIC(24, 12)`.
2. When model pricing is missing, `cost_estimate` remains `NULL` and `has_unpriced_usage` is flagged `TRUE`.

## Consequences
- Zero floating-point rounding errors.
- Accurate and truthful cost accounting.
