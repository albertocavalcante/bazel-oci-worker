mod auth;
mod r2_storage;
mod router;
mod state;

use worker::{Context, Env, HttpRequest, HttpResponse, event};

use r2_storage::R2Storage;
use state::{AppState, SecretToken};

// TODO(go-wasm): Go WASM needs `wasm_exec.js` polyfill + manual event handler registration.
// Rust's `worker` crate handles the WASM entry point automatically via `#[event(fetch)]`.

// TODO(go-wasm): Binary size -- Standard Go produces 2-3MB+ WASM; TinyGo needed but lossy.
// Rust with `opt-level = "s"` + LTO typically produces <500KB Workers.

// TODO(go-wasm): JSON handling -- TinyGo's `encoding/json` panics at runtime; would need
// `tinyjson` codegen. Rust's serde_json works identically in wasm32 and native.

/// Cloudflare Worker entry point.
#[event(fetch)]
async fn fetch(req: HttpRequest, env: Env, _ctx: Context) -> worker::Result<HttpResponse> {
    let bucket = env.bucket("BUCKET")?;
    let namespace = env
        .var("NAMESPACE")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "default".to_owned());
    let auth_token = env
        .secret("AUTH_TOKEN")
        .map(|s| SecretToken::new(s.to_string()))
        .ok();

    let state = AppState {
        storage: R2Storage::new(bucket),
        namespace,
        auth_token,
    };

    let router = router::build_router(state);

    let response = tower_service::Service::call(&mut router.into_service(), req).await?;
    Ok(response)
}
