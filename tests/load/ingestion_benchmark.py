#!/usr/bin/env python3
"""
AgentLens Ingestion Load Benchmark
Simulates concurrent clients streaming OTLP JSON span batches to POST /v1/traces.
Measures throughput (spans/sec), request latencies (P50, P95, P99), and error rates.
"""

import argparse
import json
import math
import secrets
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed


def generate_trace_id() -> str:
    return secrets.token_hex(16)


def generate_span_id() -> str:
    return secrets.token_hex(8)


def build_otlp_payload(spans_per_req: int = 10) -> dict:
    trace_id = generate_trace_id()
    now_nano = int(time.time() * 1_000_000_000)

    spans = []
    for _ in range(spans_per_req):
        start_nano = now_nano - 50_000_000  # 50ms ago
        end_nano = now_nano
        span_id = generate_span_id()

        spans.append(
            {
                "traceId": trace_id,
                "spanId": span_id,
                "name": "benchmark_llm_call",
                "kind": 3,  # SPAN_KIND_CLIENT
                "startTimeUnixNano": str(start_nano),
                "endTimeUnixNano": str(end_nano),
                "attributes": [
                    {"key": "gen_ai.system", "value": {"stringValue": "openai"}},
                    {"key": "gen_ai.request.model", "value": {"stringValue": "gpt-4o"}},
                    {"key": "gen_ai.usage.input_tokens", "value": {"intValue": 150}},
                    {"key": "gen_ai.usage.output_tokens", "value": {"intValue": 42}},
                    {"key": "openinference.span.kind", "value": {"stringValue": "llm"}},
                ],
                "status": {"code": 1},  # STATUS_CODE_OK
            }
        )

    return {
        "resourceSpans": [
            {
                "resource": {
                    "attributes": [
                        {"key": "service.name", "value": {"stringValue": "benchmark-client"}},
                        {"key": "agentlens.agent.name", "value": {"stringValue": "benchmark-agent"}},
                        {"key": "agentlens.agent.version", "value": {"stringValue": "v1.0.0"}},
                    ]
                },
                "scopeSpans": [
                    {
                        "scope": {"name": "agentlens.benchmark", "version": "1.0.0"},
                        "spans": spans,
                    }
                ],
            }
        ]
    }


def send_batch(endpoint: str, api_key: str, payload_bytes: bytes) -> tuple[bool, float, int]:
    req = urllib.request.Request(
        url=f"{endpoint.rstrip('/')}/v1/traces",
        data=payload_bytes,
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {api_key}",
            "User-Agent": "AgentLens-Benchmark/1.0",
        },
        method="POST",
    )

    start = time.perf_counter()
    status_code = 0
    success = False
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            status_code = resp.status
            success = status_code in (200, 202)
    except urllib.error.HTTPError as err:
        status_code = err.code
        success = False
    except Exception:
        status_code = 0
        success = False
    elapsed_ms = (time.perf_counter() - start) * 1000.0
    return success, elapsed_ms, status_code


def percentile(data: list[float], pct: float) -> float:
    if not data:
        return 0.0
    k = (len(data) - 1) * (pct / 100.0)
    f = math.floor(k)
    c = math.ceil(k)
    if f == c:
        return data[int(k)]
    d0 = data[int(f)] * (c - k)
    d1 = data[int(c)] * (k - f)
    return d0 + d1


def run_benchmark(
    endpoint: str,
    api_key: str,
    concurrency: int = 100,
    total_requests: int = 1000,
    spans_per_req: int = 10,
):
    print("=" * 60)
    print("AgentLens Ingestion Benchmark Suite")
    print("=" * 60)
    print(f"Target Endpoint:  {endpoint}/v1/traces")
    print(f"Concurrency:      {concurrency} clients")
    print(f"Total Requests:   {total_requests}")
    print(f"Spans/Request:    {spans_per_req}")
    print(f"Total Spans:      {total_requests * spans_per_req}")
    print("-" * 60)

    # Pre-generate payloads to avoid client-side JSON serialization bottleneck during measurement
    payloads = [
        json.dumps(build_otlp_payload(spans_per_req)).encode("utf-8")
        for _ in range(min(total_requests, 50))
    ]

    latencies: list[float] = []
    success_count = 0
    error_count = 0

    benchmark_start = time.perf_counter()

    with ThreadPoolExecutor(max_workers=concurrency) as executor:
        futures = [
            executor.submit(
                send_batch,
                endpoint,
                api_key,
                payloads[i % len(payloads)],
            )
            for i in range(total_requests)
        ]

        for future in as_completed(futures):
            ok, lat, _ = future.result()
            latencies.append(lat)
            if ok:
                success_count += 1
            else:
                error_count += 1

    total_duration = time.perf_counter() - benchmark_start
    latencies.sort()

    total_spans = success_count * spans_per_req
    throughput_rps = total_requests / total_duration if total_duration > 0 else 0
    throughput_sps = total_spans / total_duration if total_duration > 0 else 0

    p50 = percentile(latencies, 50)
    p90 = percentile(latencies, 90)
    p95 = percentile(latencies, 95)
    p99 = percentile(latencies, 99)

    print("\nBenchmark Execution Results:")
    print(f"Total Duration:   {total_duration:.2f} s")
    print(f"Throughput:       {throughput_rps:.1f} req/s ({throughput_sps:.1f} spans/s)")
    print(f"Success Count:    {success_count} / {total_requests} ({(success_count / total_requests) * 100:.1f}%)")
    print(f"Error Count:      {error_count}")
    print(f"Latency P50:      {p50:.2f} ms")
    print(f"Latency P90:      {p90:.2f} ms")
    print(f"Latency P95:      {p95:.2f} ms")
    print(f"Latency P99:      {p99:.2f} ms")
    print("=" * 60)

    return {
        "concurrency": concurrency,
        "total_requests": total_requests,
        "spans_per_request": spans_per_req,
        "total_spans": total_spans,
        "duration_seconds": round(total_duration, 3),
        "requests_per_sec": round(throughput_rps, 2),
        "spans_per_sec": round(throughput_sps, 2),
        "p50_latency_ms": round(p50, 2),
        "p95_latency_ms": round(p95, 2),
        "p99_latency_ms": round(p99, 2),
        "error_count": error_count,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="AgentLens Ingestion Benchmark")
    parser.add_argument("--endpoint", default="http://localhost:4318", help="OTLP endpoint")
    parser.add_argument("--api-key", default="agl_live_mock_key_for_testing", help="Ingestion API key")
    parser.add_argument("--concurrency", type=int, default=100, help="Concurrent workers")
    parser.add_argument("--requests", type=int, default=1000, help="Total requests to send")
    parser.add_argument("--spans-per-req", type=int, default=10, help="Spans per batch")
    args = parser.parse_args()

    run_benchmark(
        endpoint=args.endpoint,
        api_key=args.api_key,
        concurrency=args.concurrency,
        total_requests=args.requests,
        spans_per_req=args.spans_per_req,
    )
