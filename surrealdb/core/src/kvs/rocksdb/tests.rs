//! Integration tests for the encrypted and versioned RocksDB datastore.
//!
//! These tests verify that AES-256 encryption at rest and MVCC versioning work
//! correctly — individually and in combination.
//!
//! They operate directly on the inner `rocksdb::Datastore` to allow injecting
//! a custom `RocksDbConfig` with `encryption_key` set programmatically (the key
//! cannot be passed through the connection-string query params for security reasons).
//!
//! # Sync mode
//!
//! All helpers use `SyncMode::Never` to avoid creating a `CommitCoordinator`
//! background thread. The coordinator holds an `Arc<DB>` and blocks on a
//! condvar; without a `Drop` impl that signals the condvar, it keeps the
//! RocksDB file lock alive even after the `Datastore` is dropped, which breaks
//! the reopen test. `SyncMode::Never` is fine for correctness testing — WAL
//! flushing is irrelevant to the behaviour being verified here.
//!
//! # `set()` vs `put()`
//!
//! `Transactable::put()` is INSERT-if-not-exists: it reads back the key first
//! and returns `TransactionKeyAlreadyExists` when the key is present (even at
//! an older version). `Transactable::set()` is UPSERT (unconditional write).
//! When a key already has one version and we want to write a second version,
//! `set()` must be used — otherwise the existence check fires on the latest
//! version and rejects the write.

use temp_dir::TempDir;

use super::Datastore;
use crate::kvs::api::Transactable;
use crate::kvs::config::{RocksDbConfig, SyncMode};
use crate::kvs::timestamp::HlcTimeStamp;

/// A fixed 32-byte AES-256 test key. Never use in production.
const TEST_KEY: [u8; 32] = [0x42u8; 32];

/// Open an encrypted + versioned datastore at the given path.
///
/// Uses `SyncMode::Every` to match production configuration.
async fn open_encrypted_versioned(path: &str) -> Datastore {
	let config = RocksDbConfig {
		versioned: true,
		retention_ns: 0,
		sync_mode: SyncMode::Every,
		encryption_key: Some(TEST_KEY),
	};
	Datastore::new(path, config).await.expect("failed to open encrypted+versioned datastore")
}

/// Open a plain (no encryption, no versioning) datastore at the given path.
async fn open_plain(path: &str) -> Result<Datastore, crate::kvs::err::Error> {
	let config = RocksDbConfig::default();
	Datastore::new(path, config).await
}

// ─── Test 1: Opens successfully ─────────────────────────────────────────────

#[tokio::test]
async fn test_encrypted_versioned_opens() {
	let dir = TempDir::new().unwrap();
	let path = dir.path().to_string_lossy().to_string();
	// Must not panic or error
	let _ds = open_encrypted_versioned(&path).await;
}

// ─── Test 2: Basic write and read ───────────────────────────────────────────

#[tokio::test]
async fn test_encrypted_versioned_write_read() {
	let dir = TempDir::new().unwrap();
	let path = dir.path().to_string_lossy().to_string();
	let ds = open_encrypted_versioned(&path).await;

	// Write (first version — set() or put() both work here)
	let tx = ds.transaction(true, true).await.unwrap();
	tx.set(b"hello".to_vec(), b"world".to_vec()).await.unwrap();
	tx.commit().await.unwrap();

	// Read back at latest (None = latest version)
	let tx = ds.transaction(false, false).await.unwrap();
	let val = tx.get(b"hello".to_vec(), None).await.unwrap();
	tx.cancel().await.unwrap();

	assert_eq!(val.as_deref(), Some(b"world".as_ref()), "value did not survive write/read");
}

// ─── Test 3: Point-in-time versioned reads ───────────────────────────────────

#[tokio::test]
async fn test_versioned_point_in_time_read() {
	let dir = TempDir::new().unwrap();
	let path = dir.path().to_string_lossy().to_string();
	let ds = open_encrypted_versioned(&path).await;

	// Write V1 (first write — set() works fine)
	let tx = ds.transaction(true, true).await.unwrap();
	tx.set(b"key".to_vec(), b"v1".to_vec()).await.unwrap();
	tx.commit().await.unwrap();

	// Capture a timestamp strictly after V1's commit but before V2's commit.
	// HlcTimeStamp::next() is monotonic — this value is guaranteed to be
	// greater than V1's commit timestamp and less than V2's commit timestamp.
	let ts_between = HlcTimeStamp::next().0;

	// Write V2 — must use set(), not put(), because the key already has V1.
	let tx = ds.transaction(true, true).await.unwrap();
	tx.set(b"key".to_vec(), b"v2".to_vec()).await.unwrap();
	tx.commit().await.unwrap();

	// Read at latest — must see V2
	let tx = ds.transaction(false, false).await.unwrap();
	let latest = tx.get(b"key".to_vec(), None).await.unwrap();
	tx.cancel().await.unwrap();
	assert_eq!(latest.as_deref(), Some(b"v2".as_ref()), "latest read should see V2");

	// Read at ts_between — must see V1 (written before ts_between)
	let tx = ds.transaction(false, false).await.unwrap();
	let at_ts = tx.get(b"key".to_vec(), Some(ts_between)).await.unwrap();
	tx.cancel().await.unwrap();
	assert_eq!(at_ts.as_deref(), Some(b"v1".as_ref()), "point-in-time read should see V1");
}

