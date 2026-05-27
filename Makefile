.PHONY: dev watch down test lint check logs bench status clean help

.DEFAULT_GOAL := help

help:
	@echo "Usage: make [dev|watch|down|test|lint|check|logs|bench|status|clean]"
	@echo ""
	@echo "  dev     Start full stack (infra + python + rust + frontend)"
	@echo "  watch   Start full stack with Rust hot reload (cargo watch)"
	@echo "  down    Stop all services"
	@echo "  test    Run all test suites (matches CI test commands)"
	@echo "  lint    Run all lint/format/typecheck (matches CI lint steps)"
	@echo "  check   Full CI preflight: lint + test (what CI gates on)"
	@echo "  logs    Follow all stack logs"
	@echo "  bench   Run performance benchmarks"
	@echo "  status  Show service status (PID, port, health)"
	@echo "  clean   Reset to fresh state (removes volumes)"
	@echo ""
	@echo "Quick start: make dev"
	@echo "          -> http://localhost:4321"
	@echo ""
	@echo "Before pushing: make check"

dev:
	@./scripts/dev.sh

watch:
	@./scripts/dev.sh --watch

down:
	@./scripts/down.sh

test:
	@./scripts/test.sh

lint:
	@./scripts/lint.sh

check: lint test
	@echo ""
	@echo "=== CI preflight complete ==="

logs:
	@./scripts/logs.sh

bench:
	@./scripts/bench.sh

status:
	@./scripts/status.sh

clean:
	@./scripts/clean.sh
