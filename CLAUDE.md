# Kiroku (記録) — SeaORM Metadata Store

> **★★★ CSE / Knowable Construction.** This repo operates under **Constructive Substrate Engineering** — canonical specification at [`pleme-io/theory/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md`](https://github.com/pleme-io/theory/blob/main/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md). The Compounding Directive (operational rules: solve once, load-bearing fixes only, idiom-first, models stay current, direction beats velocity) is in the org-level pleme-io/CLAUDE.md ★★★ section. Read both before non-trivial changes.


## Build & Test

```bash
cargo build          # compile
cargo test           # 12 unit tests + 1 doc-test
```

## Architecture

SeaORM database connection wrapper with:
- Dual sync/async runtime support (owned vs external tokio)
- Frecency tracking (time-decay scoring for usage history)
- Database CLI helpers (create table, reset, count)

### Module Map

| Path | Purpose |
|------|---------|
| `src/lib.rs` | Re-exports + sea_orm/chrono |
| `src/store.rs` | `MetadataStore` — dual-runtime DB connection (3 tests) |
| `src/frecency.rs` | Frecency entity + async operations (6 tests) |
| `src/db.rs` | `DbCli` — table management helpers (3 tests) |

### Key Types

- **`MetadataStore`** — SeaORM connection with sync/async runtime
- **`DbCli`** — generic table creation, reset, count
- **`frecency::record`** — SeaORM entity for frecency tracking
- **`calculate_frecency(count, last_access)`** — `count / (1 + days * 0.1)`

### Dual Runtime

```rust
// Sync mode (GUI/standalone) — owns a tokio runtime
let store = MetadataStore::open_sqlite("/tmp/state.db")?;
store.block_on(frecency::ensure_table(store.db()))?;

// Async mode (daemon/gRPC) — uses caller's runtime
let store = MetadataStore::connect_async("postgres://...").await?;
frecency::ensure_table(store.db()).await?;
```

### Frecency API

All frecency functions are async, taking `&DatabaseConnection`:
- `ensure_table(db)` — create frecency_records table
- `record_access(db, item_id)` — increment count + update timestamp
- `get_stats(db, item_id)` — returns `(count, frecency_score)`
- `all_frecency(db)` — all items' frecency scores
- `recent_items(db, limit)` — most recently accessed items

### Database CLI

```rust
// Create table (idempotent)
DbCli::create_table::<frecency::record::Entity>(&db).await?;

// Count rows
let count = DbCli::count::<frecency::record::Entity>(&db).await?;

// Drop and recreate
DbCli::reset_table::<frecency::record::Entity>(&db).await?;
```

## Consumers

- **tobira** — app launcher frecency store
- **hikyaku** — email metadata store
