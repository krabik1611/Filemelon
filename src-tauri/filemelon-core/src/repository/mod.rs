pub mod rules;
use sqlx::{migrate::Migrator, sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions}, SqlitePool};
use std::path::Path;
use crate::core::model::{RuleRun, timestamp};
pub struct Repository { pub rules: rules::RulesRepository, pub pool: SqlitePool }
// Embed SQL using the public MigrationSource API. No runtime source directory is needed.
#[derive(Debug)]
struct EmbeddedMigrations;
impl<'s> sqlx::migrate::MigrationSource<'s> for EmbeddedMigrations {
    fn resolve(self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<sqlx::migrate::Migration>, Box<dyn std::error::Error + Send + Sync>>> + Send + 's>> {
        Box::pin(async {
            use sqlx::migrate::{Migration, MigrationType};
            Ok(vec![
                Migration::new(1, "init".into(), MigrationType::Simple, include_str!("../../../migrations/0001_init.sql").into(), false),
                Migration::new(2, "rule engine".into(), MigrationType::Simple, include_str!("../../../migrations/0002_rule_engine.sql").into(), false),
                Migration::new(3, "cancelled runs".into(), MigrationType::Simple, include_str!("../../../migrations/0003_cancelled_runs.sql").into(), false),
                Migration::new(4, "rule order".into(), MigrationType::Simple, include_str!("../../../migrations/0004_rule_order.sql").into(), false),
            ])
        })
    }
}
impl Repository {
    pub async fn open(data_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let pool = SqlitePoolOptions::new().max_connections(5).connect_with(SqliteConnectOptions::new().filename(data_dir.join("app.db")).create_if_missing(true).foreign_keys(true).journal_mode(SqliteJournalMode::Wal).busy_timeout(std::time::Duration::from_secs(5))).await.map_err(|e| e.to_string())?;
        Migrator::new(EmbeddedMigrations).await.map_err(|e| e.to_string())?.run(&pool).await.map_err(|e| e.to_string())?;
        sqlx::query("UPDATE rule_runs SET status='interrupted', end_time=?, error='Application stopped during run' WHERE status='running'").bind(timestamp()).execute(&pool).await.map_err(|e| e.to_string())?;
        Ok(Self { rules: rules::RulesRepository::new(pool.clone()), pool })
    }
    pub async fn history(&self) -> Result<Vec<RuleRun>, String> {
        sqlx::query_as::<_, RuleRun>("SELECT * FROM rule_runs ORDER BY id DESC LIMIT 200").fetch_all(&self.pool).await.map_err(|e| e.to_string())
    }
}




