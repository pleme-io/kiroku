//! Frecency tracking — launch/access history with time-decay scoring.
//!
//! Uses SeaORM entities for dual SQLite/Postgres support.
//! Formula: `count / (1 + days_since_last * 0.1)`

use std::collections::HashMap;

use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, QueryOrder, QuerySelect, Schema, Set};

/// Frecency record entity — tracks per-item frequency and recency.
pub mod record {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "frecency_records")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub item_id: String,
        pub access_count: i32,
        pub last_access_at: DateTime,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// Re-export the record model for external use.
pub use record::Model as FrecencyRecord;

/// Ensure the frecency table exists.
pub async fn ensure_table(db: &DatabaseConnection) -> Result<(), DbErr> {
    let builder = db.get_database_backend();
    let schema = Schema::new(builder);
    let mut stmt = schema.create_table_from_entity(record::Entity);
    stmt.if_not_exists();
    db.execute(builder.build(&stmt)).await?;
    Ok(())
}

/// Record an access: increment count + update timestamp.
pub async fn record_access(db: &DatabaseConnection, item_id: &str) -> Result<(), DbErr> {
    let existing = record::Entity::find_by_id(item_id).one(db).await?;

    if let Some(rec) = existing {
        let count = rec.access_count;
        let mut active: record::ActiveModel = rec.into();
        active.access_count = Set(count + 1);
        active.last_access_at = Set(chrono::Utc::now().naive_utc());
        active.update(db).await?;
    } else {
        let new = record::ActiveModel {
            item_id: Set(item_id.to_string()),
            access_count: Set(1),
            last_access_at: Set(chrono::Utc::now().naive_utc()),
        };
        record::Entity::insert(new).exec(db).await?;
    }

    Ok(())
}

/// Get access count and frecency score for an item.
pub async fn get_stats(db: &DatabaseConnection, item_id: &str) -> (u32, f64) {
    record::Entity::find_by_id(item_id)
        .one(db)
        .await
        .ok()
        .flatten()
        .map_or((0, 0.0), |m| {
            let frecency = calculate_frecency(m.access_count.cast_unsigned(), m.last_access_at);
            (m.access_count.cast_unsigned(), frecency)
        })
}

/// Get frecency scores for all tracked items.
pub async fn all_frecency(db: &DatabaseConnection) -> HashMap<String, f64> {
    record::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|m| {
            let f = calculate_frecency(m.access_count.cast_unsigned(), m.last_access_at);
            (m.item_id, f)
        })
        .collect()
}

/// Get the most recently accessed item IDs.
pub async fn recent_items(db: &DatabaseConnection, limit: usize) -> Vec<String> {
    record::Entity::find()
        .order_by_desc(record::Column::LastAccessAt)
        .limit(limit as u64)
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.item_id)
        .collect()
}

/// Calculate frecency: `count / (1 + days_since_last * 0.1)`
///
/// Recent frequent accesses score highest. Score decays with time:
/// - Accessed today with 10 accesses: ~10.0
/// - Accessed 10 days ago with 10 accesses: ~5.0
/// - Never accessed: 0.0
#[must_use]
pub fn calculate_frecency(access_count: u32, last_access: chrono::NaiveDateTime) -> f64 {
    let now = chrono::Utc::now().naive_utc();
    let days_ago = now.signed_duration_since(last_access).num_seconds().max(0) as f64 / 86400.0;
    f64::from(access_count) / (1.0 + days_ago * 0.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MetadataStore;
    use tempfile::TempDir;

    fn store_with_table() -> (TempDir, MetadataStore) {
        let dir = TempDir::new().unwrap();
        let store = MetadataStore::open_sqlite(dir.path().join("test.db")).unwrap();
        store.block_on(ensure_table(store.db())).unwrap();
        (dir, store)
    }

    #[test]
    fn record_and_get_stats() {
        let (_dir, store) = store_with_table();
        let db = store.db();

        store.block_on(record_access(db, "item.a")).unwrap();
        store.block_on(record_access(db, "item.a")).unwrap();

        let (count, frecency) = store.block_on(get_stats(db, "item.a"));
        assert_eq!(count, 2);
        assert!(frecency > 0.0);
    }

    #[test]
    fn unknown_item_returns_zero() {
        let (_dir, store) = store_with_table();
        let db = store.db();

        let (count, frecency) = store.block_on(get_stats(db, "nonexistent"));
        assert_eq!(count, 0);
        assert_eq!(frecency, 0.0);
    }

    #[test]
    fn all_frecency_multiple_items() {
        let (_dir, store) = store_with_table();
        let db = store.db();

        store.block_on(record_access(db, "item.a")).unwrap();
        store.block_on(record_access(db, "item.b")).unwrap();
        store.block_on(record_access(db, "item.b")).unwrap();

        let map = store.block_on(all_frecency(db));
        assert_eq!(map.len(), 2);
        assert!(map["item.b"] > map["item.a"]);
    }

    #[test]
    fn recent_items_ordering() {
        let (_dir, store) = store_with_table();
        let db = store.db();

        store.block_on(record_access(db, "item.old")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        store.block_on(record_access(db, "item.new")).unwrap();

        let recent = store.block_on(recent_items(db, 10));
        assert_eq!(recent[0], "item.new");
    }

    #[test]
    fn frecency_decays_with_time() {
        let now = chrono::Utc::now().naive_utc();
        let f = calculate_frecency(10, now);
        assert!(f > 9.0 && f <= 10.0, "expected ~10, got {f}");

        let ten_days_ago = now - chrono::TimeDelta::days(10);
        let f = calculate_frecency(10, ten_days_ago);
        assert!(f > 4.0 && f < 6.0, "expected ~5, got {f}");
    }

    #[test]
    fn ensure_table_idempotent() {
        let (_dir, store) = store_with_table();
        // Call again — should not error
        store.block_on(ensure_table(store.db())).unwrap();
    }
}