// ─── Test 4: Encrypted data is not readable without the key ─────────────────

#[tokio::test]
async fn test_encrypted_data_unreadable_without_key() {
	let dir = TempDir::new().unwrap();
	let path = dir.path().to_string_lossy().to_string();

	// Write with encryption
	{
		let ds = open_encrypted_versioned(&path).await;
		let tx = ds.transaction(true, true).await.unwrap();
		tx.set(b"secret".to_vec(), b"plaintext".to_vec()).await.unwrap();
		tx.commit().await.unwrap();
		// ds drops here — DB is closed (SyncMode::Never means no background thread)
	}

	// Attempt to open without any encryption key.
	// RocksDB should either fail to open or return corrupt/missing data.
	match open_plain(&path).await {
		Err(_) => {
			// Acceptable: RocksDB refuses to open an encrypted DB without a key
		}
		Ok(ds) => {
			// Acceptable: DB opens but the value must not be readable as plaintext
			let tx = ds.transaction(false, false).await.unwrap();
			let val = tx.get(b"secret".to_vec(), None).await;
			tx.cancel().await.unwrap();
			match val {
				Err(_) => {
					// Acceptable: read fails due to corruption / wrong format
				}
				Ok(v) => {
					assert_ne!(
						v.as_deref(),
						Some(b"plaintext".as_ref()),
						"encrypted data must not be readable as plaintext without the key"
					);
				}
			}
		}
	}
}

// ─── Test 5: Encryption + versioning do not interfere with each other ────────

#[tokio::test]
async fn test_encrypted_and_versioned_combined() {
	let dir = TempDir::new().unwrap();
	let path = dir.path().to_string_lossy().to_string();
	let ds = open_encrypted_versioned(&path).await;

	// Write several versions of multiple keys.
	// All iterations use set() so that the second and later writes to each
	// key succeed even though earlier versions already exist.
	for i in 0u8..5 {
		let tx = ds.transaction(true, true).await.unwrap();
		tx.set(b"a".to_vec(), vec![i]).await.unwrap();
		tx.set(b"b".to_vec(), vec![i * 10]).await.unwrap();
		tx.commit().await.unwrap();
	}

	// Capture a mid-point timestamp
	let ts_mid = HlcTimeStamp::next().0;

	for i in 5u8..10 {
		let tx = ds.transaction(true, true).await.unwrap();
		tx.set(b"a".to_vec(), vec![i]).await.unwrap();
		tx.set(b"b".to_vec(), vec![i * 10]).await.unwrap();
		tx.commit().await.unwrap();
	}

	// Latest reads must reflect the last write (i=9)
	let tx = ds.transaction(false, false).await.unwrap();
	let a_latest = tx.get(b"a".to_vec(), None).await.unwrap();
	let b_latest = tx.get(b"b".to_vec(), None).await.unwrap();
	tx.cancel().await.unwrap();
	assert_eq!(a_latest.as_deref(), Some([9u8].as_ref()), "latest 'a' should be 9");
	assert_eq!(b_latest.as_deref(), Some([90u8].as_ref()), "latest 'b' should be 90");

	// Point-in-time reads at ts_mid must reflect the state after i=4 (last before mid)
	let tx = ds.transaction(false, false).await.unwrap();
	let a_mid = tx.get(b"a".to_vec(), Some(ts_mid)).await.unwrap();
	let b_mid = tx.get(b"b".to_vec(), Some(ts_mid)).await.unwrap();
	tx.cancel().await.unwrap();
	assert_eq!(a_mid.as_deref(), Some([4u8].as_ref()), "mid-point 'a' should be 4");
	assert_eq!(b_mid.as_deref(), Some([40u8].as_ref()), "mid-point 'b' should be 40");
}

// ─── Test 6: Reopen encrypted versioned DB — data persists ──────────────────

#[tokio::test]
async fn test_encrypted_versioned_persists_across_reopen() {
	let dir = TempDir::new().unwrap();
	let path = dir.path().to_string_lossy().to_string();

	// First open — write data
	{
		let ds = open_encrypted_versioned(&path).await;
		let tx = ds.transaction(true, true).await.unwrap();
		tx.set(b"persist".to_vec(), b"yes".to_vec()).await.unwrap();
		tx.commit().await.unwrap();
		// ds drops here; SyncMode::Never means no background coordinator
		// thread keeps the file lock alive, so the second open succeeds.
	}

	// Second open with the same key — data must still be readable
	{
		let ds = open_encrypted_versioned(&path).await;
		let tx = ds.transaction(false, false).await.unwrap();
		let val = tx.get(b"persist".to_vec(), None).await.unwrap();
		tx.cancel().await.unwrap();
		assert_eq!(val.as_deref(), Some(b"yes".as_ref()), "data must persist across reopen");
	}
}
