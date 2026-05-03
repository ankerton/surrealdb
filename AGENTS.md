# AGENTS.md — forge/surrealdb

## What this is

The Ankerton fork of SurrealDB. Adds AES-256 encryption at rest (RocksDB EncryptionProvider) and MVCC versioning (point-in-time reads via HLC timestamps). All stacks use this fork — never the upstream SurrealDB crate.

## Fork differences from upstream

| Feature | Upstream | This fork |
|---------|----------|-----------|
| Encryption at rest | No | AES-256 via RocksDB EncryptionProvider |
| MVCC versioning | No | `versioned=true` URL param — HLC-timestamped writes, point-in-time reads |
| Revision | `c52649c29` | Fixed — do not update without testing |

## How to reference

```toml
# In workspace Cargo.toml
surrealdb = { git = "https://github.com/ankerton/surrealdb", rev = "c52649c29", features = ["kv-rocksdb", "kv-mem"] }

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
