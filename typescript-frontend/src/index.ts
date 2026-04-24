// TypeScript Frontend using Bun
// This service integrates with:
// - Rust backend: gRPC/HTTP API for core business logic
// - Python services: REST API for data processing and analytics

const apiVersion = "1.0.0";

console.log(`🚀 Frontend service started (v${apiVersion})`);
console.log("📡 API Integration Points:");
console.log("   - Rust Backend: http://localhost:3000 (gRPC/HTTP)");
console.log("   - Python Services: http://localhost:5000 (REST)");

// Example environment variable loading
const backendUrl = process.env.RUST_BACKEND_URL || "http://localhost:3000";
const pythonServiceUrl = process.env.PYTHON_SERVICE_URL || "http://localhost:5000";

console.log(`\n✅ Connected to:`);
console.log(`   Rust: ${backendUrl}`);
console.log(`   Python: ${pythonServiceUrl}`);
