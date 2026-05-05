#!/usr/bin/env bash
# validate-env.sh — Validate .env against .env.example
# Catches: missing required vars, CHANGE_ME placeholders, shell syntax errors.
# Works on bash 4+, zsh.

ENV_FILE="${1:-.env}"
EXAMPLE_FILE="${2:-.env.example}"
MISSING=0

# 1. Guard: .env must exist
if [ ! -f "$ENV_FILE" ]; then
    echo "ERROR: $ENV_FILE not found."
    echo "  Run: cp .env.example .env"
    echo "  Then edit .env and replace CHANGE_ME values."
    exit 1
fi

# 2. Guard: .env.example must exist
if [ ! -f "$EXAMPLE_FILE" ]; then
    echo "ERROR: $EXAMPLE_FILE not found. Cannot validate required keys."
    exit 1
fi

# 3. Source .env with error detection
# Capture stderr so we show the actual error instead of hiding it.
SOURCE_ERR=$(mktemp)
set -a
if ! source "$ENV_FILE" 2>"$SOURCE_ERR"; then
    echo "ERROR: .env has syntax errors:"
    cat "$SOURCE_ERR"
    rm -f "$SOURCE_ERR"
    echo ""
    echo "Common fix: quote values with spaces."
    echo "  Wrong: REDIS_SAVE=900 1 300 10 60 10000"
    echo "  Right: REDIS_SAVE=\"900 1 300 10 60 10000\""
    exit 1
fi
rm -f "$SOURCE_ERR"
set +a

# 4. Extract keys from .env.example
# Matches KEY=VALUE and export KEY=VALUE. Keys may contain digits.
# Skips comments and blank lines. Guards against grep finding no matches.
REQUIRED_KEYS=$(grep -E '^(export[[:space:]]+)?[A-Z_][A-Z0-9_]*[[:space:]]*=' "$EXAMPLE_FILE" \
    | sed 's/^export[[:space:]]*//; s/[[:space:]]*=.*//' \
    | sort -u || true)

if [ -z "$REQUIRED_KEYS" ]; then
    echo "WARNING: No parseable keys found in $EXAMPLE_FILE. Nothing to validate."
    exit 0
fi

# 5. Check each key
for key in $REQUIRED_KEYS; do
    val="${!key:-}"
    if [ -z "$val" ]; then
        echo "  MISSING: $key — add to .env (see .env.example for default)"
        MISSING=1
    elif [ "$val" = "CHANGE_ME" ]; then
        echo "  PLACEHOLDER: $key=CHANGE_ME — replace with a real value"
        MISSING=1
    fi
done

if [ "$MISSING" -eq 0 ]; then
    echo ".env is valid."
    exit 0
else
    echo ""
    echo "Fix the issues above in .env, then re-run."
    exit 1
fi
