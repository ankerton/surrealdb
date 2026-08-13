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
| Revision | `ce78f485d` | Fixed — do not update without testing |

## How to reference

```toml
# In workspace Cargo.toml
surrealdb = { git = "https://github.com/ankerton/surrealdb", rev = "ce78f485d", features = ["kv-rocksdb", "kv-mem"] }

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
