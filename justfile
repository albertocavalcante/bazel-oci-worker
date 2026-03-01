# bazel-oci-worker development commands

# Default recipe
default:
    @just --list

# Build all targets
build:
    cargo build --workspace

# Run all tests
test:
    cargo test --workspace

# Format Rust code
fmt:
    cargo fmt

# Run clippy
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Run local dev server
dev:
    wrangler dev

# Deploy to Cloudflare
deploy:
    wrangler deploy
