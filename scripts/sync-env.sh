#!/usr/bin/env bash
# sync-env.sh — Compare .env against .env.example and report missing keys.
# With --apply, append missing keys (commented out) to .env.
# Works on bash 4+, zsh.

set -euo pipefail

ENV_FILE=".env"
EXAMPLE_FILE=".env.example"
APPLY=false

if [ "${1:-}" = "--apply" ]; then
    APPLY=true
fi

if [ ! -f "$ENV_FILE" ]; then
    echo "ERROR: $ENV_FILE not found. Run: cp .env.example .env"
    exit 1
fi

if [ ! -f "$EXAMPLE_FILE" ]; then
    echo "ERROR: $EXAMPLE_FILE not found."
    exit 1
fi

# Extract keys from .env.example
EXAMPLE_KEYS=$(grep -E '^(export[[:space:]]+)?[A-Z_][A-Z0-9_]*[[:space:]]*=' "$EXAMPLE_FILE" \
    | sed 's/^export[[:space:]]*//; s/[[:space:]]*=.*//' \
    | sort -u || true)

# Extract keys from .env (active, non-commented)
ENV_KEYS=$(grep -E '^[A-Z_][A-Z0-9_]*[[:space:]]*=' "$ENV_FILE" \
    | sed 's/[[:space:]]*=.*//' \
    | sort -u || true)

MISSING_COUNT=0
ADDED_COUNT=0

while IFS= read -r key; do
    [ -z "$key" ] && continue
    if ! echo "$ENV_KEYS" | grep -qxF "$key"; then
        # Get example value for display
        example_val=$(grep -E "^(export[[:space:]]+)?${key}[[:space:]]*=" "$EXAMPLE_FILE" \
            | head -1 | sed 's/^export[[:space:]]*//; s/^[^=]*=[[:space:]]*//')
        echo "  MISSING: ${key} (example: ${key}=${example_val})"
        MISSING_COUNT=$((MISSING_COUNT + 1))
        if $APPLY; then
            echo "# ${key}=${example_val}" >> "$ENV_FILE"
            ADDED_COUNT=$((ADDED_COUNT + 1))
        fi
    fi
done <<< "$EXAMPLE_KEYS"

if [ "$MISSING_COUNT" -eq 0 ]; then
    echo ".env is in sync with .env.example."
    exit 0
fi

echo ""
echo "$MISSING_COUNT key(s) in .env.example missing from .env."

if $APPLY; then
    echo "Appended $ADDED_COUNT missing key(s) to .env (commented out — uncomment and set values)."
fi

exit 1
