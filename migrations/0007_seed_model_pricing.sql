-- 0007_seed_model_pricing.sql
-- Seed baseline versioned model pricing rate cards for major providers.

INSERT INTO model_pricing (
    provider,
    model_pattern,
    match_type,
    currency,
    input_per_million,
    output_per_million,
    cached_input_per_million,
    effective_from,
    effective_until,
    source_url,
    source_checked_at,
    version
) VALUES
-- OpenAI exact models
('openai', 'gpt-4o', 'exact', 'USD', 2.500000000000, 10.000000000000, 1.250000000000, '2026-01-01 00:00:00+00', NULL, 'https://openai.com/api/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1'),
('openai', 'gpt-4o-2024-08-06', 'exact', 'USD', 2.500000000000, 10.000000000000, 1.250000000000, '2026-01-01 00:00:00+00', NULL, 'https://openai.com/api/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1'),
('openai', 'gpt-4o-mini', 'exact', 'USD', 0.150000000000, 0.600000000000, 0.075000000000, '2026-01-01 00:00:00+00', NULL, 'https://openai.com/api/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1'),
('openai', 'gpt-4o-mini-2024-07-18', 'exact', 'USD', 0.150000000000, 0.600000000000, 0.075000000000, '2026-01-01 00:00:00+00', NULL, 'https://openai.com/api/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1'),
('openai', 'o1', 'exact', 'USD', 15.000000000000, 60.000000000000, 7.500000000000, '2026-01-01 00:00:00+00', NULL, 'https://openai.com/api/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1'),
('openai', 'o1-preview', 'exact', 'USD', 15.000000000000, 60.000000000000, 7.500000000000, '2026-01-01 00:00:00+00', NULL, 'https://openai.com/api/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1'),
('openai', 'o1-mini', 'exact', 'USD', 3.000000000000, 12.000000000000, 1.500000000000, '2026-01-01 00:00:00+00', NULL, 'https://openai.com/api/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1'),
('openai', 'gpt-4-turbo', 'exact', 'USD', 10.000000000000, 30.000000000000, 5.000000000000, '2026-01-01 00:00:00+00', NULL, 'https://openai.com/api/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1'),
('openai', 'gpt-3.5-turbo', 'exact', 'USD', 0.500000000000, 1.500000000000, 0.250000000000, '2026-01-01 00:00:00+00', NULL, 'https://openai.com/api/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1'),

-- OpenAI prefix fallbacks
('openai', 'gpt-4o', 'prefix', 'USD', 2.500000000000, 10.000000000000, 1.250000000000, '2026-01-01 00:00:00+00', NULL, 'https://openai.com/api/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1'),
('openai', 'gpt-4o-mini', 'prefix', 'USD', 0.150000000000, 0.600000000000, 0.075000000000, '2026-01-01 00:00:00+00', NULL, 'https://openai.com/api/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1'),
('openai', 'o1-', 'prefix', 'USD', 15.000000000000, 60.000000000000, 7.500000000000, '2026-01-01 00:00:00+00', NULL, 'https://openai.com/api/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1'),

