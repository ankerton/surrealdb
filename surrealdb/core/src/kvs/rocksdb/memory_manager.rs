use std::sync::Arc;

use rocksdb::{BlockBasedOptions, Cache, Options, WriteBufferManager};

use super::{TARGET, cnf};
use crate::kvs::Result;
use crate::kvs::config::RocksDbSizing;
use crate::mem::{MemoryReporter, cleanup_memory_reporters, register_memory_reporter};

pub(super) struct MemoryManager {
	/// The write buffer manager
	write_buffer_manager: WriteBufferManager,
	/// The RocksDB block cache
	cache: Cache,
}

impl MemoryReporter for MemoryManager {
	fn memory_allocated(&self) -> usize {
		self.write_buffer_manager.get_usage() + self.cache.get_usage()
	}
}

impl MemoryManager {
	/// Pre-configure the memory manager: this datastore's `sizing` where it says, the
	/// process-wide defaults where it does not.
	pub(super) fn configure(opts: &mut Options, sizing: &RocksDbSizing) -> Result<Self> {
		// Get the configuration options
		let block_cache_size = sizing.block_cache_size.unwrap_or(*cnf::ROCKSDB_BLOCK_CACHE_SIZE);
		let write_buffer_size = sizing.write_buffer_size.unwrap_or(*cnf::ROCKSDB_WRITE_BUFFER_SIZE);
		let write_buffer_number =
			sizing.max_write_buffer_number.unwrap_or(*cnf::ROCKSDB_MAX_WRITE_BUFFER_NUMBER);
		let total_write_buffer_size = write_buffer_number.saturating_mul(write_buffer_size);
		let max_write_buffer_number = write_buffer_number.min(i32::MAX as usize) as i32;
		// A flush waits for this many memtables, so it must stay below the number that may exist
		// (RocksDB lowers it to that itself); with the defaults this is unchanged
		let min_write_buffers_to_merge = (*cnf::ROCKSDB_MIN_WRITE_BUFFER_NUMBER_TO_MERGE)
			.min(write_buffer_number.saturating_sub(1))
			.max(1)
			.min(i32::MAX as usize) as i32;
		let total_memory_limit = total_write_buffer_size + block_cache_size;
		// Set the block cache size in bytes
		info!(target: TARGET, "Memory manager: block cache size: {block_cache_size}B");
		// Set the amount of data to build up in memory
		info!(target: TARGET, "Memory manager: write buffer size: {write_buffer_size}B");
		opts.set_write_buffer_size(write_buffer_size);
		// Set the maximum number of write buffers
		info!(target: TARGET, "Memory manager: maximum write buffers: {max_write_buffer_number}");
		opts.set_max_write_buffer_number(max_write_buffer_number);
		// Set minimum number of write buffers to merge
		info!(target: TARGET, "Memory manager: minimum write buffers to merge: {min_write_buffers_to_merge}");
		opts.set_min_write_buffer_number_to_merge(min_write_buffers_to_merge);
		// Combine the cache and the write buffers to get the memory limit
		info!(target: TARGET, "Memory manager: total memory limit: {total_memory_limit}");
		// Configure the in-memory cache options
		let cache = Cache::new_lru_cache(block_cache_size);
		// Configure the database with the cache
		opts.set_block_based_table_factory(&Self::table_options(&cache));
		opts.set_blob_cache(&cache);
		opts.set_row_cache(&cache);
		// Create a new write buffer manager with the cache
		let write_buffer_manager = WriteBufferManager::new_write_buffer_manager_with_cache(
			total_memory_limit,
			true,
			cache.clone(),
		);
		// Set the write buffer manager in the options
		opts.set_write_buffer_manager(&write_buffer_manager);
		// Continue
		Ok(Self {
			write_buffer_manager,
			cache,
		})
	}

	/// The block based file options, reading through `cache`
	fn table_options(cache: &Cache) -> BlockBasedOptions {
		let mut block = BlockBasedOptions::default();
		block.set_pin_l0_filter_and_index_blocks_in_cache(true);
		block.set_pin_top_level_index_and_filter(true);
		block.set_bloom_filter(10.0, false);
		block.set_block_size(*cnf::ROCKSDB_BLOCK_SIZE);
		block.set_block_cache(cache);
		block
	}

	/// Size a column family opened with options of its own — the versioned datastore's
	/// "default" family, which carries the timestamp comparator — as `sizing` says. The
	/// datastore-wide options set in [`Self::configure`] do not reach such a family: without
	/// this, its memtables and its block cache are RocksDB's own defaults whatever was asked
	/// for. Only what `sizing` sets is applied, so a datastore without sizing keeps exactly the
	/// family options it had.
	pub(super) fn configure_cf(&self, cf_opts: &mut Options, sizing: &RocksDbSizing) {
		if let Some(size) = sizing.write_buffer_size {
			info!(target: TARGET, "Memory manager: column family write buffer size: {size}B");
			cf_opts.set_write_buffer_size(size);
		}
		if let Some(count) = sizing.max_write_buffer_number {
			let count = count.min(i32::MAX as usize) as i32;
			info!(target: TARGET, "Memory manager: column family maximum write buffers: {count}");
			cf_opts.set_max_write_buffer_number(count);
		}
		if sizing.block_cache_size.is_some() {
			info!(target: TARGET, "Memory manager: column family reads through the block cache");
			cf_opts.set_block_based_table_factory(&Self::table_options(&self.cache));
			cf_opts.set_blob_cache(&self.cache);
		}
	}

	// Register the memory manager with the global allocator tracker
	pub(super) fn register_with_allocator_tracker(self: &Arc<Self>) {
		// Downgrade the memory manager to a memory reporter
		let reporter: Arc<dyn MemoryReporter> = self.clone();
		// Register with the global allocator tracker
		register_memory_reporter("rocksdb", Arc::downgrade(&reporter));
	}

	/// Shutdown the memory manager
	pub fn shutdown(&self) -> Result<()> {
		// Clean up the memory manager
		cleanup_memory_reporters();
		// All good
		Ok(())
	}
}
