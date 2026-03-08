//! Choubo (帳簿) — SeaORM metadata store with dual sync/async runtime.
//!
//! Provides a database connection wrapper that works in both sync contexts
//! (GUI/standalone — owns a tokio runtime) and async contexts (daemon/gRPC —
//! uses the caller's runtime). Includes helpers for frecency tracking and
//! database bootstrapping.
//!
//! # Quick Start
//!
//! ```no_run
//! use choubo::MetadataStore;
//!
//! // Sync mode (standalone/GUI) — owns a tokio runtime
//! let store = MetadataStore::open_sqlite("/tmp/state.db").unwrap();
//!
//! // Use store.db() for SeaORM operations
//! ```

mod db;
pub mod frecency;
mod store;

pub use db::DbCli;
pub use frecency::{FrecencyRecord, calculate_frecency};
pub use store::MetadataStore;

pub use sea_orm;
pub use chrono;
