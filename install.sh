#!/bin/bash
set -euo pipefail

PROJECT_NAME=""
DRY_RUN=false
SKIP_DEPS=false
SKIP_GIT=false
SKIP_VERIFY=false
PHASE=""
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log()     { echo -e "${CYAN}[*]${NC} $*"; }
ok()      { echo -e "${GREEN}[✓]${NC} $*"; }
warn()    { echo -e "${YELLOW}[!]${NC} $*"; }
error()   { echo -e "${RED}[✗]${NC} $*"; }
header()  { echo -e "\n${BOLD}── $* ──${NC}\n"; }

run() {
  if [ "$DRY_RUN" = true ]; then
    echo -e "${YELLOW}[DRY-RUN]${NC} $*"
  else
    eval "$*"
  fi
}

run_in() {
  local dir="$1"
  shift
  if [ "$DRY_RUN" = true ]; then
    echo -e "${YELLOW}[DRY-RUN]${NC} (cd \"$dir\" && $*)"
  else
    (cd "$dir" && eval "$*")
  fi
}

version_ge() {
  local v1=$1 v2=$2
  local IFS=.
  set -- $v1
  local a1=${1:-0} a2=${2:-0} a3=${3:-0}
  set -- $v2
  local b1=${1:-0} b2=${2:-0} b3=${3:-0}
  [ "$a1" -gt "$b1" ] 2>/dev/null && return 0
  [ "$a1" -lt "$b1" ] 2>/dev/null && return 1
  [ "$a2" -gt "$b2" ] 2>/dev/null && return 0
  [ "$a2" -lt "$b2" ] 2>/dev/null && return 1
  [ "$a3" -ge "$b3" ] 2>/dev/null && return 0
  return 1
}

check_tool() {
  local name=$1 cmd=$2 min_version=$3 get_version_cmd=$4 install_cmd=$5
  if ! command -v "$cmd" &>/dev/null; then
    error "$name is not installed."
    echo "  Install: $install_cmd"
    return 1
  fi
  if [ -n "$min_version" ]; then
    local version_str
    version_str=$(eval "$get_version_cmd" 2>/dev/null || true)
    if [ -n "$version_str" ]; then
      if ! version_ge "$version_str" "$min_version"; then
        error "$name $version_str is too old. Minimum: $min_version"
        echo "  Update: $install_cmd"
        return 1
      fi
      ok "$name $version_str"
    else
      ok "$name"
    fi
  else
    ok "$name"
  fi
}

cleanup() {
  local exit_code=$?
  if [ $exit_code -eq 0 ]; then return; fi
  case "$PHASE" in
    validate|scaffold)
      echo
      warn "Error during $PHASE phase. Cleaning up..."
      if [ -n "$PROJECT_NAME" ] && [ -d "$PROJECT_NAME" ]; then
        rm -rf "$PROJECT_NAME"
        log "Removed $PROJECT_NAME"
      fi
      ;;
    *)
      echo
      warn "Error during $PHASE phase. Partial scaffold left at $PROJECT_NAME — remove it and retry."
      ;;
  esac
}

print_usage() {
  cat <<'EOF'
Usage: ./install.sh <project-name> [options]

Options:
  --dry-run       Preview actions without executing
  --skip-deps     Skip dependency installation (uv sync, bun install)
  --skip-git      Skip git init and initial commit
  --skip-verify   Skip proof-of-concept build checks
EOF
}

# ── Phase 1 — Validate ──

validate() {
  if [ -z "$PROJECT_NAME" ]; then
    error "Missing project name."
    print_usage
    exit 1
  fi
  if [ -d "$PROJECT_NAME" ]; then
    error "Target directory '$PROJECT_NAME' already exists."
    exit 1
  fi
  if ! echo "$PROJECT_NAME" | grep -q '^[a-zA-Z0-9][a-zA-Z0-9_-]*$'; then
    error "Project name must start with an alphanumeric character and contain only alphanumeric chars, hyphens, and underscores."
    exit 1
  fi

  local fail=false

  check_tool "Cargo" "cargo" "1.95" \
    "cargo --version 2>/dev/null | awk '{print \$2}'" \
    "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" || fail=true

  check_tool "Bun" "bun" "1.0" \
    "bun --version 2>/dev/null" \
    "curl -fsSL https://bun.sh/install | bash" || fail=true

  check_tool "uv" "uv" "0.6" \
    "uv --version 2>/dev/null | awk '{print \$2}'" \
    "curl -LsSf https://astral.sh/uv/install.sh | sh" || fail=true

  check_tool "Docker" "docker" "" \
    "" \
    "See: https://docs.docker.com/engine/install/" || fail=true

  if [ "$fail" = true ]; then
    exit 1
  fi
  ok "All tools validated."
}

