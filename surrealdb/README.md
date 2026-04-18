# SurrealDB — Ankerton Fork

**GitHub:** https://github.com/ankerton/surrealdb

This is the Ankerton fork of SurrealDB. It adds two capabilities to the **RocksDB storage engine** that are not available anywhere in upstream SurrealDB — not on RocksDB, not on SurrealKV, not on any other backend:

- **Encryption at rest** — all data on disk is AES-256-CTR encrypted. Files are unreadable without the key. **This is only available with RocksDB.** SurrealKV and the in-memory engine do not support encryption at rest.
- **Point-in-time reads** — every write is timestamped with a Hybrid Logical Clock. You can query the database as it existed at any past moment. Available on both RocksDB (this fork) and SurrealKV.

If your application stores sensitive data and requires data-at-rest protection, RocksDB with encryption enabled is the only storage engine in this fork that provides it.

Everything else — SurrealQL, graph traversal, vector search, full-text search, multi-tenancy, live queries, the server, the Rust SDK — is unchanged from upstream SurrealDB.

---

## Table of Contents

1. [What SurrealDB can do](#what-surrealdb-can-do)
2. [Installation](#installation)
3. [Connecting to the database](#connecting-to-the-database)
   - [Embedded (inside your application)](#embedded-inside-your-application)
   - [Remote server](#remote-server)
4. [Querying with SurrealQL](#querying-with-surrealql)
   - [Basic CRUD](#basic-crud)
   - [Graph traversal](#graph-traversal)
   - [Vector similarity search](#vector-similarity-search)
   - [Full-text search](#full-text-search)
   - [Geospatial queries](#geospatial-queries)
   - [Live queries](#live-queries)
5. [Encryption at rest](#encryption-at-rest)
6. [Point-in-time reads](#point-in-time-reads)
7. [Durability and sync modes](#durability-and-sync-modes)
8. [Connection string reference](#connection-string-reference)
9. [Environment variables](#environment-variables)

---

## What SurrealDB can do

SurrealDB is a single database that covers workloads that normally require several specialised systems:

| Capability | How |
|---|---|
| Relational queries | SurrealQL — a SQL superset with JOINs, aggregations, subqueries |
| Document storage | Schemaless or schema-enforced records with nested objects and arrays |
| Graph traversal | First-class edge records; `->` / `<-` syntax for multi-hop traversal |
| Vector search | HNSW indexes; approximate nearest-neighbour queries in SurrealQL |
| Full-text search | BM25-ranked `SEARCH` indexes with highlighting |
| Geospatial queries | Point, line, polygon types; distance and containment predicates |
| Time-series | Time-ordered record IDs; range queries over time |
| Multi-tenancy | Namespaces and databases; row-level permissions |
| Real-time | Live queries push change notifications over WebSocket |
| Transactions | ACID across all tables and record types |
| Auth | Built-in user/role management; JWT-compatible access tokens |

---

## Installation

Add to `Cargo.toml`:

```toml
[dependencies]
surrealdb = { path = "../surrealdb", features = ["storage-rocksdb"] }
tokio    = { version = "1", features = ["rt-multi-thread", "macros"] }
serde    = { version = "1", features = ["derive"] }
```

For production embedded use, also enable the allocator:

```toml
surrealdb = { path = "../surrealdb", features = ["allocator", "storage-rocksdb"] }
```

Available storage features:

| Feature | Storage | Encryption at rest | Point-in-time reads |
|---|---|---|---|
| `storage-rocksdb` | RocksDB — persistent, recommended for production | **yes (this fork only)** | yes |
| `storage-surrealkv` | SurrealKV — persistent, lighter weight | **no** | yes |
| `storage-mem` | In-memory — no persistence, useful for tests | **no** | optional |

> **Only RocksDB supports encryption at rest.** If your deployment requires data-at-rest encryption, you must use `storage-rocksdb`.

Configure the Tokio runtime for embedded use — embedded SurrealDB benefits from multiple threads and a larger stack:

```rust
fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(10 * 1024 * 1024) // 10 MiB
        .build()
        .unwrap()
        .block_on(async {
            run().await.unwrap();
        });
}
```

---

## Connecting to the database

### Embedded (inside your application)

The database runs in the same process as your application. No separate server process is needed.

```rust
use surrealdb::Surreal;
use surrealdb::engine::local::RocksDb;

let db = Surreal::new::<RocksDb>("path/to/db").await?;

// Every connection must select a namespace and database before querying.
db.use_ns("myapp").use_db("production").await?;
```

Other embedded engines:

```rust
use surrealdb::engine::local::SurrealKv;
use surrealdb::engine::local::Mem;

// SurrealKV
let db = Surreal::new::<SurrealKv>("path/to/db").await?;

// Pure in-memory (data is lost on drop)
let db = Surreal::new::<Mem>(()).await?;
```

Choose the engine at runtime:

```rust
use surrealdb::engine::any::connect;

let db = connect("rocksdb:///path/to/db").await?;
let db = connect("surrealkv:///path/to/db").await?;
let db = connect("mem://").await?;
```

### Remote server

Start the server:

```bash
surreal start --log info --user root --pass root rocksdb:///data
```

Connect:

```rust
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

let db = Surreal::new::<Ws>("127.0.0.1:8000").await?;

db.signin(Root { username: "root", password: "root" }).await?;
db.use_ns("myapp").use_db("production").await?;
```

With TLS:

```rust
use surrealdb::engine::remote::ws::Wss;

let db = Surreal::new::<Wss>("db.example.com:443").await?;
```

Via HTTP instead of WebSocket:

```rust
use surrealdb::engine::remote::http::{Http, Https};

let db = Surreal::new::<Http>("127.0.0.1:8000").await?;
let db = Surreal::new::<Https>("db.example.com").await?;
```

---

## Querying with SurrealQL

### Basic CRUD

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct Person {
    name: String,
    age:  u32,
}

// Create — random ID
let alice: Option<Person> = db
    .create("person")
    .content(Person { name: "Alice".into(), age: 30 })
    .await?;

// Create — specific ID
let bob: Option<Person> = db
    .create(("person", "bob"))
    .content(Person { name: "Bob".into(), age: 25 })
    .await?;

// Read one
let alice: Option<Person> = db.select(("person", "alice")).await?;

// Read all
let everyone: Vec<Person> = db.select("person").await?;

// Update (merge — only the supplied fields are changed)
let updated: Option<Person> = db
    .update(("person", "bob"))
    .merge(serde_json::json!({ "age": 26 }))
    .await?;

// Replace (full document replace)
let replaced: Option<Person> = db
    .update(("person", "bob"))
    .content(Person { name: "Bob".into(), age: 26 })
    .await?;

// Delete
let _: Option<Person> = db.delete(("person", "bob")).await?;

// SurrealQL query with bound parameters
let results = db
    .query("SELECT * FROM person WHERE age > $min ORDER BY age ASC")
    .bind(("min", 18))
    .await?;

let people: Vec<Person> = results.take(0)?;
```

### Graph traversal

SurrealDB stores graph edges as records. Traverse them with `->` (outbound) and `<-` (inbound):

```rust
// Create nodes
db.query("CREATE user:alice SET name = 'Alice'").await?;
db.query("CREATE user:bob   SET name = 'Bob'").await?;
db.query("CREATE user:carol SET name = 'Carol'").await?;

// Create edges
db.query("RELATE user:alice ->follows-> user:bob").await?;
db.query("RELATE user:bob   ->follows-> user:carol").await?;

// Who does Alice follow?
let following = db
    .query("SELECT ->follows->user.name AS following FROM user:alice")
    .await?;

// Who follows Carol (reverse traversal)?
let followers = db
    .query("SELECT <-follows<-user.name AS followers FROM user:carol")
    .await?;

// Two-hop: people Alice follows who also follow Carol
let mutual = db
    .query("SELECT ->follows->user->follows->user.name AS names FROM user:alice")
    .await?;
```

Edges can carry their own data:

```rust
db.query(r#"
    RELATE user:alice ->knows-> user:bob
    SET since = time::now(), strength = 0.9
"#).await?;

// Query edge properties
db.query(r#"
    SELECT ->(knows WHERE strength > 0.5)->user.name AS close_friends
    FROM user:alice
"#).await?;
```

### Vector similarity search

Define a vector index on a field, then query by cosine or Euclidean distance:

```sql
-- Define a table with a vector field and an HNSW index
DEFINE TABLE article SCHEMALESS;
DEFINE FIELD embedding ON article TYPE array<float>;
DEFINE INDEX article_embedding ON article
    FIELDS embedding
    HNSW DIMENSION 1536 DIST COSINE;
```

```rust
// Insert a document with an embedding
db.query(r#"
    CREATE article SET
        title     = 'Introduction to Rust',
        embedding = $vec
"#)
.bind(("vec", my_embedding_vector))
.await?;

// Find the 5 nearest neighbours to a query vector
let results = db
    .query(r#"
        SELECT title, vector::similarity::cosine(embedding, $query) AS score
        FROM article
        WHERE embedding <|5,40|> $query
        ORDER BY score DESC
    "#)
    .bind(("query", query_vector))
    .await?;
```

The `<|k,ef|>` operator performs approximate nearest-neighbour search: `k` = number of results, `ef` = search beam width (higher = more accurate, slower).

### Full-text search

```sql
-- Define a full-text index
DEFINE INDEX article_title ON article
    FIELDS title
    SEARCH ANALYZER ascii BM25 HIGHLIGHTS;
```

```rust
let results = db
    .query(r#"
        SELECT title, search::highlight('<b>', '</b>', 1) AS highlighted
        FROM article
        WHERE title @1@ 'rust programming'
        ORDER BY search::score(1) DESC
        LIMIT 10
    "#)
    .await?;
```

`@1@` is the match operator — the number identifies which index to score against, useful when querying multiple indexed fields.

### Geospatial queries

SurrealDB natively handles GeoJSON-compatible types:

```rust
db.query(r#"
    CREATE city:london SET
        name     = 'London',
        location = (-0.1276, 51.5074)
"#).await?;

// Find cities within 100 km of a point
let nearby = db
    .query(r#"
        SELECT name
        FROM city
        WHERE geo::distance(location, (-0.1276, 51.5074)) < 100000
    "#)
    .await?;

// Containment: records whose location falls inside a polygon
let inside = db
    .query(r#"
        SELECT name FROM city
        WHERE geo::contains($area, location)
    "#)
    .bind(("area", geojson_polygon))
    .await?;
```

### Live queries

Live queries push change notifications to your application over the WebSocket connection whenever matching records are created, updated, or deleted:

```rust
use surrealdb::Action;

// Subscribe to all changes on the person table
let mut stream = db.select("person").live().await?;

// Subscribe to changes matching a condition
let mut stream = db
    .query("LIVE SELECT * FROM person WHERE age > 18")
    .await?
    .stream::<surrealdb::Notification<Person>>(0)?;

// Process notifications
while let Some(notification) = stream.next().await {
    let notification = notification?;
    match notification.action {
        Action::Create => println!("created: {:?}", notification.data),
        Action::Update => println!("updated: {:?}", notification.data),
        Action::Delete => println!("deleted: {:?}", notification.data),
        _ => {}
    }
}
```

---

## Encryption at rest

> This is an Ankerton fork addition. Upstream SurrealDB does not support encryption at rest.

When encryption is enabled, every byte written to disk — SST files, write-ahead log, manifest — is encrypted with AES-256-CTR. The files cannot be read without the correct key.

**The encryption key cannot be passed through a connection string.** It must be injected programmatically at startup. This requires working directly with the `Datastore` API from `surrealdb-core`:

```toml
[dependencies]
surrealdb-core = { path = "../surrealdb/core", features = ["kv-rocksdb"] }
```

```rust
use surrealdb_core::kvs::rocksdb::Datastore;
use surrealdb_core::kvs::config::{RocksDbConfig, SyncMode};
use surrealdb_core::kvs::api::Transactable;

// Load your key from a KMS, HSM, or secure vault — never hardcode it.
let key: [u8; 32] = load_key_from_vault();

let config = RocksDbConfig {
    encryption_key: Some(key), // AES-256-CTR; key is zeroed in memory after handoff
    versioned:      true,      // enable point-in-time reads (see below)
    sync_mode:      SyncMode::Every,
    retention_ns:   0,         // keep all versions forever
};

let ds = Datastore::new("path/to/db", config).await?;

// Write
let tx = ds.transaction(true, true).await?;
tx.set(b"hello".to_vec(), b"world".to_vec()).await?;
tx.commit().await?;

// Read
let tx = ds.transaction(false, false).await?;
let val = tx.get(b"hello".to_vec(), None).await?;
tx.cancel().await?;
```

**Key management notes:**
- Use a 32-byte (256-bit) key. The `[u8; 32]` type enforces this at compile time.
- The key is zeroed in Rust memory immediately after it is passed to C++.
- Load the key from a hardware security module (HSM) or cloud KMS (AWS KMS, GCP Cloud KMS, HashiCorp Vault). Never derive it from a password without a proper KDF (e.g. Argon2).
- Key rotation requires copying all data to a new database opened with the new key.
- Encryption provides confidentiality of data at rest. It does not protect against a compromised process that has the key loaded in memory.

---

## Point-in-time reads

> This is an Ankerton fork addition. Upstream SurrealDB does not support point-in-time reads on RocksDB.

When `versioned: true`, every committed write is stamped with a Hybrid Logical Clock (HLC) timestamp. All versions of each key are retained on disk. You can read the state of any key as it existed at any past timestamp.

```rust
use surrealdb_core::kvs::timestamp::HlcTimeStamp;

// Write version 1
let tx = ds.transaction(true, true).await?;
tx.set(b"config".to_vec(), b"v1".to_vec()).await?;
tx.commit().await?;

// Capture a timestamp — this marks the "snapshot point"
let snapshot = HlcTimeStamp::next().0;

// Write version 2
let tx = ds.transaction(true, true).await?;
tx.set(b"config".to_vec(), b"v2".to_vec()).await?;
tx.commit().await?;

// Read the latest version — returns "v2"
let tx = ds.transaction(false, false).await?;
let latest = tx.get(b"config".to_vec(), None).await?;

// Read at the snapshot — returns "v1"
let historical = tx.get(b"config".to_vec(), Some(snapshot)).await?;
tx.cancel().await?;
```

`HlcTimeStamp::next().0` returns a `u64` that encodes wall-clock milliseconds in the upper 48 bits and a logical counter in the lower 16 bits. It is always monotonically increasing — safe to use across threads.

**Version retention:** by default all versions are kept forever. Set `retention_ns` to allow the storage engine to discard versions older than the given duration:

```rust
use std::time::Duration;

let config = RocksDbConfig {
    versioned:    true,
    retention_ns: Duration::from_secs(30 * 86400).as_nanos() as u64, // 30 days
    ..RocksDbConfig::default()
};
```

Or via the connection string: `rocksdb:///path?versioned=true&retention=30d`

---

## Durability and sync modes

Controls when committed writes are flushed to disk:

| Mode | Meaning | Durability | Performance |
|---|---|---|---|
| `every` (default) | fsync after every commit | crash-safe | moderate |
| `never` | let the OS decide | data loss possible on crash | fastest |
| duration (e.g. `5s`) | background flush on interval | bounded loss window | high |

Set via query parameter:

```
rocksdb:///path?sync=every
rocksdb:///path?sync=never
rocksdb:///path?sync=500ms
```

Or via `RocksDbConfig::sync_mode`:

```rust
use surrealdb_core::kvs::config::SyncMode;
use std::time::Duration;

SyncMode::Every
SyncMode::Never
SyncMode::Interval(Duration::from_millis(500))
```

With `sync=every`, write throughput under concurrent load is maintained by a grouped commit coordinator: multiple transactions commit in parallel, and their WAL flushes are coalesced into a single `fsync`. Each writer still waits for that `fsync` to complete before its `commit()` returns, so full durability is preserved.

---

## Connection string reference

### RocksDB

```
rocksdb:///absolute/path?param=value&param=value
```

| Parameter | Values | Default | Description |
|---|---|---|---|
| `versioned` | `true` / `false` | `false` | Enable point-in-time reads |
| `sync` | `never` / `every` / duration | `every` | WAL flush mode |
| `retention` | duration string | `0` (unlimited) | How long to keep old versions |

### SurrealKV

```
surrealkv:///absolute/path?param=value
```

| Parameter | Values | Default | Description |
|---|---|---|---|
| `versioned` | `true` / `false` | `false` | Enable point-in-time reads |
| `sync` | `never` / `every` / duration | `every` | WAL flush mode |
| `retention` | duration string | `0` | How long to keep old versions |

### In-memory

```
mem://                           # no persistence
mem:///absolute/path?param=value # with persistence
```

| Parameter | Values | Default | Requires path |
|---|---|---|---|
| `versioned` | `true` / `false` | `false` | no |
| `sync` | `never` / `every` / duration | `never` | yes |
| `aol` | `never` / `sync` / `async` | `never` | yes |
| `snapshot` | `never` / duration > 30s | `never` | yes |
| `retention` | duration string | `0` | no |

### Duration format

`500us`, `200ms`, `5s`, `1m`, `2h`, `30d`

---

## Environment variables

### Datastore defaults (all engines)

| Variable | Default | Description |
|---|---|---|
| `SURREAL_DATASTORE_SYNC_DATA` | — | Override sync mode |
| `SURREAL_DATASTORE_VERSIONED` | `false` | Enable versioning |
| `SURREAL_DATASTORE_RETENTION` | `0` | Version retention duration |

### RocksDB performance tuning

| Variable | Default | Description |
|---|---|---|
| `SURREAL_ROCKSDB_THREAD_COUNT` | `#CPUs` | Flush and compaction threads |
| `SURREAL_ROCKSDB_BLOCK_CACHE_SIZE` | ≈ ½ RAM | LRU block cache size |
| `SURREAL_ROCKSDB_WRITE_BUFFER_SIZE` | dynamic | Memtable write buffer size |
| `SURREAL_ROCKSDB_MAX_WRITE_BUFFER_NUMBER` | dynamic | Max memtable count |
| `SURREAL_ROCKSDB_TARGET_FILE_SIZE_BASE` | `64 MiB` | L1 SST target size |
| `SURREAL_ROCKSDB_ENABLE_BLOB_FILES` | `true` | Offload large values to blob files |
| `SURREAL_ROCKSDB_MIN_BLOB_SIZE` | `4 KiB` | Minimum value size for blob offload |
| `SURREAL_ROCKSDB_BLOB_FILE_SIZE` | `256 MiB` | Target blob file size |
| `SURREAL_ROCKSDB_MAX_OPEN_FILES` | `1024` | Open file descriptor limit |
| `SURREAL_ROCKSDB_COMPACTION_STYLE` | `level` | `level` or `universal` |
| `SURREAL_ROCKSDB_STORAGE_LOG_LEVEL` | `warn` | RocksDB internal log verbosity |
| `SURREAL_ROCKSDB_SST_MAX_ALLOWED_SPACE_USAGE` | `0` | Disk space cap (0 = unlimited) |

### Grouped commit tuning (sync=every only)

| Variable | Default | Description |
|---|---|---|
| `SURREAL_ROCKSDB_GROUPED_COMMIT_TIMEOUT` | `5ms` | Max time to wait for a batch to fill |
| `SURREAL_ROCKSDB_GROUPED_COMMIT_WAIT_THRESHOLD` | `12` | Batch size that triggers wait |
| `SURREAL_ROCKSDB_GROUPED_COMMIT_MAX_BATCH_SIZE` | `4096` | Hard ceiling on batch size |
