use std::rc::Rc;

use axum::body::Body;
use axum::extract::Path;
use axum::response::Response;
use http::{HeaderMap, StatusCode};

use bazel_oci_registry::error::RegistryError;
use bazel_oci_registry::oci::ping::PingResponse;
use bazel_oci_registry::storage::Storage;

use crate::auth::{self, AuthResult};
use crate::state::AppState;

fn error_response(err: &RegistryError) -> Response {
    let status =
        StatusCode::from_u16(err.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = serde_json::to_vec(&err.to_error_response()).unwrap_or_default();
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    response.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/json"),
    );
    response
}

fn require_auth<S: Storage>(state: &AppState<S>, headers: &HeaderMap) -> Option<Response> {
    let auth_header = headers.get("Authorization").and_then(|v| v.to_str().ok());
    match auth::check_auth(auth_header, state.auth_token.as_deref()) {
        AuthResult::Allowed => None,
        AuthResult::Denied => Some(
            Response::builder()
                .status(auth::UNAUTHORIZED)
                .header("WWW-Authenticate", auth::WWW_AUTHENTICATE)
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"errors":[{"code":"UNAUTHORIZED","message":"authentication required"}]}"#,
                ))
                .unwrap(),
        ),
    }
}

/// Extract a query parameter value from a URI.
fn query_param<'a>(uri: &'a http::Uri, key: &str) -> Option<&'a str> {
    uri.query().and_then(|q| {
        q.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            if k == key { Some(v) } else { None }
        })
    })
}

/// Build the axum Router for the OCI + BCR endpoints.
///
// TODO(go-wasm): Go WASM needs `wasm_exec.js` polyfill + manual event handler registration.
// Rust's `worker` crate handles the WASM entry point automatically via `#[event(fetch)]`,
// and axum's Router maps directly to the Workers fetch handler.
pub fn build_router<S: Storage + 'static>(state: AppState<S>) -> axum::Router {
    let state = Rc::new(state);

    axum::Router::new()
        // OCI Distribution API
        .route("/v2/", axum::routing::get({
            let state = Rc::clone(&state);
            move || async move { handle_ping(&state) }
        }))
        .route("/v2/{*name_and_rest}", axum::routing::get({
            let state = Rc::clone(&state);
            move |path: Path<String>, headers: HeaderMap| async move {
                handle_oci_get(&state, &path, &headers).await
            }
        }).head({
            let state = Rc::clone(&state);
            move |path: Path<String>, headers: HeaderMap| async move {
                handle_oci_head(&state, &path, &headers).await
            }
        }).put({
            let state = Rc::clone(&state);
            move |uri: http::Uri, path: Path<String>, headers: HeaderMap, body: axum::body::Bytes| async move {
                handle_oci_put(&state, &uri, &path, &headers, &body).await
            }
        }).post({
            let state = Rc::clone(&state);
            move |uri: http::Uri, path: Path<String>, headers: HeaderMap, body: axum::body::Bytes| async move {
                handle_oci_post(&state, &uri, &path, &headers, &body).await
            }
        }).patch({
            let state = Rc::clone(&state);
            move |path: Path<String>, headers: HeaderMap, body: axum::body::Bytes| async move {
                handle_oci_patch(&state, &path, &headers, &body).await
            }
        }).delete({
            let state = Rc::clone(&state);
            move |path: Path<String>, headers: HeaderMap| async move {
                handle_oci_delete(&state, &path, &headers).await
            }
        }))
        // BCR HTTP API
        .route("/bazel_registry.json", axum::routing::get(|| async {
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(bazel_oci_registry::bcr::registry_json::registry_json()))
                .unwrap()
        }))
        .route("/modules/{name}/metadata.json", axum::routing::get({
            let state = Rc::clone(&state);
            move |Path(name): Path<String>| async move {
                handle_bcr_metadata(&state, &name).await
            }
        }))
        .route("/modules/{name}/{version}/MODULE.bazel", axum::routing::get({
            let state = Rc::clone(&state);
            move |Path((name, version)): Path<(String, String)>| async move {
                handle_bcr_module_bazel(&state, &name, &version).await
            }
        }))
        .route("/modules/{name}/{version}/source.json", axum::routing::get({
            let state = Rc::clone(&state);
            move |Path((name, version)): Path<(String, String)>| async move {
                handle_bcr_source_json(&state, &name, &version).await
            }
        }))
        .route("/modules/{name}/{version}/patches/{*file}", axum::routing::get({
            let state = Rc::clone(&state);
            move |Path((name, version, file)): Path<(String, String, String)>| async move {
                handle_bcr_patch(&state, &name, &version, &file).await
            }
        }))
        .route("/modules/{name}/{version}/overlay/{*file}", axum::routing::get({
            let state = Rc::clone(&state);
            move |Path((name, version, file)): Path<(String, String, String)>| async move {
                handle_bcr_overlay(&state, &name, &version, &file).await
            }
        }))
}

