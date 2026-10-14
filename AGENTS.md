# AGENTS.md — forge/surrealdb

## What this is

The Ankerton fork of SurrealDB. Adds AES-256 encryption at rest (RocksDB EncryptionProvider) and MVCC versioning (point-in-time reads via HLC timestamps). All stacks use this fork — never the upstream SurrealDB crate.

## Fork differences from upstream

| Feature | Upstream | This fork |
|---------|----------|-----------|
| Encryption at rest | No | AES-256 via RocksDB EncryptionProvider |
| MVCC versioning | No | `versioned=true` URL param — HLC-timestamped writes, point-in-time reads |
| Full-text planner: OR-of-MATCHES under an AND wrapper | Table scan + per-row MATCHES (O(table)) | Union of FullTextScans, O(matches) — PR #5 |
| Plan-time index analysis of bound params (`field @1@ $q`) | No candidates (table scan) | Params resolved per execution — PR #5 |
| kv-mem-only builds (`--no-default-features --features kv-mem`) | n/a | Compile fixed (encryption call gated on `kv-rocksdb`) — PR #4 |
| AES block cipher of the encrypted env | n/a | Per-thread context reuse, ~37 MiB/s cap gone (rust-rocksdb rev `712b3e41b`) — PR #7 |
| Closing an embedded datastore | Only by dropping every handle: unawaitable, and never done by a process that keeps its handle | `Surreal::<Db>::close().await` — stops taking requests, waits for those running, syncs the WAL, flushes memtables, stops background work, lets go of the files and the lock (the path reopens in-process); idempotent — PR #8 |
| Datastore shutdown with an index ever defined | Index builder ↔ its buildings' contexts form an `Arc` cycle: the datastore (and RocksDB, its lock) is never dropped | Builder cleared at shutdown; node delete failing no longer skips the engine's flush — PR #8 |
| RocksDB sizing per datastore | Process-wide only (`SURREAL_ROCKSDB_*`; server-sized from system memory); the versioned `default` column family ignored it and ran on RocksDB's own 64 MiB defaults | Query params `write_buffer_size`, `max_write_buffer_number`, `max_total_wal_size`, `block_cache_size` (bytes, `kb`/`mb`/`gb` suffixes) and SDK builders of the same names, applied to the versioned family too; unset = unchanged — PR #8 |
| Revision | `8580781b5` | Fixed — do not update without testing |

## How to reference

```toml
# In workspace Cargo.toml
surrealdb = { git = "https://github.com/ankerton/surrealdb", rev = "8580781b5", features = ["kv-rocksdb", "kv-mem"] }

[patch."https://github.com/ankerton/surrealdb"]
surrealdb = { path = "../../forge/surrealdb/surrealdb" }
```

The actual library crate is in the `surrealdb/` subdirectory of this repo, not at the repo root.

## Key constraints

- Always pin to the exact `rev` — floating refs will break reproducible builds
- Never update the rev without verifying the encrypted RocksDB and MVCC features still work
- The `kv-rocksdb` feature requires the `forge/rust-rocksdb` fork (see its AGENTS.md)

## Forge dependency

Uses `forge/rust-rocksdb` for the RocksDB bindings. See [`../rust-rocksdb/AGENTS.md`](../rust-rocksdb/AGENTS.md).
