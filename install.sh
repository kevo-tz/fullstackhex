#!/usr/bin/env bash
set -euo pipefail

if [ -t 0 ]; then
  READ_INPUT=/dev/stdin
elif ( true < /dev/tty ) 2>/dev/null; then
  READ_INPUT=/dev/tty
else
  READ_INPUT=/dev/stdin
  NONINTERACTIVE=true
fi

PROJECT_NAME=""
DRY_RUN=false
SKIP_DEPS=false
SKIP_GIT=false
SKIP_VERIFY=false
PHASE=""
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_SOURCE="$SCRIPT_DIR"
TMP_REPO=""

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
    "$@"
  fi
}

run_in() {
  local dir="$1"
  shift
  if [ "$DRY_RUN" = true ]; then
    echo -e "${YELLOW}[DRY-RUN]${NC} (cd \"$dir\" && $*)"
  else
    (cd "$dir" && "$@")
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
    version_str=$(bash -c "$get_version_cmd" 2>/dev/null || true)
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
  if [ -n "$TMP_REPO" ] && [ -d "$TMP_REPO" ]; then
    rm -rf "$TMP_REPO"
  fi
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
Usage:
  curl -fsSL https://raw.githubusercontent.com/kevo-tz/fullstackhex/main/install.sh | bash
  ./install.sh [project-name] [options]

If project-name is omitted, you'll be prompted for it interactively.

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
    if [ "${NONINTERACTIVE:-}" = true ]; then
      error "Project name is required when running non-interactively."
      echo "  Usage: curl -fsSL https://raw.githubusercontent.com/kevo-tz/fullstackhex/main/install.sh | bash -s -- <project-name>"
      echo "  Or:    ./install.sh <project-name>"
      exit 1
    fi
    log "No project name provided."
    read -r -p "Enter project name: " PROJECT_NAME < "$READ_INPUT" || true
  fi
  if [ -z "$PROJECT_NAME" ]; then
    error "Project name is required."
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
  if [ ! -f "$REPO_SOURCE/backend/Cargo.toml" ]; then
    TMP_REPO=$(mktemp -d)
    log "Downloading template from GitHub..."
    curl -fsSL "https://github.com/kevo-tz/fullstackhex/archive/main.tar.gz" | tar xz -C "$TMP_REPO" --strip-components=1
    REPO_SOURCE="$TMP_REPO"
    ok "Template downloaded."
  fi

  log "Creating $PROJECT_NAME..."
  run mkdir -p "$PROJECT_NAME"

  local rsync_excludes
  rsync_excludes="--exclude=.git/ --exclude=target/ --exclude=node_modules/ --exclude=.venv/ --exclude=dist/ --exclude=.gitignore --exclude=.dockerignore"

  log "Copying backend/..."
  run rsync -a $rsync_excludes "$REPO_SOURCE/backend/" "$PROJECT_NAME/backend/"
  log "Copying compose/..."
  run rsync -a $rsync_excludes "$REPO_SOURCE/compose/" "$PROJECT_NAME/compose/"
  log "Copying frontend/..."
  run rsync -a $rsync_excludes "$REPO_SOURCE/frontend/" "$PROJECT_NAME/frontend/"
  log "Copying py-api/..."
  run rsync -a $rsync_excludes "$REPO_SOURCE/py-api/" "$PROJECT_NAME/py-api/"
  log "Copying scripts/..."
  run rsync -a $rsync_excludes "$REPO_SOURCE/scripts/" "$PROJECT_NAME/scripts/"

  log "Copying root files..."
  for f in .env.example .gitignore .dockerignore Makefile LICENSE; do
    if [ -f "$REPO_SOURCE/$f" ]; then
      run cp "$REPO_SOURCE/$f" "$PROJECT_NAME/$f"
    fi
  done

  log "Resetting VERSION to 0.1.0.0..."
  printf '0.1.0.0\n' > "$PROJECT_NAME/VERSION"

  ok "Scaffold complete."
}

# ── Phase 3 — Configure ──

