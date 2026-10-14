use std::fmt;
use std::time::Duration;

use surrealdb_core::kvs::config::{AolMode, SnapshotMode, format_duration};

use crate::engine::local::Db;
use crate::{Connect, Error};

impl<R> Connect<Db, R> {
	/// Enable MVCC versioning on the datastore.
	///
	/// Supported by `SurrealKv` and `Mem` engines.
	///
	/// # Examples
	///
	/// ```no_run
	/// # #[tokio::main]
	/// # async fn main() -> surrealdb::Result<()> {
	/// use surrealdb::Surreal;
	/// use surrealdb::engine::local::SurrealKv;
	///
	/// let db = Surreal::new::<SurrealKv>("path/to/database-folder").versioned().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn versioned(mut self) -> Self {
		self.address = self.address.map(|mut endpoint| {
			endpoint.append_query_param("versioned", "true");
			endpoint
		});
		self
	}

	/// Set the version retention period.
	///
	/// Determines how long old versions are kept before being garbage collected.
	/// A duration of zero means unlimited retention.
	///
	/// Supported by `SurrealKv` and `Mem` engines. Requires `versioned()`.
	///
	/// # Examples
	///
	/// ```no_run
	/// # #[tokio::main]
	/// # async fn main() -> surrealdb::Result<()> {
	/// use std::time::Duration;
	/// use surrealdb::Surreal;
	/// use surrealdb::engine::local::SurrealKv;
	///
	/// let db = Surreal::new::<SurrealKv>("path/to/database-folder")
	///     .versioned()
	///     .retention(Duration::from_secs(30 * 86400)) // 30 days
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn retention(mut self, duration: Duration) -> Self {
		self.address = self.address.map(|mut endpoint| {
			endpoint.append_query_param("retention", &format_duration(duration));
			endpoint
		});
		self
	}

	/// Set how many bytes a memtable (write buffer) of this datastore fills before it is
	/// flushed to a data file. Without it, the process-wide default applies
	/// (`SURREAL_ROCKSDB_WRITE_BUFFER_SIZE`, else sized from the system's memory: 128 MiB on a
	/// machine with 16 GiB or more).
	///
	/// Supported by the `RocksDb` engine. With the write-buffer number and the WAL cap below, it
	/// sizes an embedded datastore for an application rather than a server.
	///
	/// # Examples
	///
	/// ```no_run
	/// # #[tokio::main]
	/// # async fn main() -> surrealdb::Result<()> {
	/// use surrealdb::Surreal;
	/// use surrealdb::engine::local::RocksDb;
	///
	/// let db = Surreal::new::<RocksDb>("path/to/database-folder")
	///     .write_buffer_size(16 << 20)
	///     .max_write_buffer_number(2)
	///     .max_total_wal_size(64 << 20)
	///     .block_cache_size(64 << 20)
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn write_buffer_size(mut self, bytes: usize) -> Self {
		self.address = self.address.map(|mut endpoint| {
			endpoint.append_query_param("write_buffer_size", &bytes.to_string());
			endpoint
		});
		self
	}

	/// Set how many memtables of this datastore may exist at once, the one being written
	/// included. Without it, the process-wide default applies
	/// (`SURREAL_ROCKSDB_MAX_WRITE_BUFFER_NUMBER`, else 2 to 32 by the system's memory).
	///
	/// Supported by the `RocksDb` engine.
	pub fn max_write_buffer_number(mut self, count: usize) -> Self {
		self.address = self.address.map(|mut endpoint| {
			endpoint.append_query_param("max_write_buffer_number", &count.to_string());
			endpoint
		});
		self
	}

	/// Cap this datastore's live write-ahead log: once its files together pass `bytes`, the
	/// memtables holding the oldest of it are flushed so it can be dropped. That bounds what an
	/// unclean exit leaves for the next open to replay. Zero lets RocksDB choose (four times the
	/// write buffers), as it does without this.
	///
	/// Supported by the `RocksDb` engine.
	pub fn max_total_wal_size(mut self, bytes: u64) -> Self {
		self.address = self.address.map(|mut endpoint| {
			endpoint.append_query_param("max_total_wal_size", &bytes.to_string());
			endpoint
		});
		self
	}

	/// Set the size in bytes of this datastore's block cache, which its write buffers are also
	/// charged to. Without it, the process-wide default applies
	/// (`SURREAL_ROCKSDB_BLOCK_CACHE_SIZE`, else half the system's memory less 1 GiB).
	///
	/// Supported by the `RocksDb` engine.
	pub fn block_cache_size(mut self, bytes: usize) -> Self {
		self.address = self.address.map(|mut endpoint| {
			endpoint.append_query_param("block_cache_size", &bytes.to_string());
			endpoint
		});
		self
	}

	/// Set the disk sync mode.
	///
	/// Controls how and when data is flushed to disk. Supported by `SurrealKv`,
	/// `RocksDb`, and `Mem` engines (the `Mem` engine requires a persist path,
	/// e.g. `Surreal::new::<Mem>("/tmp/data")` or `connect("mem:///tmp/data")`,
	/// for sync to take effect).
	///
	/// The `mode` argument can be any type that implements `Display`. The
	/// canonical type is `SyncMode`:
	///
	/// - `SyncMode::Never` - leave flushing to the OS (fastest, least durable).
	/// - `SyncMode::Every` - sync on every transaction commit (fast, most durable).
	/// - `SyncMode::Interval(duration)` - periodic background flushing (fast, less durable).
	///
	/// # Examples
	///
	/// ```no_run
	/// # #[tokio::main]
	/// # async fn main() -> surrealdb::Result<()> {
	/// use surrealdb::Surreal;
	/// use surrealdb::engine::local::SurrealKv;
	/// use surrealdb_core::kvs::config::SyncMode;
	///
	/// let db = Surreal::new::<SurrealKv>("path/to/database-folder")
	///     .sync(SyncMode::Every)
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// ```no_run
	/// # #[tokio::main]
	/// # async fn main() -> surrealdb::Result<()> {
	/// use std::time::Duration;
	/// use surrealdb::Surreal;
	/// use surrealdb::engine::local::RocksDb;
	/// use surrealdb_core::kvs::config::SyncMode;
	///
	/// let db = Surreal::new::<RocksDb>("path/to/database-folder")
	///     .sync(SyncMode::Interval(Duration::from_millis(200)))
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn sync(mut self, mode: impl fmt::Display) -> Self {
		self.address = self.address.map(|mut endpoint| {
			endpoint.append_query_param("sync", &mode.to_string());
			endpoint
		});
		self
	}

	/// Set the AOL (Append-Only Log) mode for the `Mem` engine.
	///
	/// Requires a persist path (e.g. `Surreal::new::<Mem>("/tmp/data")` or
	/// `connect("mem:///tmp/data")`).
	///
	/// - `MemAolMode::Never` - never use AOL (default).
	/// - `MemAolMode::Sync` - write synchronously to AOL on every commit.
	/// - `MemAolMode::Async` - write asynchronously to AOL after commit.
	pub fn aol(mut self, mode: AolMode) -> Self {
		self.address = self.address.and_then(|mut endpoint| match endpoint.url.scheme() {
			"mem" => {
				endpoint.append_query_param("aol", &mode.to_string());
				Ok(endpoint)
			}
			scheme => Err(Error::internal(format!(
				"The 'aol' option is only supported by the 'mem' engine, not '{scheme}'"
			))),
		});
		self
	}

	/// Set the snapshot interval for the `Mem` engine.
	///
	/// Requires a persist path. Periodic snapshots are created at the given interval.
	///
	/// - `SnapshotMode::Never` - never use snapshots (default).
	/// - `SnapshotMode::Interval(duration)` - take snapshots at the given interval.
	pub fn snapshot(mut self, mode: SnapshotMode) -> Self {
		self.address = self.address.and_then(|mut endpoint| match endpoint.url.scheme() {
			"mem" => {
				endpoint.append_query_param("snapshot", &mode.to_string());
				Ok(endpoint)
			}
			scheme => Err(Error::internal(format!(
				"The 'snapshot' option is only supported by the 'mem' engine, not '{scheme}'"
			))),
		});
		self
	}
}