# ── Phase 2 — Scaffold ──

scaffold() {
  log "Creating $PROJECT_NAME..."
  run "mkdir -p \"$PROJECT_NAME\""

  local rsync_excludes
  rsync_excludes="--exclude=.git/ --exclude=target/ --exclude=node_modules/ --exclude=.venv/ --exclude='*.lock' --exclude=dist/ --exclude=.gitignore --exclude=.dockerignore"

  log "Copying backend/..."
  run "rsync -a $rsync_excludes \"$SCRIPT_DIR/backend/\" \"$PROJECT_NAME/backend/\""
  log "Copying compose/..."
  run "rsync -a $rsync_excludes \"$SCRIPT_DIR/compose/\" \"$PROJECT_NAME/compose/\""
  log "Copying frontend/..."
  run "rsync -a $rsync_excludes \"$SCRIPT_DIR/frontend/\" \"$PROJECT_NAME/frontend/\""
  log "Copying py-api/..."
  run "rsync -a $rsync_excludes \"$SCRIPT_DIR/py-api/\" \"$PROJECT_NAME/py-api/\""
  log "Copying scripts/..."
  run "rsync -a $rsync_excludes \"$SCRIPT_DIR/scripts/\" \"$PROJECT_NAME/scripts/\""

  log "Copying root files..."
  for f in .env.example .gitignore .dockerignore Makefile AGENTS.md CLAUDE.md CONTRIBUTING.md CODE_OF_CONDUCT.md LICENSE; do
    if [ -f "$SCRIPT_DIR/$f" ]; then
      run "cp \"$SCRIPT_DIR/$f\" \"$PROJECT_NAME/$f\""
    fi
  done

  ok "Scaffold complete."
}

# ── Phase 3 — Configure ──

configure() {
  log "Generating .env..."
  run_in "$PROJECT_NAME" "cp .env.example .env"
  run_in "$PROJECT_NAME" "sed -i '1s|^|# Application\\nAPP_NAME=$PROJECT_NAME\\n\\n|' .env"

  log "Configuring backend/Cargo.toml (repository URL)..."
  run_in "$PROJECT_NAME" "sed -i 's|https://github.com/kevo-tz/fullstackhex|https://github.com/kevo-tz/$PROJECT_NAME|' backend/Cargo.toml"

  log "Configuring frontend/package.json (name)..."
  run_in "$PROJECT_NAME" "sed -i 's|\"name\": \"frontend\"|\"name\": \"$PROJECT_NAME\"|' frontend/package.json"

  log "Configuring py-api/pyproject.toml (name)..."
  run_in "$PROJECT_NAME" "sed -i 's|name = \"py-api\"|name = \"$PROJECT_NAME\"|' py-api/pyproject.toml"

  log "Configuring compose files (container names, network names)..."
  for f in prod.yml dev.yml monitor.yml; do
    run_in "$PROJECT_NAME" "sed -i 's|fullstackhex_|${PROJECT_NAME}_|g' compose/$f"
    run_in "$PROJECT_NAME" "sed -i 's|fullstackhex-network|${PROJECT_NAME}-network|g' compose/$f"
  done

  log "Configuring Makefile (APP_NAME)..."
  run_in "$PROJECT_NAME" "sed -i 's|^APP_NAME ?= fullstackhex|APP_NAME ?= $PROJECT_NAME|' Makefile"

  ok "Configuration complete."
}

# ── Phase 4 — Install ──

