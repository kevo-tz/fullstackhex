#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$SCRIPT_DIR/.."
CERTS_DIR="$REPO_ROOT/compose/nginx/certs"

usage() {
  echo "Usage: $0 [--dev|--prod] [--domain DOMAIN]"
  echo "  --dev         Generate self-signed certificates for development"
  echo "  --prod        Set up Let's Encrypt certificates for production"
  echo "  --domain      Domain name (required for --prod)"
  exit 1
}

MODE=""
DOMAIN=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dev) MODE="dev"; shift ;;
    --prod) MODE="prod"; shift ;;
    --domain) DOMAIN="$2"; shift 2 ;;
    *) usage ;;
  esac
done

mkdir -p "$CERTS_DIR"

if [ "$MODE" = "dev" ]; then
  if [ -f "$CERTS_DIR/fullchain.pem" ] && [ -f "$CERTS_DIR/privkey.pem" ]; then
    echo "Self-signed certificates already exist in $CERTS_DIR"
    exit 0
  fi
  openssl req -x509 -nodes -days 3650 -newkey rsa:2048 \
    -keyout "$CERTS_DIR/privkey.pem" \
    -out "$CERTS_DIR/fullchain.pem" \
    -subj "/C=US/ST=Development/L=Local/O=FullStackHex/CN=localhost" 2>/dev/null
  chmod 644 "$CERTS_DIR/fullchain.pem"
  chmod 600 "$CERTS_DIR/privkey.pem"
  echo "Self-signed certificates generated in $CERTS_DIR"
elif [ "$MODE" = "prod" ]; then
  if [ -z "$DOMAIN" ]; then
    echo "Error: --domain is required for --prod mode"
    exit 1
  fi
  if ! command -v certbot &>/dev/null; then
    echo "Error: certbot not found. Install it first:"
    echo "  sudo apt-get install certbot"
    exit 1
  fi
  DEPLOY_HOOK="$REPO_ROOT/compose/nginx/certbot-deploy-hook.sh"
  cat > "$DEPLOY_HOOK" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail
cp -L /etc/letsencrypt/live/$RENEWED_DOMAIN/fullchain.pem /etc/nginx/certs/
cp -L /etc/letsencrypt/live/$RENEWED_DOMAIN/privkey.pem /etc/nginx/certs/
chmod 644 /etc/nginx/certs/fullchain.pem
chmod 600 /etc/nginx/certs/privkey.pem
nginx -s reload 2>/dev/null || true
HOOK
  chmod +x "$DEPLOY_HOOK"
  certbot certonly --standalone -d "$DOMAIN" --deploy-hook "$DEPLOY_HOOK"
  echo "Let's Encrypt certificates deployed for $DOMAIN"
else
  usage
fi