fn handle_ping<S: Storage>(_state: &AppState<S>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(PingResponse::HEADER_NAME, PingResponse::HEADER_VALUE)
        .header("Content-Type", "application/json")
        .body(Body::from("{}"))
        .unwrap()
}

/// Route OCI GET requests based on the path pattern.
async fn handle_oci_get<S: Storage>(
    state: &AppState<S>,
    path: &str,
    _headers: &HeaderMap,
) -> Response {
    // /v2/{name}/manifests/{reference}
    if let Some((name, reference)) = parse_oci_path(path, "manifests") {
        match bazel_oci_registry::oci::manifests::get_manifest(&state.storage, name, reference)
            .await
        {
            Ok((data, digest)) => Response::builder()
                .status(StatusCode::OK)
                .header(
                    "Content-Type",
                    bazel_oci_types::media_types::MEDIA_TYPE_OCI_MANIFEST,
                )
                .header("Docker-Content-Digest", digest)
                .header("Content-Length", data.len().to_string())
                .body(Body::from(data))
                .unwrap(),
            Err(e) => error_response(&e),
        }
    }
    // /v2/{name}/blobs/{digest}
    else if let Some((name, digest)) = parse_oci_path(path, "blobs") {
        match bazel_oci_registry::oci::blobs::get_blob(&state.storage, name, digest).await {
            Ok(data) => Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/octet-stream")
                .header("Docker-Content-Digest", digest)
                .header("Content-Length", data.len().to_string())
                .body(Body::from(data))
                .unwrap(),
            Err(e) => error_response(&e),
        }
    }
    // /v2/{name}/tags/list
    else if let Some(name) = parse_tags_list_path(path) {
        match bazel_oci_registry::oci::catalog::tags_list(&state.storage, name).await {
            Ok(result) => {
                let body = serde_json::to_vec(&result).unwrap_or_default();
                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .unwrap()
            }
            Err(e) => error_response(&e),
        }
    }
    // /v2/_catalog
    else if path == "_catalog" {
        match bazel_oci_registry::oci::catalog::catalog(&state.storage).await {
            Ok(result) => {
                let body = serde_json::to_vec(&result).unwrap_or_default();
                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .unwrap()
            }
            Err(e) => error_response(&e),
        }
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap()
    }
}

async fn handle_oci_head<S: Storage>(
    state: &AppState<S>,
    path: &str,
    _headers: &HeaderMap,
) -> Response {
    if let Some((name, reference)) = parse_oci_path(path, "manifests") {
        match bazel_oci_registry::oci::manifests::head_manifest(&state.storage, name, reference)
            .await
        {
            Ok((size, digest)) => Response::builder()
                .status(StatusCode::OK)
                .header(
                    "Content-Type",
                    bazel_oci_types::media_types::MEDIA_TYPE_OCI_MANIFEST,
                )
                .header("Docker-Content-Digest", digest)
                .header("Content-Length", size.to_string())
                .body(Body::empty())
                .unwrap(),
            Err(e) => error_response(&e),
        }
    } else if let Some((name, digest)) = parse_oci_path(path, "blobs") {
        match bazel_oci_registry::oci::blobs::head_blob(&state.storage, name, digest).await {
            Ok(size) => Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/octet-stream")
                .header("Docker-Content-Digest", digest)
                .header("Content-Length", size.to_string())
                .body(Body::empty())
                .unwrap(),
            Err(e) => error_response(&e),
        }
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap()
    }
}