install_deps() {
  if [ "$SKIP_DEPS" = true ]; then
    warn "Skipping dependency installation (--skip-deps)."
    return
  fi

  log "Installing Python 3.14 via uv (managed)..."
  run_in "$PROJECT_NAME/py-api" "uv python install 3.14"
  ok "Python 3.14 installed."

  log "Installing Python dependencies (uv sync)..."
  run_in "$PROJECT_NAME/py-api" "uv sync --python 3.14"
  ok "Python dependencies installed."

  log "Installing frontend dependencies (bun install)..."
  run_in "$PROJECT_NAME/frontend" "bun install"
  ok "Frontend dependencies installed."
}

# ── Phase 5 — Verify ──

verify() {
  if [ "$SKIP_VERIFY" = true ]; then
    warn "Skipping verification (--skip-verify)."
    return
  fi

  log "Verifying backend (cargo check)..."
  run_in "$PROJECT_NAME/backend" "cargo check"
  ok "Backend compiles."

  log "Verifying frontend (bun run typecheck)..."
  run_in "$PROJECT_NAME/frontend" "bun run typecheck"
  ok "Frontend typechecks."

  log "Running py-api tests (optional)..."
  if run_in "$PROJECT_NAME/py-api" "uv run pytest"; then
    ok "py-api tests pass."
  else
    warn "py-api tests had issues (non-fatal)."
  fi
}

# ── Phase 6 — Git ──

init_git() {
  if [ "$SKIP_GIT" = true ]; then
    warn "Skipping git init (--skip-git)."
    return
  fi

  log "Initializing git repository..."
  run_in "$PROJECT_NAME" "git init"
  run_in "$PROJECT_NAME" "git add ."
  run_in "$PROJECT_NAME" "git commit -m 'chore: scaffold from fullstackhex template'"
  ok "Git repository initialized."
}

print_next_steps() {
  echo
  echo -e "${BOLD}── Project Scaffolded: $PROJECT_NAME ──${NC}"
  echo
  if command -v tree &>/dev/null; then
    (cd "$PROJECT_NAME" && tree -L 2 --dirsfirst -I 'node_modules|target|.venv|.git' 2>/dev/null) || true
  else
    (cd "$PROJECT_NAME" && find . -maxdepth 2 -not -path './.git/*' -not -path '*/node_modules/*' -not -path '*/target/*' -not -path '*/.venv/*' | sort)
  fi
  echo
  echo -e "${BOLD}Next steps:${NC}"
  echo "  cd $PROJECT_NAME"
  echo "  make up          # Start Docker services (PostgreSQL, Redis, RustFS)"
  echo "  make dev         # Start everything (infra + backend + frontend)"
  echo
  echo "  Or run components individually:"
  echo "  cd $PROJECT_NAME/backend && cargo run -p api"
  echo "  cd $PROJECT_NAME/frontend && bun run dev"
  echo "  cd $PROJECT_NAME/py-api && uv run uvicorn main:app --uds /tmp/$PROJECT_NAME-python.sock"
  echo
  echo "  See docs/ for detailed documentation."
  echo
}

# ── Main ──

trap cleanup EXIT

while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=true; shift ;;
    --skip-deps) SKIP_DEPS=true; shift ;;
    --skip-git) SKIP_GIT=true; shift ;;
    --skip-verify) SKIP_VERIFY=true; shift ;;
    --help|-h) print_usage; exit 0 ;;
    -*)
      error "Unknown option: $1"
      print_usage
      exit 1
      ;;
    *)
      if [ -z "$PROJECT_NAME" ]; then
        PROJECT_NAME="$1"
      else
        error "Unexpected argument: $1"
        print_usage
        exit 1
      fi
      shift
      ;;
  esac
done

header "Phase 1/6 — Validate"
PHASE="validate"
validate
PHASE="scaffold"

header "Phase 2/6 — Scaffold"
scaffold
PHASE="configure"

header "Phase 3/6 — Configure"
configure
PHASE="install"

header "Phase 4/6 — Install"
install_deps
PHASE="verify"

header "Phase 5/6 — Verify"
verify
PHASE="git"

header "Phase 6/6 — Git"
init_git
PHASE="done"

print_next_steps
