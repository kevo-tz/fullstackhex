# Detailed Implementation Plan for Deferred Items

## Overview

This document outlines detailed implementation plans for the deferred items identified during the scripts improvement initiative. These items were deferred to allow focus on higher-priority foundational improvements first.

---

## Item 1: Consolidate bench.sh and bench-lite.sh to reduce duplication

### Current State
- `bench.sh`: Uses bombardier for benchmarking (more features, requires Go)
- `bench-lite.sh`: Uses Apache Bench (ab) for benchmarking (lite version, wider availability)
- Both scripts share significant duplication in:
  - Service checking logic
  - Configuration handling
  - Output formatting
  - Help text structure
  - Result processing logic

### Implementation Plan

#### Phase 1: Create Unified Benchmark Interface
1. Create a new shared benchmark library: `scripts/benchmark.sh`
2. Extract common functionality:
   - Service checking functions
   - Configuration loading
   - Result formatting utilities
   - Help/usage text generation
   - JSON output handling

#### Phase 2: Implement Backend Agnostic Design
1. Create benchmark adapter interface:
   - Define common functions: `run_benchmark`, `parse_results`, `format_output`
   - Create adapter for bombardier: `benchmark_bombardier.sh`
   - Create adapter for ab: `benchmark_ab.sh`
   - Allow selection via environment variable or command flag

#### Phase 3: Consolidate Scripts
1. Replace both `bench.sh` and `bench-lite.sh` with thin wrappers:
   - `bench.sh`: Default to bombardier adapter
   - `bench-lite.sh`: Default to ab adapter
   - Both delegate to shared benchmark library
2. Maintain backward compatibility:
   - Same command-line interface
   - Same output format (unless --json specified)
   - Same error codes and messaging

#### Phase 4: Enhance Functionality
1. Add automatic fallback:
   - If preferred tool not available, try alternative
   - Clear messaging about which tool is being used
2. Standardize result formats:
   - Unified JSON structure regardless of backend
   - Consistent field names and data types

### Files to Modify/Create
- `scripts/benchmark.sh` (new) - Shared benchmark library
- `scripts/benchmark_bombardier.sh` (new) - Bombardier adapter
- `scripts/benchmark_ab.sh` (new) - AB adapter
- Modified `scripts/bench.sh` - Thin wrapper
- Modified `scripts/bench-lite.sh` - Thin wrapper

### Estimated Effort: Medium (8-12 hours)

---

## Item 2: Implement dry-run mode and safety checks for destructive operations

### Current State
- Some scripts perform potentially destructive operations without confirmation:
  - `setup-rust.sh`: Removes invalid crate directories (`rm -rf`)
  - `install.sh` family: Creates/overwrites files
  - Environment setup: Modifies `.env` files
- No dry-run capability to preview changes

### Implementation Plan

#### Phase 1: Define Safety Framework
1. Create safety utilities in `scripts/common.sh`:
   - `confirm_action()`: Prompt for user confirmation
   - `safe_remove()`: Wrapper around rm with safety checks
   - `safe_copy()`: Wrapper around cp with backup option
   - `safe_move()`: Wrapper around mv with safety checks
   - `dry_run_mode()`: Check if dry-run is enabled
   - `log_dry_run()`: Log what would be done in dry-run mode

#### Phase 2: Implement Dry-Run Capability
1. Add `--dry-run` flag to all scripts that perform file operations
2. When dry-run mode is active:
   - Log all actions that would be taken
   - Do not actually modify any files
   - Return success exit code
   - Show summary of changes that would be made

#### Phase 3: Apply to Specific Scripts
1. `setup-rust.sh`:
   - Add confirmation before removing crate directories
   - Add dry-run mode for directory creation and file writing
   - Log what would be created vs what exists

2. Environment setup scripts:
   - Add backup of existing .env before modification
   - Confirm before overwriting existing configurations
   - Dry-run mode shows proposed .env diff

3. Installation scripts:
   - Confirm before running system-level commands (where applicable)
   - Dry-run mode shows what would be installed

#### Phase 4: Safety Enhancements
1. Add pre-operation validation:
   - Check available disk space before large operations
   - Verify write permissions before file operations
   - Check for conflicting processes/services

2. Add rollback capability (where feasible):
   - Track changes made during operation
   - Provide `--rollback` option for recent operations
   - Create backup snapshots before destructive operations

### Files to Modify
- `scripts/common.sh` - Add safety utility functions
- `scripts/setup-rust.sh` - Add confirmations and dry-run
- `scripts/setup-env.sh` - Add backup and dry-run
- `scripts/install-deps.sh` - Add dry-run for installations
- Other scripts as needed based on operation risk

### Estimated Effort: Medium (6-10 hours)

---

## Item 3: Add unit test hooks and improve script testability