configure() {
  log "Generating .env..."
  run_in "$PROJECT_NAME" mv .env.example .env

  log "Configuring backend/Cargo.toml (repository URL)..."
  read -r -p "GitHub username (for repository URL) [kevo-tz]: " GITHUB_USER < "$READ_INPUT" || true
  GITHUB_USER=${GITHUB_USER:-kevo-tz}
  run_in "$PROJECT_NAME" sed -i 's|https://github.com/kevo-tz/fullstackhex|https://github.com/${GITHUB_USER}/${PROJECT_NAME}|' backend/Cargo.toml

  log "Configuring frontend/package.json (name)..."
  run_in "$PROJECT_NAME" sed -i 's|"name": "frontend"|"name": "'"$PROJECT_NAME"'"|' frontend/package.json

  log "Configuring py-api/pyproject.toml (name)..."
  run_in "$PROJECT_NAME" sed -i 's|name = "py-api"|name = "'"$PROJECT_NAME"'"|' py-api/pyproject.toml

  log "Configuring compose files (container names, network names)..."
  for f in prod.yml dev.yml monitor.yml; do
    run_in "$PROJECT_NAME" sed -i 's|fullstackhex_|${PROJECT_NAME}_|g' compose/$f
    run_in "$PROJECT_NAME" sed -i 's|fullstackhex-network|${PROJECT_NAME}-network|g' compose/$f
  done

  log "Configuring scripts/config.sh (project paths)..."
  run_in "$PROJECT_NAME" sed -i 's|/tmp/fullstackhex-dev|/tmp/${PROJECT_NAME}-dev|g' scripts/config.sh
  run_in "$PROJECT_NAME" sed -i 's|/tmp/fullstackhex-python.sock|/tmp/${PROJECT_NAME}-python.sock|g' scripts/config.sh
  run_in "$PROJECT_NAME" sed -i 's|-p fullstackhex-monitor|-p ${PROJECT_NAME}-monitor|g' scripts/config.sh

  log "Configuring .env (project-specific defaults)..."
  run_in "$PROJECT_NAME" sed -i 's|JWT_ISSUER=fullstackhex|JWT_ISSUER=${PROJECT_NAME}|' .env
  run_in "$PROJECT_NAME" sed -i 's|RUSTFS_BUCKET=fullstackhex|RUSTFS_BUCKET=${PROJECT_NAME}|' .env
  run_in "$PROJECT_NAME" sed -i 's|/opt/fullstackhex|/opt/${PROJECT_NAME}|' .env
  run_in "$PROJECT_NAME" sed -i 's|REDIS_KEY_PREFIX=fullstackhex|REDIS_KEY_PREFIX=${PROJECT_NAME}|' .env

  # README.md is no longer copied — skip configuration

  ok "Configuration complete."
}

# ── Phase 4 — Install ──

install_deps() {
  if [ "$SKIP_DEPS" = true ]; then
    warn "Skipping dependency installation (--skip-deps)."
    return
  fi

  log "Installing Python 3.14 via uv (managed)..."
  run_in "$PROJECT_NAME/py-api" uv python install 3.14
  ok "Python 3.14 installed."

  log "Installing Python dependencies (uv sync)..."
  run_in "$PROJECT_NAME/py-api" uv sync --python 3.14
  ok "Python dependencies installed."

  log "Installing frontend dependencies (bun install)..."
  run_in "$PROJECT_NAME/frontend" bun install
  ok "Frontend dependencies installed."
}

# ── Phase 5 — Verify ──

verify() {
  if [ "$SKIP_VERIFY" = true ]; then
    warn "Skipping verification (--skip-verify)."
    return
  fi

  log "Verifying backend (cargo check)..."
  run_in "$PROJECT_NAME/backend" cargo check
  ok "Backend compiles."

  log "Verifying frontend (bun run typecheck)..."
  run_in "$PROJECT_NAME/frontend" bun run typecheck
  ok "Frontend typechecks."

  log "Running py-api tests (optional)..."
  if run_in "$PROJECT_NAME/py-api" uv run pytest; then
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
  run_in "$PROJECT_NAME" git init
  run_in "$PROJECT_NAME" git add .
  run_in "$PROJECT_NAME" git commit -m 'chore: scaffold from fullstackhex template'
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
  echo "  docker compose -f compose/dev.yml up -d   # Start Docker services"
  echo "  make dev                                  # Start everything (infra + apps)"
  echo
  echo "  Or run components individually:"
  echo "  cd $PROJECT_NAME/backend && cargo run -p api"
  echo "  cd $PROJECT_NAME/frontend && bun run dev"
  echo "  cd $PROJECT_NAME/py-api && uv run uvicorn app.main:app --uds /tmp/$PROJECT_NAME-python.sock"
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
