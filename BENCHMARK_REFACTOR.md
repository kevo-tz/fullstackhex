# Benchmark Refactor Plan - AB Only

## Goal
Remove bombardier support, keep only Apache Bench (ab) for benchmarking.

## Current State
- `scripts/benchmark.sh` - Main benchmark library (keep)
- `scripts/benchmark_ab.sh` - AB adapter (keep)
- `scripts/benchmark_bombardier.sh` - Bombardier adapter (delete)

## Actions

### Phase 1: Remove Bombardier
1. Delete `scripts/benchmark_bombardier.sh`
2. Update `scripts/benchmark.sh` to remove bombardier references
3. Update `config.sh` to remove bombardier-related config

### Phase 2: Simplify
1. Make `benchmark.sh` use ab directly without adapter abstraction
2. Consolidate all benchmark logic into single file
3. Update `bench.sh` to use consolidated library

### Phase 3: Clean Up
1. Remove BENCH_BACKEND env var usage (not needed)
2. Update documentation/comments
3. Commit changes

## Files to Modify
- `scripts/benchmark.sh` - Remove bombadier references, simplify
- `scripts/benchmark_bombardier.sh` - DELETE
- `scripts/config.sh` - Remove bombardier config
- `scripts/bench.sh` - Use simplified benchmark lib