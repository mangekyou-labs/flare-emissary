//! FlareEmissary Indexer Benchmark
//!
//! Measures indexer pipeline performance against a live Flare RPC to verify
//! that block processing stays within the < 2s target latency.
//!
//! ## Usage
//!
//! ```bash
//! # Use default public Flare RPC (rate-limited)
//! cargo run --bin benchmark
//!
//! # Use a specific RPC endpoint
//! FLARE_RPC_URL="https://your-quicknode.quiknode.pro/xxx/" cargo run --bin benchmark
//!
//! # Customize block count
//! BENCHMARK_BLOCKS=100 cargo run --bin benchmark
//! ```
//!
//! The benchmark does NOT require a database — it measures RPC fetch + decode
//! latency only, which is the critical path for staying under 2s.

use std::time::{Duration, Instant};

use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::Filter;
use chrono::{TimeZone, Utc};

use flare_common::types::Chain;
use flare_decoders::DecoderRegistry;

/// Per-block timing metrics.
struct BlockMetrics {
    _block_number: u64,
    _block_timestamp_unix: u64,
    fetch_block_ms: f64,
    fetch_logs_ms: f64,
    decode_ms: f64,
    total_ms: f64,
    log_count: usize,
    decoded_event_count: usize,
    /// How far behind the chain tip this block was processed (wall clock - block timestamp).
    lag_from_tip_ms: f64,
}

/// Aggregate statistics computed from per-block metrics.
struct AggregateStats {
    block_count: usize,
    total_logs: usize,
    total_decoded_events: usize,

    avg_total_ms: f64,
    p50_total_ms: f64,
    p95_total_ms: f64,
    p99_total_ms: f64,
    max_total_ms: f64,

    avg_fetch_block_ms: f64,
    avg_fetch_logs_ms: f64,
    avg_decode_ms: f64,

    avg_lag_ms: f64,
    max_lag_ms: f64,
}

fn compute_percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn compute_stats(metrics: &[BlockMetrics]) -> AggregateStats {
    let n = metrics.len();
    assert!(n > 0);

    let total_logs: usize = metrics.iter().map(|m| m.log_count).sum();
    let total_decoded: usize = metrics.iter().map(|m| m.decoded_event_count).sum();

    let avg_total = metrics.iter().map(|m| m.total_ms).sum::<f64>() / n as f64;
    let avg_fetch_block = metrics.iter().map(|m| m.fetch_block_ms).sum::<f64>() / n as f64;
    let avg_fetch_logs = metrics.iter().map(|m| m.fetch_logs_ms).sum::<f64>() / n as f64;
    let avg_decode = metrics.iter().map(|m| m.decode_ms).sum::<f64>() / n as f64;
    let avg_lag = metrics.iter().map(|m| m.lag_from_tip_ms).sum::<f64>() / n as f64;
    let max_lag = metrics
        .iter()
        .map(|m| m.lag_from_tip_ms)
        .fold(0.0_f64, f64::max);

    let mut totals: Vec<f64> = metrics.iter().map(|m| m.total_ms).collect();
    totals.sort_by(|a, b| a.partial_cmp(b).unwrap());

    AggregateStats {
        block_count: n,
        total_logs,
        total_decoded_events: total_decoded,
        avg_total_ms: avg_total,
        p50_total_ms: compute_percentile(&totals, 50.0),
        p95_total_ms: compute_percentile(&totals, 95.0),
        p99_total_ms: compute_percentile(&totals, 99.0),
        max_total_ms: *totals.last().unwrap_or(&0.0),
        avg_fetch_block_ms: avg_fetch_block,
        avg_fetch_logs_ms: avg_fetch_logs,
        avg_decode_ms: avg_decode,
        avg_lag_ms: avg_lag,
        max_lag_ms: max_lag,
    }
}

