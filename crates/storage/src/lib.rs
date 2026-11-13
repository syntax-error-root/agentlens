#![forbid(unsafe_code)]

//! `agentlens-storage` crate.
//!
//! Handles large payload storage in S3/MinIO for captured span attributes exceeding 64 KiB,
//! and facilitates batch payload deletion during retention pruning.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use thiserror::Error;
use tracing::{debug, error, info};

pub const LARGE_PAYLOAD_THRESHOLD_BYTES: usize = 64 * 1024; // 64 KiB

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Storage configuration error: {0}")]
    Config(String),
    #[error("Network I/O error: {0}")]
    Network(String),
    #[error("Object not found: {0}")]
    NotFound(String),
    #[error("Storage operation failed: {0}")]
    OperationFailed(String),
}

/// Computes the canonical S3 key for an offloaded large span payload.
#[must_use]
pub fn format_payload_s3_key(project_id: &str, trace_id: &str, span_id: &str) -> String {
    format!("projects/{project_id}/traces/{trace_id}/spans/{span_id}.json")
}

#[async_trait]
pub trait StorageClient: Send + Sync {
    async fn put_payload(&self, key: &str, data: &[u8]) -> Result<(), StorageError>;
    async fn get_payload(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;
    async fn delete_payload(&self, key: &str) -> Result<(), StorageError>;
    async fn delete_payloads(&self, keys: &[String]) -> Result<usize, StorageError>;
}

/// In-memory storage client for unit testing and local pipelines without external MinIO/S3.
#[derive(Debug, Default, Clone)]
pub struct MemoryStorageClient {
    store: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl MemoryStorageClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.store.read().map(|s| s.len()).unwrap_or(0)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[async_trait]
impl StorageClient for MemoryStorageClient {
    async fn put_payload(&self, key: &str, data: &[u8]) -> Result<(), StorageError> {
        let mut store = self
            .store
            .write()
            .map_err(|e| StorageError::OperationFailed(format!("Lock poisoned: {e}")))?;
        store.insert(key.to_string(), data.to_vec());
        debug!(key = %key, bytes = data.len(), "Stored payload in memory storage");
        Ok(())
    }

    async fn get_payload(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let store = self
            .store
            .read()
            .map_err(|e| StorageError::OperationFailed(format!("Lock poisoned: {e}")))?;
        Ok(store.get(key).cloned())
    }

    async fn delete_payload(&self, key: &str) -> Result<(), StorageError> {
        let mut store = self
            .store
            .write()
            .map_err(|e| StorageError::OperationFailed(format!("Lock poisoned: {e}")))?;
        store.remove(key);
        debug!(key = %key, "Deleted payload from memory storage");
        Ok(())
    }

    async fn delete_payloads(&self, keys: &[String]) -> Result<usize, StorageError> {
        let mut store = self
            .store
            .write()
            .map_err(|e| StorageError::OperationFailed(format!("Lock poisoned: {e}")))?;
        let mut count = 0;
        for k in keys {
            if store.remove(k).is_some() {
                count += 1;
            }
        }
        debug!(
            deleted_count = count,
            "Batch deleted payloads from memory storage"
        );
        Ok(count)
    }
}

/// S3 / MinIO HTTP storage client.
#[derive(Debug, Clone)]
pub struct S3StorageClient {
    endpoint: String,
    bucket: String,
    client: reqwest::Client,
}

impl S3StorageClient {
    pub fn new(endpoint: &str, bucket: &str) -> Result<Self, StorageError> {
        if endpoint.is_empty() || bucket.is_empty() {
            return Err(StorageError::Config(
                "S3 endpoint and bucket cannot be empty".to_string(),
            ));
        }

        let clean_endpoint = endpoint.trim_end_matches('/').to_string();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| StorageError::Config(format!("Failed to build HTTP client: {e}")))?;

        Ok(Self {
            endpoint: clean_endpoint,
            bucket: bucket.to_string(),
            client,
        })
    }

    fn object_url(&self, key: &str) -> String {
        format!(
            "{}/{}/{}",
            self.endpoint,
            self.bucket,
            key.trim_start_matches('/')
        )
    }
}

#[async_trait]
impl StorageClient for S3StorageClient {
    async fn put_payload(&self, key: &str, data: &[u8]) -> Result<(), StorageError> {
        let url = self.object_url(key);
        let resp = self
            .client
            .put(&url)
            .header("Content-Type", "application/json")
            .body(data.to_vec())
            .send()
            .await
            .map_err(|e| StorageError::Network(format!("PUT failed for {key}: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(StorageError::OperationFailed(format!(
                "S3 PUT returned HTTP {status}: {text}"
            )));
        }

        info!(key = %key, bytes = data.len(), "Successfully offloaded large payload to S3");
        Ok(())
    }

    async fn get_payload(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let url = self.object_url(key);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| StorageError::Network(format!("GET failed for {key}: {e}")))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(StorageError::OperationFailed(format!(
                "S3 GET returned HTTP {status}: {text}"
            )));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| StorageError::Network(format!("Failed reading response bytes: {e}")))?;

        Ok(Some(bytes.to_vec()))
    }

    async fn delete_payload(&self, key: &str) -> Result<(), StorageError> {
        let url = self.object_url(key);
        let resp = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| StorageError::Network(format!("DELETE failed for {key}: {e}")))?;

        if !resp.status().is_success() && resp.status() != reqwest::StatusCode::NOT_FOUND {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(StorageError::OperationFailed(format!(
                "S3 DELETE returned HTTP {status}: {text}"
            )));
        }

        debug!(key = %key, "Deleted payload from S3");
        Ok(())
    }

    async fn delete_payloads(&self, keys: &[String]) -> Result<usize, StorageError> {
        let mut count = 0;
        for key in keys {
            if self.delete_payload(key).await.is_ok() {
                count += 1;
            } else {
                error!(key = %key, "Failed deleting S3 payload during retention batch");
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_storage_roundtrip() {
        let storage = MemoryStorageClient::new();
        assert!(storage.is_empty());

        let key = "projects/p1/traces/t1/spans/s1.json";
        let data = b"{\"prompt\":\"large payload test\"}";

        storage
            .put_payload(key, data)
            .await
            .expect("Put must succeed");
        assert_eq!(storage.len(), 1);

        let retrieved = storage
            .get_payload(key)
            .await
            .expect("Get must succeed")
            .expect("Payload must exist");
        assert_eq!(retrieved, data);

        let missing = storage
            .get_payload("nonexistent")
            .await
            .expect("Get missing must succeed");
        assert!(missing.is_none());

        storage
            .delete_payload(key)
            .await
            .expect("Delete must succeed");
        assert!(storage.is_empty());
    }

    #[tokio::test]
    async fn test_batch_delete_payloads() {
        let storage = MemoryStorageClient::new();
        let k1 = "key1".to_string();
        let k2 = "key2".to_string();
        let k3 = "key3".to_string();

        storage.put_payload(&k1, b"1").await.unwrap();
        storage.put_payload(&k2, b"2").await.unwrap();
        storage.put_payload(&k3, b"3").await.unwrap();
        assert_eq!(storage.len(), 3);

        let deleted = storage
            .delete_payloads(&[k1, k2, "missing".to_string()])
            .await
            .unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(storage.len(), 1);
    }

    #[test]
    fn test_format_payload_s3_key() {
        let key = format_payload_s3_key("proj-123", "trace-abc", "span-xyz");
        assert_eq!(
            key,
            "projects/proj-123/traces/trace-abc/spans/span-xyz.json"
        );
    }
}
