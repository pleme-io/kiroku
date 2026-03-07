//! Database CLI helpers — create tables, check status, reset.
//!
//! Generic helpers that work with any SeaORM entity.
//! Used by `tobira db migrate`, `hikyaku db migrate`, etc.

use sea_orm::entity::prelude::*;
use sea_orm::{Database, DatabaseConnection, Schema};

/// Database CLI operations for any set of entities.
pub struct DbCli;

impl DbCli {
    /// Connect to the database.
    pub async fn connect(database_url: &str) -> Result<DatabaseConnection, Box<dyn std::error::Error>> {
        let db = Database::connect(database_url).await?;
        Ok(db)
    }

    /// Create a table from a concrete entity (idempotent).
    pub async fn create_table<E>(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>>
    where
        E: EntityTrait,
    {
        let builder = db.get_database_backend();
        let schema = Schema::new(builder);
        let mut stmt = schema.create_table_from_entity(E::default());
        stmt.if_not_exists();
        db.execute(builder.build(&stmt)).await?;
        Ok(())
    }

    /// Drop and recreate a table from a concrete entity.
    pub async fn reset_table<E>(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>>
    where
        E: EntityTrait,
    {
        let builder = db.get_database_backend();

        let drop_stmt = sea_orm::sea_query::Table::drop()
            .table(E::default())
            .if_exists()
            .to_owned();
        db.execute(builder.build(&drop_stmt)).await?;

        let schema = Schema::new(builder);
        let create_stmt = schema.create_table_from_entity(E::default());
        db.execute(builder.build(&create_stmt)).await?;

        Ok(())
    }

    /// Count rows in a table.
    pub async fn count<E>(db: &DatabaseConnection) -> Result<u64, Box<dyn std::error::Error>>
    where
        E: EntityTrait,
        E::Model: Sync,
    {
        let count = E::find().count(db).await?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frecency::record;
    use tempfile::TempDir;

    #[tokio::test]
    async fn create_and_count() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let db = DbCli::connect(&url).await.unwrap();

        DbCli::create_table::<record::Entity>(&db).await.unwrap();

        let count = DbCli::count::<record::Entity>(&db).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn create_table_idempotent() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let db = DbCli::connect(&url).await.unwrap();

        DbCli::create_table::<record::Entity>(&db).await.unwrap();
        DbCli::create_table::<record::Entity>(&db).await.unwrap();
    }

    #[tokio::test]
    async fn reset_table_clears_data() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let db = DbCli::connect(&url).await.unwrap();

        DbCli::create_table::<record::Entity>(&db).await.unwrap();

        // Insert a record
        crate::frecency::record_access(&db, "test").await.unwrap();
        assert_eq!(DbCli::count::<record::Entity>(&db).await.unwrap(), 1);

        // Reset
        DbCli::reset_table::<record::Entity>(&db).await.unwrap();
        assert_eq!(DbCli::count::<record::Entity>(&db).await.unwrap(), 0);
    }
}