async fn handle_oci_put<S: Storage>(
    state: &AppState<S>,
    uri: &http::Uri,
    path: &str,
    headers: &HeaderMap,
    body: &[u8],
) -> Response {
    if let Some(resp) = require_auth(state, headers) {
        return resp;
    }

    // PUT /v2/{name}/manifests/{reference}
    if let Some((name, reference)) = parse_oci_path(path, "manifests") {
        match bazel_oci_registry::oci::manifests::put_manifest(
            &state.storage,
            name,
            reference,
            body,
        )
        .await
        {
            Ok(digest) => Response::builder()
                .status(StatusCode::CREATED)
                .header("Docker-Content-Digest", digest)
                .header("Location", format!("/v2/{name}/manifests/{reference}"))
                .body(Body::empty())
                .unwrap(),
            Err(e) => error_response(&e),
        }
    }
    // PUT /v2/{name}/blobs/uploads/{uuid}?digest=...
    else if let Some((name, rest)) = parse_oci_path(path, "blobs") {
        if let Some(uuid) = rest.strip_prefix("uploads/") {
            let digest = query_param(uri, "digest").unwrap_or("");

            if digest.is_empty() {
                return error_response(&RegistryError::oci(
                    bazel_oci_registry::error::ErrorCode::DigestInvalid,
                    "missing digest parameter",
                ));
            }

            match bazel_oci_registry::oci::uploads::complete_upload(
                &state.storage,
                name,
                uuid,
                digest,
                if body.is_empty() { None } else { Some(body) },
            )
            .await
            {
                Ok(d) => Response::builder()
                    .status(StatusCode::CREATED)
                    .header("Docker-Content-Digest", &d)
                    .header("Location", format!("/v2/{name}/blobs/{d}"))
                    .body(Body::empty())
                    .unwrap(),
                Err(e) => error_response(&e),
            }
        } else {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap()
        }
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap()
    }
}

async fn handle_oci_post<S: Storage>(
    state: &AppState<S>,
    uri: &http::Uri,
    path: &str,
    headers: &HeaderMap,
    body: &[u8],
) -> Response {
    if let Some(resp) = require_auth(state, headers) {
        return resp;
    }

    // POST /v2/{name}/blobs/uploads/
    if let Some((name, rest)) = parse_oci_path(path, "blobs") {
        if rest == "uploads/" || rest == "uploads" {
            // Check for monolithic upload (digest in query param)
            let digest = query_param(uri, "digest");

            if let Some(digest) = digest {
                // Monolithic upload
                match bazel_oci_registry::oci::uploads::monolithic_upload(
                    &state.storage,
                    name,
                    digest,
                    body,
                )
                .await
                {
                    Ok(d) => Response::builder()
                        .status(StatusCode::CREATED)
                        .header("Docker-Content-Digest", &d)
                        .header("Location", format!("/v2/{name}/blobs/{d}"))
                        .body(Body::empty())
                        .unwrap(),
                    Err(e) => error_response(&e),
                }
            } else {
                // Initiate chunked upload
                let uuid = uuid::Uuid::new_v4().to_string();
                match bazel_oci_registry::oci::uploads::initiate_upload(&state.storage, name, &uuid)
                    .await
                {
                    Ok(id) => Response::builder()
                        .status(StatusCode::ACCEPTED)
                        .header("Location", format!("/v2/{name}/blobs/uploads/{id}"))
                        .header("Docker-Upload-UUID", &id)
                        .header("Range", "0-0")
                        .body(Body::empty())
                        .unwrap(),
                    Err(e) => error_response(&e),
                }
            }
        } else {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap()
        }
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap()
    }
}

async fn handle_oci_patch<S: Storage>(
    state: &AppState<S>,
    path: &str,
    headers: &HeaderMap,
    body: &[u8],
) -> Response {
    if let Some(resp) = require_auth(state, headers) {
        return resp;
    }

    // PATCH /v2/{name}/blobs/uploads/{uuid}
    if let Some((_name, rest)) = parse_oci_path(path, "blobs") {
        if let Some(uuid) = rest.strip_prefix("uploads/") {
            match bazel_oci_registry::oci::uploads::upload_chunk(&state.storage, uuid, body).await {
                Ok(offset) => Response::builder()
                    .status(StatusCode::ACCEPTED)
                    .header("Location", format!("/v2/{path}"))
                    .header("Docker-Upload-UUID", uuid)
                    .header("Range", format!("0-{}", offset.saturating_sub(1)))
                    .body(Body::empty())
                    .unwrap(),
                Err(e) => error_response(&e),
            }
        } else {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap()
        }
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap()
    }
}

