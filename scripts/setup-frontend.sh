#!/bin/bash
# FullStackHex Frontend Setup
# Scaffold the Astro frontend application

# Source common functions and configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"
source "$SCRIPT_DIR/config.sh"

log_info "Scaffolding Astro frontend..."

if [ -d "frontend" ]; then
    log_success "Frontend directory already exists"
    pushd frontend > /dev/null

    # Install dependencies if node_modules doesn't exist
    if [ ! -d "node_modules" ]; then
        log_info "Installing frontend dependencies..."
        bun install
    fi

    # Add Tailwind v4 and Node SSR adapter if not present
    if ! grep -q "@tailwindcss/vite" package.json 2>/dev/null; then
        log_info "Adding Tailwind v4 and Node SSR adapter..."
        bun add @tailwindcss/vite tailwindcss @astrojs/node
    fi

    # Create API health proxy route if it doesn't exist
    mkdir -p src/pages/api
    if [ ! -f src/pages/api/health.ts ]; then
        log_info "Creating API health proxy route..."
        cat > src/pages/api/health.ts << 'EOF'
export async function GET() {
    const response = await fetch(`${import.meta.env.VITE_RUST_BACKEND_URL}/health`);
    const body = await response.json();

    return new Response(JSON.stringify(body), {
        headers: { 'Content-Type': 'application/json' },
    });
}
EOF
    fi

    popd > /dev/null
else
    log_info "Creating Astro app..."
    bun create astro@latest frontend -- --template minimal --no-install --no-git --yes

    pushd frontend > /dev/null

    log_info "Installing Tailwind v4 and Node SSR adapter..."
    bun add @tailwindcss/vite tailwindcss @astrojs/node

    log_info "Installing TypeScript runtime types and check tooling for Bun/Node..."
    bun add --dev @astrojs/check typescript @types/node bun-types

    log_info "Installing remaining dependencies..."
    bun install

    # Inject typecheck and lint scripts (astro check) into package.json
    log_info "Injecting typecheck and lint scripts..."
    bun -e "
    const fs = require('fs');
    const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
    pkg.scripts = pkg.scripts || {};
    pkg.scripts.typecheck = pkg.scripts.typecheck || 'astro check';
    pkg.scripts.lint = pkg.scripts.lint || 'astro check';
    fs.writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
    "

    # Write astro.config.mjs with SSR output and Tailwind vite plugin
    log_info "Writing astro.config.mjs..."
    cat > astro.config.mjs << 'EOF'
// @ts-check
import { defineConfig } from 'astro/config';
import node from '@astrojs/node';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  output: 'server',
  adapter: node({ mode: 'standalone' }),
  vite: {
    plugins: [tailwindcss()]
  }
});
EOF

    # Ensure Bun/Node globals are typed for generated test files
    log_info "Setting up TypeScript configuration..."
    cat > tsconfig.json << 'EOF'
{
    "extends": "astro/tsconfigs/strict",
    "compilerOptions": {
        "types": ["node", "bun-types"]
    },
    "include": [".astro/types.d.ts", "**/*"],
    "exclude": ["dist"]
}
EOF

    # Create API health proxy route
    log_info "Creating API health proxy route..."
    mkdir -p src/pages/api
    cat > src/pages/api/health.ts << 'EOF'
export async function GET() {
    const response = await fetch(`${import.meta.env.VITE_RUST_BACKEND_URL}/health`);
    const body = await response.json();

    return new Response(JSON.stringify(body), {
        headers: { 'Content-Type': 'application/json' },
    });
}
EOF

    log_success "Astro frontend ready (port 4321)"
    popd > /dev/null
fi

log_success "Frontend setup completed"
exit 0