# bazel-oci-worker

A self-hosted [Bazel Central Registry](https://bazel.build/external/registry) (BCR) backed by an OCI Distribution registry, deployed as a single Cloudflare Worker with R2 storage.

Modules are stored as standard OCI artifacts. Bazel clients see a BCR-compatible HTTP API, while container tooling (ORAS, crane, skopeo) can push and inspect modules through the OCI Distribution v2 API. One binary, one R2 bucket, no database.

## Overview

The project is a Rust workspace compiled to WebAssembly. Three crates separate concerns:

```
crates/
├── bazel-oci-types      Pure types: OCI manifests, BCR structs, digests, media types
├── bazel-oci-registry   Framework-agnostic protocol handlers (OCI + BCR)
└── bazel-oci-worker     Cloudflare Worker entry point, axum routing, R2 storage
```

The registry crate is deliberately decoupled from any HTTP framework or storage backend. It operates on a `Storage` trait, making it testable with an in-memory implementation and deployable against any object store.

### How modules are stored

Each Bazel module version is an OCI manifest with typed layers:

| Layer | Media Type | Description |
| --- | --- | --- |
| Config | `application/vnd.bazel.module.config.v1+json` | Module name, version, compatibility level |
| MODULE.bazel | `application/vnd.bazel.module.file.v1` | The module definition file |
| source.json | `application/vnd.bazel.module.source.v1+json` | Archive URL, integrity, strip prefix |
| Patches | `application/vnd.bazel.module.patch.v1` | Zero or more patch files |
| Overlays | `application/vnd.bazel.module.overlay.v1` | Zero or more BUILD file overlays |

A separate metadata manifest (tagged `metadata`) holds the version list for each module.

### R2 key layout

```
{namespace}/bazel/{module}/manifests/{tag-or-digest}
{namespace}/bazel/{module}/blobs/{algo}/{hex}
_uploads/{uuid}/data
_uploads/{uuid}/_meta
```

### OCI reference format

```
{registry}/{namespace}/bazel/{module}:{version}
```

Examples:

```
my-worker.example.com/myns/bazel/rules_go:0.50.1        # module version
my-worker.example.com/myns/bazel/rules_go:metadata       # version list
my-worker.example.com/myns/bazel/_registry:config         # registry config
```

## Prerequisites

- [Rust 1.90+](https://rustup.rs/) with `clippy` and `rustfmt` components
- [wrangler](https://developers.cloudflare.com/workers/wrangler/) CLI for deployment
- A Cloudflare account with an R2 bucket

Optional:

- [just](https://github.com/casey/just) for development commands
- [lefthook](https://github.com/evilmartians/lefthook) for pre-commit hooks

## Quick start

### 1. Clone and build

```bash
git clone https://github.com/albertocavalcante/bazel-oci-worker.git
cd bazel-oci-worker
cargo build --workspace
cargo test -p bazel-oci-types -p bazel-oci-registry
```

### 2. Configure Cloudflare

Create an R2 bucket:

```bash
wrangler r2 bucket create bazel-oci
```

Set the auth token secret (optional, omit for public read-only):

```bash
wrangler secret put AUTH_TOKEN
```

Edit `wrangler.toml` to match your environment:

```toml
name = "bazel-oci-worker"
main = "build/worker/shim.mjs"
compatibility_date = "2025-01-01"

[build]
command = "cargo install -q worker-build && worker-build --release"

[[r2_buckets]]
binding = "BUCKET"
bucket_name = "bazel-oci"   # your bucket name

[vars]
NAMESPACE = "default"        # your namespace
```

### 3. Deploy

```bash
wrangler deploy
```

### 4. Configure Bazel

Add a `.bazelrc` entry pointing to your worker:

```
common --registry=https://your-worker.your-subdomain.workers.dev
```

Or in `MODULE.bazel`:

```starlark
bazel_dep(name = "rules_go", version = "0.50.1")
```

## Pushing modules

Use any OCI-compliant tool. With [ORAS](https://oras.land/):

```bash
# Push a module version manifest
oras push your-worker.workers.dev/myns/bazel/rules_go:0.50.1 \
  --config config.json:application/vnd.bazel.module.config.v1+json \
  MODULE.bazel:application/vnd.bazel.module.file.v1 \
  source.json:application/vnd.bazel.module.source.v1+json

# Push metadata
oras push your-worker.workers.dev/myns/bazel/rules_go:metadata \
  --config config.json:application/vnd.bazel.module.config.v1+json \
  metadata.json:application/vnd.bazel.module.metadata.v1+json
```

Write endpoints require a Bearer token when `AUTH_TOKEN` is configured:

```bash
oras push --plain-http --password "$TOKEN" --username "" \
  your-worker.workers.dev/myns/bazel/rules_go:0.50.1 ...
```

## Configuration

| Variable | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `BUCKET` | R2 binding | Yes | — | R2 bucket binding name |
| `NAMESPACE` | env var | No | `default` | OCI namespace prefix for all repositories |
| `AUTH_TOKEN` | secret | No | — | Bearer/Basic token for write operations. Omit for public read-only |

### Authentication

When `AUTH_TOKEN` is set, all write operations (PUT, POST, PATCH, DELETE) require one of:

- `Authorization: Bearer {token}`
- `Authorization: Basic {base64}` where the password portion matches the token

Read operations (GET, HEAD) are always unauthenticated.

When `AUTH_TOKEN` is not set, all operations are allowed without authentication.

## API reference

### OCI Distribution v2

| Method | Path | Auth | Description |
| --- | --- | --- | --- |
| GET | `/v2/` | No | Version check, returns `Docker-Distribution-API-Version` header |
| GET | `/v2/{name}/manifests/{reference}` | No | Retrieve manifest by tag or digest |
| HEAD | `/v2/{name}/manifests/{reference}` | No | Check manifest existence |
| PUT | `/v2/{name}/manifests/{reference}` | Yes | Store manifest |
| DELETE | `/v2/{name}/manifests/{reference}` | Yes | Delete manifest |
| GET | `/v2/{name}/blobs/{digest}` | No | Download blob |
| HEAD | `/v2/{name}/blobs/{digest}` | No | Check blob existence |
| DELETE | `/v2/{name}/blobs/{digest}` | Yes | Delete blob |
| POST | `/v2/{name}/blobs/uploads/` | Yes | Initiate chunked upload or monolithic upload with `?digest=` |
| PATCH | `/v2/{name}/blobs/uploads/{uuid}` | Yes | Append chunk |
| PUT | `/v2/{name}/blobs/uploads/{uuid}?digest=...` | Yes | Complete upload |
| GET | `/v2/_catalog` | No | List repositories |
| GET | `/v2/{name}/tags/list` | No | List tags |

### BCR HTTP API

| Method | Path | Description |
| --- | --- | --- |
| GET | `/bazel_registry.json` | Registry configuration |
| GET | `/modules/{name}/metadata.json` | Module version list |
| GET | `/modules/{name}/{version}/MODULE.bazel` | Module definition |
| GET | `/modules/{name}/{version}/source.json` | Source descriptor |
| GET | `/modules/{name}/{version}/patches/{file}` | Patch file |
| GET | `/modules/{name}/{version}/overlay/{file}` | Overlay file |

BCR endpoints translate internally to OCI manifest/blob lookups. No separate storage is involved.

## Development

### Commands

```bash
just build       # cargo build --workspace
just test        # cargo test --workspace
just fmt         # cargo fmt
just lint        # cargo clippy --workspace --all-targets -- -D warnings
just dev         # wrangler dev (local server)
just deploy      # wrangler deploy
```

### Pre-commit hooks

[lefthook](https://github.com/evilmartians/lefthook) is configured for:

- **Format check** on all `.rs` files
- **Clippy** with `-D warnings`
- **Conventional Commits** enforcement on commit messages

Install hooks:

```bash
lefthook install
```

Commit messages must follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(registry): add OCI manifest push
fix(uploads): clean up artifacts on digest mismatch
docs: update API reference
```

### Testing

The types and registry crates have full test coverage and run on native targets:

```bash
cargo test -p bazel-oci-types -p bazel-oci-registry
```

The worker crate compiles only to `wasm32-unknown-unknown` and is tested through `wrangler dev` locally.

### Project structure

```
bazel-oci-worker/
├── Cargo.toml                  Workspace root
├── wrangler.toml               Cloudflare Workers config
├── justfile                    Development commands
├── lefthook.yml                Pre-commit hooks
├── rust-toolchain.toml         Rust 1.90 with clippy/rustfmt
└── crates/
    ├── bazel-oci-types/
    │   └── src/
    │       ├── annotations.rs  OCI annotation constants
    │       ├── bcr.rs          BCR metadata/source structs
    │       ├── config.rs       Module config type
    │       ├── digest.rs       SHA-256 digest helpers
    │       ├── media_types.rs  Custom media type constants
    │       ├── oci.rs          OCI manifest/descriptor types
    │       └── reference.rs    OCI reference parsing
    ├── bazel-oci-registry/
    │   └── src/
    │       ├── error.rs        OCI error codes and responses
    │       ├── keys.rs         Centralized R2 key builders
    │       ├── memory.rs       In-memory storage for tests
    │       ├── storage.rs      Storage trait abstraction
    │       ├── bcr/            BCR endpoint handlers
    │       │   ├── metadata.rs
    │       │   ├── module_bazel.rs
    │       │   ├── overlays.rs
    │       │   ├── patches.rs
    │       │   ├── registry_json.rs
    │       │   └── source_json.rs
    │       └── oci/            OCI endpoint handlers
    │           ├── blobs.rs
    │           ├── catalog.rs
    │           ├── manifests.rs
    │           ├── ping.rs
    │           └── uploads.rs
    └── bazel-oci-worker/
        └── src/
            ├── auth.rs         Bearer/Basic auth validation
            ├── r2_storage.rs   R2 Storage implementation
            ├── router.rs       axum HTTP routing
            └── state.rs        Shared app state, SecretToken
```

### Storage trait

The registry is decoupled from Cloudflare through the `Storage` trait:

```rust
#[async_trait(?Send)]
pub trait Storage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String>;
    async fn put(&self, key: &str, data: &[u8], content_type: Option<&str>) -> Result<(), String>;
    async fn head(&self, key: &str) -> Result<Option<ObjectMeta>, String>;
    async fn delete(&self, key: &str) -> Result<(), String>;
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, String>;
    async fn append(&self, key: &str, data: &[u8]) -> Result<u64, String>;
}
```

`InMemoryStorage` is used in tests. `R2Storage` is the production implementation. Adding a new backend (S3, GCS, local filesystem) requires implementing this trait.

### Linting

The workspace enforces strict clippy lints including `pedantic` and `nursery` sets, plus:

- `unsafe_code = "deny"`
- `unwrap_used = "deny"`
- `expect_used = "deny"`
- `dbg_macro = "deny"`
- `print_stdout = "deny"` / `print_stderr = "deny"`
- `todo = "deny"` / `unimplemented = "deny"`

## CI

GitHub Actions runs on pushes and pull requests to `main`:

1. Format check (`cargo fmt --check`)
2. Clippy (`cargo clippy --workspace --all-targets -- -D warnings`)
3. Tests (`cargo test -p bazel-oci-types -p bazel-oci-registry`)

## License

[Apache-2.0](LICENSE)
