use async_trait::async_trait;
use worker::Bucket;

use bazel_oci_registry::storage::{ObjectMeta, Storage};

/// R2-backed storage implementation for Cloudflare Workers.
///
// TODO(go-wasm): Go requires raw `syscall/js` FFI vs workers-rs idiomatic bindings.
// Rust's `worker` crate provides type-safe R2 bindings that compile to efficient
// wasm32 code. Go would need manual JS interop for every R2 API call.
pub struct R2Storage {
    bucket: Bucket,
}

impl R2Storage {
    pub fn new(bucket: Bucket) -> Self {
        Self { bucket }
    }
}

#[async_trait(?Send)]
impl Storage for R2Storage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let obj = self
            .bucket
            .get(key)
            .execute()
            .await
            .map_err(|e| e.to_string())?;

        let Some(obj) = obj else {
            return Ok(None);
        };

        let body = obj.body().ok_or("object has no body")?;
        let bytes = body.bytes().await.map_err(|e| e.to_string())?;
        Ok(Some(bytes))
    }

    async fn put(&self, key: &str, data: &[u8], content_type: Option<&str>) -> Result<(), String> {
        let mut builder = self.bucket.put(key, worker::Data::Bytes(data.to_vec()));
        if let Some(ct) = content_type {
            let mut headers = worker::Headers::new();
            headers.set("content-type", ct).map_err(|e| e.to_string())?;
            builder = builder.http_metadata(worker::HttpMetadata {
                content_type: Some(ct.to_owned()),
                ..Default::default()
            });
        }
        builder.execute().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn head(&self, key: &str) -> Result<Option<ObjectMeta>, String> {
        let obj = self.bucket.head(key).await.map_err(|e| e.to_string())?;

        Ok(obj.map(|o| ObjectMeta {
            size: o.size() as u64,
            content_type: o.http_metadata().content_type.clone(),
        }))
    }

    async fn delete(&self, key: &str) -> Result<(), String> {
        self.bucket.delete(key).await.map_err(|e| e.to_string())
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, String> {
        let mut keys = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let mut builder = self.bucket.list().prefix(prefix);
            if let Some(ref c) = cursor {
                builder = builder.cursor(c.clone());
            }

            let result = builder.execute().await.map_err(|e| e.to_string())?;

            for obj in result.objects() {
                keys.push(obj.key());
            }

            if result.truncated() {
                cursor = result.cursor();
                if cursor.is_none() {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(keys)
    }

    // TODO(go-wasm): Go's `io.Reader` streaming broken in wasm32; must buffer all data.
    // Rust naturally buffers with `&[u8]` so append is read-concat-write.
    async fn append(&self, key: &str, data: &[u8]) -> Result<u64, String> {
        // R2 doesn't support append, so we read + concat + write
        let existing = self.get(key).await?.unwrap_or_default();
        let mut combined = existing;
        combined.extend_from_slice(data);
        let len = combined.len() as u64;
        self.put(key, &combined, None).await?;
        Ok(len)
    }
}
