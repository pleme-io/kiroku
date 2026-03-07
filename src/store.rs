//! Dual-runtime database connection wrapper.
//!
//! Two modes:
//! - **Sync**: `open_sqlite()` / `connect()` — creates an owned tokio runtime
//! - **Async**: `connect_async()` — uses the caller's existing runtime

use std::future::Future;
use std::path::Path;

use sea_orm::{Database, DatabaseConnection};

enum Runtime {
    Owned(tokio::runtime::Runtime),
    External,
}

/// A SeaORM database connection with dual sync/async runtime support.
///
/// In sync mode (GUI/standalone), owns a tokio runtime and blocks on async ops.
/// In async mode (daemon/gRPC), uses `block_in_place` on the caller's runtime.
pub struct MetadataStore {
    db: DatabaseConnection,
    rt: Runtime,
}

impl MetadataStore {
    /// Open a SQLite database at the given path (sync, owns runtime).
    pub fn open_sqlite(db_path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let db_url = format!("sqlite://{}?mode=rwc", db_path.as_ref().display());
        Self::connect(&db_url)
    }

    /// Connect to any SeaORM-supported database (sync, owns runtime).
    pub fn connect(database_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let rt = tokio::runtime::Runtime::new()?;
        let db = rt.block_on(Database::connect(database_url))?;
        Ok(Self {
            db,
            rt: Runtime::Owned(rt),
        })
    }

    /// Connect to any SeaORM-supported database (async, uses caller's runtime).
    pub async fn connect_async(
        database_url: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let db = Database::connect(database_url).await?;
        Ok(Self {
            db,
            rt: Runtime::External,
        })
    }

    /// Get the underlying `DatabaseConnection` for direct SeaORM operations.
    #[must_use]
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Run a future, blocking if sync mode or using `block_in_place` if async.
    pub fn block_on<F: Future>(&self, f: F) -> F::Output {
        match &self.rt {
            Runtime::Owned(rt) => rt.block_on(f),
            Runtime::External => tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(f)
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn open_sqlite_creates_db() {
        let dir = TempDir::new().unwrap();
        let store = MetadataStore::open_sqlite(dir.path().join("test.db")).unwrap();
        // Should be able to get the connection
        let _db = store.db();
    }

    #[test]
    fn connect_sqlite_url() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let store = MetadataStore::connect(&url).unwrap();
        let _db = store.db();
    }

    #[tokio::test]
    async fn connect_async_sqlite() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let store = MetadataStore::connect_async(&url).await.unwrap();
        let _db = store.db();
    }
}