async fn handle_oci_delete<S: Storage>(
    state: &AppState<S>,
    path: &str,
    headers: &HeaderMap,
) -> Response {
    if let Some(resp) = require_auth(state, headers) {
        return resp;
    }

    if let Some((name, reference)) = parse_oci_path(path, "manifests") {
        match bazel_oci_registry::oci::manifests::delete_manifest(&state.storage, name, reference)
            .await
        {
            Ok(()) => Response::builder()
                .status(StatusCode::ACCEPTED)
                .body(Body::empty())
                .unwrap(),
            Err(e) => error_response(&e),
        }
    } else if let Some((name, digest)) = parse_oci_path(path, "blobs") {
        match bazel_oci_registry::oci::blobs::delete_blob(&state.storage, name, digest).await {
            Ok(()) => Response::builder()
                .status(StatusCode::ACCEPTED)
                .body(Body::empty())
                .unwrap(),
            Err(e) => error_response(&e),
        }
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap()
    }
}

// === BCR handlers ===

async fn handle_bcr_metadata<S: Storage>(state: &AppState<S>, module: &str) -> Response {
    match bazel_oci_registry::bcr::metadata::get_metadata(&state.storage, &state.namespace, module)
        .await
    {
        Ok(data) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Body::from(data))
            .unwrap(),
        Err(e) => error_response(&e),
    }
}

async fn handle_bcr_module_bazel<S: Storage>(
    state: &AppState<S>,
    module: &str,
    version: &str,
) -> Response {
    match bazel_oci_registry::bcr::module_bazel::get_module_bazel(
        &state.storage,
        &state.namespace,
        module,
        version,
    )
    .await
    {
        Ok(data) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(Body::from(data))
            .unwrap(),
        Err(e) => error_response(&e),
    }
}

async fn handle_bcr_source_json<S: Storage>(
    state: &AppState<S>,
    module: &str,
    version: &str,
) -> Response {
    match bazel_oci_registry::bcr::source_json::get_source_json(
        &state.storage,
        &state.namespace,
        module,
        version,
    )
    .await
    {
        Ok(data) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Body::from(data))
            .unwrap(),
        Err(e) => error_response(&e),
    }
}

async fn handle_bcr_patch<S: Storage>(
    state: &AppState<S>,
    module: &str,
    version: &str,
    filename: &str,
) -> Response {
    match bazel_oci_registry::bcr::patches::get_patch(
        &state.storage,
        &state.namespace,
        module,
        version,
        filename,
    )
    .await
    {
        Ok(data) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(Body::from(data))
            .unwrap(),
        Err(e) => error_response(&e),
    }
}

async fn handle_bcr_overlay<S: Storage>(
    state: &AppState<S>,
    module: &str,
    version: &str,
    overlay_path: &str,
) -> Response {
    match bazel_oci_registry::bcr::overlays::get_overlay(
        &state.storage,
        &state.namespace,
        module,
        version,
        overlay_path,
    )
    .await
    {
        Ok(data) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(Body::from(data))
            .unwrap(),
        Err(e) => error_response(&e),
    }
}

// === Path parsing helpers ===

/// Parse an OCI path like `{name}/{segment}/{value}` where segment is "manifests", "blobs", etc.
/// Returns `(name, value)`.
fn parse_oci_path<'a>(path: &'a str, segment: &str) -> Option<(&'a str, &'a str)> {
    let pattern = format!("/{segment}/");
    let idx = path.find(&pattern)?;
    let name = &path[..idx];
    let value = &path[idx + pattern.len()..];
    if name.is_empty() || value.is_empty() {
        return None;
    }
    Some((name, value))
}

/// Parse a tags list path like `{name}/tags/list`.
fn parse_tags_list_path(path: &str) -> Option<&str> {
    let suffix = "/tags/list";
    if path.ends_with(suffix) {
        let name = &path[..path.len() - suffix.len()];
        if !name.is_empty() {
            return Some(name);
        }
    }
    None
}
