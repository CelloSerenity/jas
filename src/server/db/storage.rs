use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use isideload::util::storage::SideloadingStorage;
use sqlx::SqlitePool;

pub struct DbStorage {
    prefix: String,
    pool: SqlitePool,
    cache: Arc<Mutex<HashMap<String, String>>>,
}

impl DbStorage {
    pub async fn load(pool: SqlitePool, prefix: &str) -> sqlx::Result<Self> {
        let pattern = format!("{}%", prefix);
        let rows = sqlx::query!(
            "SELECT key, value FROM sideload_storage WHERE key LIKE ?",
            pattern
        )
        .fetch_all(&pool)
        .await?;

        let cache: HashMap<String, String> = rows.into_iter().map(|r| (r.key, r.value)).collect();

        Ok(Self {
            prefix: prefix.to_string(),
            pool,
            cache: Arc::new(Mutex::new(cache)),
        })
    }

    fn scoped_key(&self, key: &str) -> String {
        format!("{}/{}", self.prefix, key)
    }

    fn write_through(&self, key: &str, value: Option<&str>) -> Result<(), rootcause::Report> {
        let pool = self.pool.clone();
        let key = key.to_string();
        let value = value.map(str::to_string);

        let join = std::thread::spawn(move || -> Result<(), String> {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("build runtime: {e}"))?;
            rt.block_on(async move {
                if let Some(v) = value {
                    sqlx::query!(
                        "INSERT INTO sideload_storage (key, value) VALUES (?, ?)
                         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                        key,
                        v
                    )
                    .execute(&pool)
                    .await
                    .map(|_| ())
                } else {
                    sqlx::query!("DELETE FROM sideload_storage WHERE key = ?", key)
                        .execute(&pool)
                        .await
                        .map(|_| ())
                }
                .map_err(|e| format!("sqlx: {e}"))
            })
        });

        join.join()
            .map_err(|_| rootcause::report!("DbStorage write thread panicked"))?
            .map_err(|e| rootcause::report!("DbStorage write-through failed: {e}"))?;

        Ok(())
    }
}

impl SideloadingStorage for DbStorage {
    fn store(&self, key: &str, value: &str) -> Result<(), rootcause::Report> {
        let scoped = self.scoped_key(key);
        {
            let mut cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
            cache.insert(scoped.clone(), value.to_string());
        }
        self.write_through(&scoped, Some(value))
    }

    fn retrieve(&self, key: &str) -> Result<Option<String>, rootcause::Report> {
        let scoped = self.scoped_key(key);
        let cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        Ok(cache.get(&scoped).cloned())
    }

    fn delete(&self, key: &str) -> Result<(), rootcause::Report> {
        let scoped = self.scoped_key(key);
        {
            let mut cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
            cache.remove(&scoped);
        }
        self.write_through(&scoped, None)
    }
}