-- Anthropic exact models
('anthropic', 'claude-3-5-sonnet-20241022', 'exact', 'USD', 3.000000000000, 15.000000000000, 0.300000000000, '2026-01-01 00:00:00+00', NULL, 'https://www.anthropic.com/pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),
('anthropic', 'claude-3-5-sonnet-latest', 'exact', 'USD', 3.000000000000, 15.000000000000, 0.300000000000, '2026-01-01 00:00:00+00', NULL, 'https://www.anthropic.com/pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),
('anthropic', 'claude-3-5-haiku-20241022', 'exact', 'USD', 0.800000000000, 4.000000000000, 0.080000000000, '2026-01-01 00:00:00+00', NULL, 'https://www.anthropic.com/pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),
('anthropic', 'claude-3-opus-20240229', 'exact', 'USD', 15.000000000000, 75.000000000000, 1.500000000000, '2026-01-01 00:00:00+00', NULL, 'https://www.anthropic.com/pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),

-- Anthropic prefix fallbacks
('anthropic', 'claude-3-5-sonnet', 'prefix', 'USD', 3.000000000000, 15.000000000000, 0.300000000000, '2026-01-01 00:00:00+00', NULL, 'https://www.anthropic.com/pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),
('anthropic', 'claude-3-5-haiku', 'prefix', 'USD', 0.800000000000, 4.000000000000, 0.080000000000, '2026-01-01 00:00:00+00', NULL, 'https://www.anthropic.com/pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),
('anthropic', 'claude-3-opus', 'prefix', 'USD', 15.000000000000, 75.000000000000, 1.500000000000, '2026-01-01 00:00:00+00', NULL, 'https://www.anthropic.com/pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),

-- Google Gemini exact models
('google', 'gemini-1.5-pro', 'exact', 'USD', 1.250000000000, 5.000000000000, 0.312500000000, '2026-01-01 00:00:00+00', NULL, 'https://ai.google.dev/pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),
('google', 'gemini-1.5-flash', 'exact', 'USD', 0.075000000000, 0.300000000000, 0.018750000000, '2026-01-01 00:00:00+00', NULL, 'https://ai.google.dev/pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),
('google', 'gemini-2.0-flash', 'exact', 'USD', 0.100000000000, 0.400000000000, 0.025000000000, '2026-01-01 00:00:00+00', NULL, 'https://ai.google.dev/pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),

-- Google Gemini prefix fallbacks
('google', 'gemini-1.5-pro', 'prefix', 'USD', 1.250000000000, 5.000000000000, 0.312500000000, '2026-01-01 00:00:00+00', NULL, 'https://ai.google.dev/pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),
('google', 'gemini-1.5-flash', 'prefix', 'USD', 0.075000000000, 0.300000000000, 0.018750000000, '2026-01-01 00:00:00+00', NULL, 'https://ai.google.dev/pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),
('google', 'gemini-2.0', 'prefix', 'USD', 0.100000000000, 0.400000000000, 0.025000000000, '2026-01-01 00:00:00+00', NULL, 'https://ai.google.dev/pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),

-- Mistral exact models
('mistral', 'mistral-large-latest', 'exact', 'USD', 2.000000000000, 6.000000000000, 0.500000000000, '2026-01-01 00:00:00+00', NULL, 'https://mistral.ai/technology/#pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),
('mistral', 'mistral-small-latest', 'exact', 'USD', 0.200000000000, 0.600000000000, 0.050000000000, '2026-01-01 00:00:00+00', NULL, 'https://mistral.ai/technology/#pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),
('mistral', 'codestral-latest', 'exact', 'USD', 0.300000000000, 0.900000000000, 0.075000000000, '2026-01-01 00:00:00+00', NULL, 'https://mistral.ai/technology/#pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),

-- Mistral prefix fallbacks
('mistral', 'mistral-large', 'prefix', 'USD', 2.000000000000, 6.000000000000, 0.500000000000, '2026-01-01 00:00:00+00', NULL, 'https://mistral.ai/technology/#pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),
('mistral', 'mistral-small', 'prefix', 'USD', 0.200000000000, 0.600000000000, 0.050000000000, '2026-01-01 00:00:00+00', NULL, 'https://mistral.ai/technology/#pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),
('mistral', 'codestral', 'prefix', 'USD', 0.300000000000, 0.900000000000, 0.075000000000, '2026-01-01 00:00:00+00', NULL, 'https://mistral.ai/technology/#pricing', '2026-01-01 00:00:00+00', '2026-01-v1'),

-- DeepSeek exact models
('deepseek', 'deepseek-chat', 'exact', 'USD', 0.140000000000, 0.280000000000, 0.014000000000, '2026-01-01 00:00:00+00', NULL, 'https://platform.deepseek.com/api-docs/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1'),
('deepseek', 'deepseek-reasoner', 'exact', 'USD', 0.550000000000, 2.190000000000, 0.140000000000, '2026-01-01 00:00:00+00', NULL, 'https://platform.deepseek.com/api-docs/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1'),

-- DeepSeek prefix fallbacks
('deepseek', 'deepseek-', 'prefix', 'USD', 0.140000000000, 0.280000000000, 0.014000000000, '2026-01-01 00:00:00+00', NULL, 'https://platform.deepseek.com/api-docs/pricing/', '2026-01-01 00:00:00+00', '2026-01-v1')
ON CONFLICT (provider, model_pattern, version) DO NOTHING;