fn print_report(stats: &AggregateStats, wall_elapsed: Duration, rpc_url: &str) {
    let target_ms = 2000.0;
    let pass = stats.p95_total_ms < target_ms;

    println!();
    println!("══════════════════════════════════════════════════════════════");
    println!("  FlareEmissary Indexer Benchmark Report");
    println!("══════════════════════════════════════════════════════════════");
    println!();
    println!("  RPC Endpoint:       {}", rpc_url);
    println!("  Blocks Processed:   {}", stats.block_count);
    println!("  Total Logs:         {}", stats.total_logs);
    println!("  Decoded Events:     {}", stats.total_decoded_events);
    println!("  Wall Clock Time:    {:.1}s", wall_elapsed.as_secs_f64());
    println!(
        "  Throughput:         {:.1} blocks/sec",
        stats.block_count as f64 / wall_elapsed.as_secs_f64()
    );
    println!();
    println!("  ── Pipeline Latency (per block) ──────────────────────────");
    println!(
        "  Fetch Block:        avg {:.1}ms",
        stats.avg_fetch_block_ms
    );
    println!("  Fetch Logs:         avg {:.1}ms", stats.avg_fetch_logs_ms);
    println!("  Decode:             avg {:.1}ms", stats.avg_decode_ms);
    println!("  Total (end-to-end): avg {:.1}ms", stats.avg_total_ms);
    println!();
    println!("  ── Latency Distribution ─────────────────────────────────");
    println!("  p50:   {:.1}ms", stats.p50_total_ms);
    println!("  p95:   {:.1}ms", stats.p95_total_ms);
    println!("  p99:   {:.1}ms", stats.p99_total_ms);
    println!("  max:   {:.1}ms", stats.max_total_ms);
    println!();
    println!("  ── Chain Tip Lag ────────────────────────────────────────");
    println!(
        "  avg lag:  {:.0}ms ({:.1}s)",
        stats.avg_lag_ms,
        stats.avg_lag_ms / 1000.0
    );
    println!(
        "  max lag:  {:.0}ms ({:.1}s)",
        stats.max_lag_ms,
        stats.max_lag_ms / 1000.0
    );
    println!();
    println!("  ── Result ─────────────────────────────────────────────");
    println!(
        "  Target: p95 < {:.0}ms    {}",
        target_ms,
        if pass { "✅ PASS" } else { "❌ FAIL" }
    );
    println!();
    println!("══════════════════════════════════════════════════════════════");
    println!();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // Initialize human-readable logging for the benchmark
    tracing_subscriber::fmt()
        .with_env_filter("benchmark=info,warn")
        .init();

    let rpc_url = std::env::var("FLARE_RPC_URL")
        .unwrap_or_else(|_| "https://flare-api.flare.network/ext/C/rpc".to_string());

    let block_count: usize = std::env::var("BENCHMARK_BLOCKS")
        .unwrap_or_else(|_| "50".to_string())
        .parse()
        .expect("BENCHMARK_BLOCKS must be a valid number");

    println!();
    println!("FlareEmissary Indexer Benchmark");
    println!("───────────────────────────────────────");
    println!("RPC:    {}", rpc_url);
    println!("Blocks: {}", block_count);
    println!();

    let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);

    let decoders = DecoderRegistry::new();
    let chain = Chain::Flare;

    // Get the latest block number and start from (latest - block_count)
    let latest = provider.get_block_number().await?;
    let start_block = latest.saturating_sub(block_count as u64);

    println!("Chain tip:   block #{}", latest);
    println!("Range:       #{} → #{}", start_block, latest);
    println!("Processing...");
    println!();

    let mut metrics = Vec::with_capacity(block_count);
    let wall_start = Instant::now();

    for block_num in start_block..latest {
        let block_start = Instant::now();

        // 1. Fetch block header
        let fetch_block_start = Instant::now();
        let block = match provider.get_block_by_number(block_num.into()).await? {
            Some(b) => b,
            None => {
                eprintln!("  ⚠ Block {} not found, skipping", block_num);
                continue;
            }
        };
        let fetch_block_ms = fetch_block_start.elapsed().as_secs_f64() * 1000.0;

        let block_timestamp = Utc
            .timestamp_opt(block.header.timestamp as i64, 0)
            .single()
            .unwrap_or_else(Utc::now);

        // 2. Fetch logs
        let fetch_logs_start = Instant::now();
        let filter = Filter::new().from_block(block_num).to_block(block_num);
        let logs = provider.get_logs(&filter).await?;
        let fetch_logs_ms = fetch_logs_start.elapsed().as_secs_f64() * 1000.0;

        // 3. Decode
        let decode_start = Instant::now();
        let mut decoded_count = 0;
        for log in &logs {
            if decoders
                .decode(&log.inner, block_num, block_timestamp, chain)
                .is_some()
            {
                decoded_count += 1;
            }
        }
        let decode_ms = decode_start.elapsed().as_secs_f64() * 1000.0;

        let total_ms = block_start.elapsed().as_secs_f64() * 1000.0;

        // Lag: how far behind the chain tip when we finished processing
        let now = Utc::now();
        let lag_ms = (now - block_timestamp).num_milliseconds() as f64;

        // Progress indicator every 10 blocks
        if (block_num - start_block) % 10 == 0 {
            println!(
                "  block #{} | {:.0}ms total | {} logs | {} decoded | lag {:.0}ms",
                block_num,
                total_ms,
                logs.len(),
                decoded_count,
                lag_ms
            );
        }

        metrics.push(BlockMetrics {
            _block_number: block_num,
            _block_timestamp_unix: block.header.timestamp,
            fetch_block_ms,
            fetch_logs_ms,
            decode_ms,
            total_ms,
            log_count: logs.len(),
            decoded_event_count: decoded_count,
            lag_from_tip_ms: lag_ms,
        });
    }

    let wall_elapsed = wall_start.elapsed();

    if metrics.is_empty() {
        println!("No blocks processed — nothing to report.");
        return Ok(());
    }

    let stats = compute_stats(&metrics);
    print_report(&stats, wall_elapsed, &rpc_url);

    // Exit with non-zero if benchmark fails
    if stats.p95_total_ms >= 2000.0 {
        std::process::exit(1);
    }

    Ok(())
}