### Current State
- Scripts are difficult to test in isolation
- Heavy reliance on external systems and side effects
- No clear separation between pure logic and I/O operations
- Difficult to mock dependencies for testing

### Implementation Plan

#### Phase 1: Architect for Testability
1. Apply Separation of Concerns:
   - Divide each script into:
     - Pure functions (logic, no side effects)
     - I/O functions (file operations, system calls)
     - Orchestration (main workflow)
   - Export pure functions for testing when sourced

2. Create Testability Layer:
   - Add `TEST_MODE` environment variable detection
   - When in test mode:
     - Skip actual system modifications
     - Use mock/temporary directories
     - Return simulated results from external calls

#### Phase 2: Implement Mocking Capabilities
1. Create mock utilities in `scripts/common.sh`:
   - `mock_command()`: Replace actual command execution
   - `mock_file_operations()`: Redirect file ops to temp dir
   - `mock_network_calls()`: Simulate HTTP responses
   - `mock_environment()`: Isolate environment variables

2. Add test hooks to key scripts:
   - `setup-rust.sh`: Mock cargo and file system operations
   - `setup-env.sh`: Mock .env file operations
   - `install-deps.sh`: Mock package manager calls
   - Benchmark scripts: Mock bombardier/ab output

#### Phase 3: Create Test Framework
1. Develop lightweight test framework:
   - `scripts/test/` directory with test helpers
   - Test assertion functions (assert_equal, assert_contains, etc.)
   - Test lifecycle functions (setup, teardown)
   - Mock database for external command results

2. Create example tests:
   - Test configuration parsing logic
   - Test argument validation functions
   - Test error handling paths
   - Test edge cases in utility functions

#### Phase 4: Improve Existing Script Structure
1. Make functions more pure where possible:
   - Extract logic functions that accept parameters and return values
   - Reduce reliance on global state
   - Make functions idempotent where appropriate

2. Add test annotations:
   - Document testability of each function
   - Mark functions that are unit-testable
   - Provide examples of how to test complex functions

### Files to Modify/Create
- `scripts/common.sh` - Add testability utilities
- `scripts/test/` (new directory) - Test framework and helpers
- `scripts/setup-rust.sh` - Refactor for testability
- `scripts/setup-env.sh` - Refactor for testability
- `scripts/install-deps.sh` - Refactor for testability
- `scripts/benchmark.sh` - Refactor for testability
- `scripts/test_example.sh` (new) - Example test file

### Estimated Effort: High (15-20 hours)

---

## Implementation Priority and Dependencies

### Recommended Order:
1. **Item 1 (Consolidate benchmarks)** - Foundation for consistent benchmarking
   - Enables better performance testing of other improvements
   - Relatively contained scope
   - Builds on existing config.sh and common.sh

2. **Item 2 (Dry-run and safety)** - Risk reduction
   - Makes existing system safer to use
   - Provides immediate value to users
   - Builds on the modular structure already created

3. **Item 3 (Testability)** - Long-term maintainability
   - Enables safer future changes
   - Requires most refactoring effort
   - Benefits from having stable interfaces first

### Dependencies:
- Item 1 can be implemented independently
- Item 2 benefits from Item 1's pattern of shared libraries
- Item 3 benefits from both previous items' improvements to modularity

### Risk Assessment:
- **Low Risk**: Item 1 (mainly refactoring, clear boundaries)
- **Medium Risk**: Item 2 (changes to destructive operations, but with safety)
- **Medium Risk**: Item 3 (refactoring for testability, but preserves behavior)

---

## Success Metrics

### For Item 1 (Benchmark Consolidation):
- [ ] 50% reduction in duplicated code between bench.sh and bench-lite.sh
- [ ] Both scripts produce identical output format in non-JSON mode
- [ ] JSON output includes all relevant benchmark metrics
- [ ] Automatic fallback works when preferred tool unavailable
- [ ] All existing functionality preserved

### For Item 2 (Dry-run and Safety):
- [ ] All scripts with file modification capabilities support --dry-run
- [ ] Dry-run mode accurately predicts changes without making them
- [ ] Destructive operations require confirmation in interactive mode
- [ ] Safety checks prevent common error conditions
- [ ] Backup/restore capability for critical files

### For Item 3 (Testability):
- [ ] Core logic functions in each script are unit-testable
- [ ] Test framework can run without modifying system state
- [ ] At least 80% of logic functions have test coverage
- [ ] Mocking capabilities work for external dependencies
- [ ] Tests can be run in CI environment without special privileges

---

## Estimated Total Effort: 29-42 hours

This represents approximately 1-1.5 weeks of focused development effort, depending on complexity encountered during implementation.

--- 
*This plan was created as part of the FullStackHex scripts improvement initiative. Implementation should follow the existing code style and conventions established in the improved scripts.*