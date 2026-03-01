use std::collections::HashMap;

use async_trait::async_trait;

use crate::storage::{ObjectMeta, Storage};

struct StoredObject {
    data: Vec<u8>,
    content_type: Option<String>,
}

/// In-memory storage backend for testing.
pub struct InMemoryStorage {
    objects: std::cell::RefCell<HashMap<String, StoredObject>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            objects: std::cell::RefCell::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait(?Send)]
impl Storage for InMemoryStorage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.objects.borrow().get(key).map(|o| o.data.clone()))
    }

    async fn put(&self, key: &str, data: &[u8], content_type: Option<&str>) -> Result<(), String> {
        self.objects.borrow_mut().insert(
            key.to_owned(),
            StoredObject {
                data: data.to_vec(),
                content_type: content_type.map(ToOwned::to_owned),
            },
        );
        Ok(())
    }

    async fn head(&self, key: &str) -> Result<Option<ObjectMeta>, String> {
        Ok(self.objects.borrow().get(key).map(|o| ObjectMeta {
            size: o.data.len() as u64,
            content_type: o.content_type.clone(),
        }))
    }

    async fn delete(&self, key: &str) -> Result<(), String> {
        self.objects.borrow_mut().remove(key);
        Ok(())
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, String> {
        let objects = self.objects.borrow();
        let mut keys: Vec<String> = objects
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        keys.sort();
        Ok(keys)
    }

    async fn append(&self, key: &str, data: &[u8]) -> Result<u64, String> {
        let mut objects = self.objects.borrow_mut();
        let entry = objects
            .entry(key.to_owned())
            .or_insert_with(|| StoredObject {
                data: Vec::new(),
                content_type: None,
            });
        entry.data.extend_from_slice(data);
        Ok(entry.data.len() as u64)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::cast_possible_wrap)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn put_and_get_roundtrip() {
        let s = InMemoryStorage::new();
        s.put("key", b"value", None).await.unwrap();
        let got = s.get("key").await.unwrap().unwrap();
        assert_eq!(got, b"value");
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let s = InMemoryStorage::new();
        let got = s.get("missing").await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn put_overwrites_existing() {
        let s = InMemoryStorage::new();
        s.put("key", b"v1", None).await.unwrap();
        s.put("key", b"v2", None).await.unwrap();
        let got = s.get("key").await.unwrap().unwrap();
        assert_eq!(got, b"v2");
    }

    #[tokio::test]
    async fn put_preserves_content_type() {
        let s = InMemoryStorage::new();
        s.put("key", b"data", Some("application/json"))
            .await
            .unwrap();
        let meta = s.head("key").await.unwrap().unwrap();
        assert_eq!(meta.content_type.as_deref(), Some("application/json"));
    }

    #[tokio::test]
    async fn put_with_no_content_type() {
        let s = InMemoryStorage::new();
        s.put("key", b"data", None).await.unwrap();
        let meta = s.head("key").await.unwrap().unwrap();
        assert!(meta.content_type.is_none());
    }

    #[tokio::test]
    async fn head_nonexistent_returns_none() {
        let s = InMemoryStorage::new();
        let meta = s.head("missing").await.unwrap();
        assert!(meta.is_none());
    }

    #[tokio::test]
    async fn head_returns_correct_size() {
        let s = InMemoryStorage::new();
        s.put("key", b"12345", None).await.unwrap();
        let meta = s.head("key").await.unwrap().unwrap();
        assert_eq!(meta.size, 5);
    }

    #[tokio::test]
    async fn head_empty_data_returns_zero_size() {
        let s = InMemoryStorage::new();
        s.put("key", b"", None).await.unwrap();
        let meta = s.head("key").await.unwrap().unwrap();
        assert_eq!(meta.size, 0);
    }

    #[tokio::test]
    async fn delete_existing_key() {
        let s = InMemoryStorage::new();
        s.put("key", b"data", None).await.unwrap();
        s.delete("key").await.unwrap();
        assert!(s.get("key").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_key_succeeds() {
        let s = InMemoryStorage::new();
        s.delete("missing").await.unwrap();
    }

    #[tokio::test]
    async fn list_keys_empty_storage() {
        let s = InMemoryStorage::new();
        let keys = s.list_keys("").await.unwrap();
        assert!(keys.is_empty());
    }

    #[tokio::test]
    async fn list_keys_returns_matching_prefix() {
        let s = InMemoryStorage::new();
        s.put("repo/blobs/a", b"1", None).await.unwrap();
        s.put("repo/blobs/b", b"2", None).await.unwrap();
        s.put("repo/manifests/c", b"3", None).await.unwrap();

        let keys = s.list_keys("repo/blobs/").await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"repo/blobs/a".to_owned()));
        assert!(keys.contains(&"repo/blobs/b".to_owned()));
    }

    #[tokio::test]
    async fn list_keys_no_matching_prefix() {
        let s = InMemoryStorage::new();
        s.put("foo/bar", b"data", None).await.unwrap();
        let keys = s.list_keys("baz/").await.unwrap();
        assert!(keys.is_empty());
    }

    #[tokio::test]
    async fn list_keys_empty_prefix_returns_all() {
        let s = InMemoryStorage::new();
        s.put("a", b"1", None).await.unwrap();
        s.put("b", b"2", None).await.unwrap();
        let keys = s.list_keys("").await.unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[tokio::test]
    async fn list_keys_returns_sorted() {
        let s = InMemoryStorage::new();
        s.put("z/key", b"1", None).await.unwrap();
        s.put("a/key", b"2", None).await.unwrap();
        s.put("m/key", b"3", None).await.unwrap();
        let keys = s.list_keys("").await.unwrap();
        assert_eq!(keys, vec!["a/key", "m/key", "z/key"]);
    }

    #[tokio::test]
    async fn append_to_nonexistent_creates_entry() {
        let s = InMemoryStorage::new();
        let size = s.append("newkey", b"hello").await.unwrap();
        assert_eq!(size, 5);
        let got = s.get("newkey").await.unwrap().unwrap();
        assert_eq!(got, b"hello");
    }

    #[tokio::test]
    async fn append_to_existing_extends_data() {
        let s = InMemoryStorage::new();
        s.put("key", b"hello", None).await.unwrap();
        let size = s.append("key", b" world").await.unwrap();
        assert_eq!(size, 11);
        let got = s.get("key").await.unwrap().unwrap();
        assert_eq!(got, b"hello world");
    }

    #[tokio::test]
    async fn append_multiple_times() {
        let s = InMemoryStorage::new();
        let s1 = s.append("key", b"a").await.unwrap();
        assert_eq!(s1, 1);
        let s2 = s.append("key", b"bc").await.unwrap();
        assert_eq!(s2, 3);
        let s3 = s.append("key", b"def").await.unwrap();
        assert_eq!(s3, 6);
        let got = s.get("key").await.unwrap().unwrap();
        assert_eq!(got, b"abcdef");
    }

    #[tokio::test]
    async fn append_empty_data() {
        let s = InMemoryStorage::new();
        s.put("key", b"data", None).await.unwrap();
        let size = s.append("key", b"").await.unwrap();
        assert_eq!(size, 4);
        let got = s.get("key").await.unwrap().unwrap();
        assert_eq!(got, b"data");
    }

    #[tokio::test]
    async fn append_new_key_has_no_content_type() {
        let s = InMemoryStorage::new();
        s.append("key", b"data").await.unwrap();
        let meta = s.head("key").await.unwrap().unwrap();
        assert!(meta.content_type.is_none());
    }

    #[tokio::test]
    async fn append_preserves_existing_content_type() {
        let s = InMemoryStorage::new();
        s.put("key", b"start", Some("text/plain")).await.unwrap();
        s.append("key", b" more").await.unwrap();
        let meta = s.head("key").await.unwrap().unwrap();
        assert_eq!(meta.content_type.as_deref(), Some("text/plain"));
    }

    #[test]
    fn default_creates_empty_storage() {
        let s = InMemoryStorage::default();
        assert!(s.objects.borrow().is_empty());
    }
}
