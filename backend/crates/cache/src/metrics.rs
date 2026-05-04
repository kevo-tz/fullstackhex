//! Redis operation metrics.
//!
//! Tracks operation counts and latency histograms for Redis commands.

use std::time::Instant;

/// Record a Redis operation metric.
pub fn record_operation(operation: &str, result: &Result<(), fred::error::Error>, duration: Instant) {
    let status = if result.is_ok() { "success" } else { "error" };
    let elapsed = duration.elapsed().as_secs_f64();

    metrics::counter!(
        "redis_operations_total",
        "operation" => operation.to_string(),
        "status" => status.to_string(),
    )
    .increment(1);

    metrics::histogram!(
        "redis_operation_duration_seconds",
        "operation" => operation.to_string(),
    )
    .record(elapsed);
}
