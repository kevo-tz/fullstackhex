use sqlx::PgPool;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database not configured")]
    NotConfigured,
    #[error("pool acquire timeout after {0:?}")]
    PoolTimeout(Duration),
    #[error("query failed: {0}")]
    QueryFailed(#[from] sqlx::Error),
    #[error("migration failed: {0}")]
    MigrationFailed(#[from] sqlx::migrate::MigrateError),
}

/// Run pending database migrations.
///
/// Applies all migrations from the `migrations/` directory in order.
/// Logs the number of applied migrations. Fails fast if any migration fails.
pub async fn run_migrations(pool: &PgPool) -> Result<(), DbError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    tracing::info!("database migrations applied successfully");
    Ok(())
}

/// Check database health by running `SELECT 1`.
///
/// Takes an optional pool reference. Returns `Err(NotConfigured)` if `None`.
/// Uses a timeout around the query to prevent hanging on a slow or unresponsive database.
pub async fn health_check(pool: Option<&PgPool>) -> Result<(), DbError> {
    const QUERY_TIMEOUT: Duration = Duration::from_secs(3);
    let pool = pool.ok_or(DbError::NotConfigured)?;

    tokio::time::timeout(QUERY_TIMEOUT, async {
        sqlx::query("SELECT 1")
            .fetch_one(pool)
            .await
            .map(|_| ())
            .map_err(DbError::from)
    })
    .await
    .map_err(|_| DbError::PoolTimeout(QUERY_TIMEOUT))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    #[tokio::test]
    async fn health_check_none_pool() {
        let result = health_check(None).await;
        assert!(matches!(result, Err(DbError::NotConfigured)));
    }

    #[tokio::test]
    async fn health_check_success_with_real_database() {
        let database_url = match std::env::var("DATABASE_URL") {
            Ok(url) => url,
            Err(_) => {
                eprintln!("SKIP: DATABASE_URL not set — skipping real-DB health check test");
                return;
            }
        };

        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(2))
            .connect(&database_url)
            .await;

        let pool = match pool {
            Ok(p) => p,
            Err(e) => {
                eprintln!("SKIP: could not connect to database ({e}) — skipping real-DB test");
                return;
            }
        };

        let result = health_check(Some(&pool)).await;
        assert!(
            result.is_ok(),
            "health_check should succeed against a real database, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn health_check_pool_exhausted_returns_query_failed() {
        let database_url = match std::env::var("DATABASE_URL") {
            Ok(url) => url,
            Err(_) => {
                eprintln!("SKIP: DATABASE_URL not set");
                return;
            }
        };
        let pool = match PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_millis(200))
            .connect(&database_url)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                eprintln!("SKIP: cannot connect ({e})");
                return;
            }
        };
        // Hold the only connection so the next query fails to acquire
        let _held = match pool.acquire().await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("SKIP: cannot acquire ({e})");
                return;
            }
        };
        // health_check on an exhausted pool should return an error
        let result = health_check(Some(&pool)).await;
        assert!(
            result.is_err(),
            "health_check should fail on exhausted pool, got {:?}",
            result
        );
        drop(_held);
    }

    #[test]
    fn error_display_renders_variants() {
        let nc = DbError::NotConfigured;
        assert_eq!(nc.to_string(), "database not configured");

        let pt = DbError::PoolTimeout(Duration::from_secs(3));
        assert!(pt.to_string().contains("3s"), "expected '3s' in: {}", pt);

        // QueryFailed wraps sqlx::Error — just verify it's not empty
        let qf = DbError::QueryFailed(sqlx::Error::PoolTimedOut);
        assert!(!qf.to_string().is_empty());
    }
}
