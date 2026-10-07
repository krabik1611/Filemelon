#[path = "transfer.rs"]
mod transfer;
pub use transfer::ImportResult;
use sqlx::{Row, SqlitePool};
use crate::core::model::{Action, Rule};
pub struct RulesRepository { pool: SqlitePool }
impl RulesRepository {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }
    pub async fn list(&self) -> Result<Vec<Rule>, String> {
        let rows = sqlx::query("SELECT r.*, a.action_type, a.destination_template FROM rules r LEFT JOIN rule_actions a ON a.id=(SELECT MIN(id) FROM rule_actions WHERE rule_id=r.id) ORDER BY r.id").fetch_all(&self.pool).await.map_err(|e| e.to_string())?;
        rows.iter().map(|r| Ok(Rule {
            id: Some(r.get("id")), name: r.get("name"), source: r.get("source"), cron: r.get("cron"), enabled: r.get::<i64,_>("enabled") != 0,
            condition: serde_json::from_str(r.get::<&str,_>("expr")).map_err(|e| format!("Rule {} has unsupported legacy conditions; migrate expr to JSON: {e}", r.get::<i64,_>("id")))?,
            action: match r.try_get::<i64,_>("action_type").map_err(|_| "Missing action")? { 0 => Action::Move, 1 => Action::Sort, _ => return Err("Unsupported action type".into()) },
            destination: r.try_get("destination_template").map_err(|e| e.to_string())?, min_age_seconds: r.get::<i64,_>("min_age_seconds") as u64,
        })).collect()
    }
    pub async fn save(&self, rule: &Rule) -> Result<i64, String> {
        rule.validate()?;
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        let expr = serde_json::to_string(&rule.condition).map_err(|e| e.to_string())?;
        let id = if let Some(id) = rule.id {
            let result = sqlx::query("UPDATE rules SET name=?,source=?,expr=?,cron=?,enabled=?,min_age_seconds=? WHERE id=?").bind(&rule.name).bind(&rule.source).bind(&expr).bind(&rule.cron).bind(rule.enabled).bind(rule.min_age_seconds as i64).bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
            if result.rows_affected() != 1 { return Err("Rule not found".into()); }
            sqlx::query("DELETE FROM rule_actions WHERE rule_id=?").bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?; id
        } else {
            sqlx::query("INSERT INTO rules(name,source,expr,cron,enabled,min_age_seconds) VALUES(?,?,?,?,?,?)").bind(&rule.name).bind(&rule.source).bind(&expr).bind(&rule.cron).bind(rule.enabled).bind(rule.min_age_seconds as i64).execute(&mut *tx).await.map_err(|e| e.to_string())?.last_insert_rowid()
        };
        sqlx::query("INSERT INTO rule_actions(rule_id,action_type,destination_template) VALUES(?,?,?)").bind(id).bind(if rule.action == Action::Move {0} else {1}).bind(&rule.destination).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        tx.commit().await.map_err(|e| e.to_string())?; Ok(id)
    }
    pub async fn delete_disabled(&self, id: i64) -> Result<(), String> {
        let result = sqlx::query("DELETE FROM rules WHERE id=? AND enabled=0")
            .bind(id).execute(&self.pool).await.map_err(|e| e.to_string())?;
        if result.rows_affected() != 1 { return Err("Only an existing disabled rule can be deleted".into()); }
        Ok(())
    }
    pub async fn set_enabled(&self, id: i64, enabled: bool) -> Result<(), String> {
        if enabled {
            let rules = self.list().await?;
            rules.iter().find(|rule|rule.id==Some(id)).ok_or("Rule not found")?.validate()?;
        }
        if sqlx::query("UPDATE rules SET enabled=? WHERE id=?").bind(enabled).bind(id).execute(&self.pool).await.map_err(|e| e.to_string())?.rows_affected() != 1 { return Err("Rule not found".into()); } Ok(())
    }
}



#[cfg(test)]
mod deletion_tests {
    use super::*;
    use crate::core::model::Condition;
    #[tokio::test]
    async fn deletion_requires_disabled_rule_and_cascades_related_rows() {
        let temp = tempfile::tempdir().unwrap();
        let repository = crate::repository::Repository::open(&temp.path().join("data")).await.unwrap();
        let rule = Rule {id:None,name:"Deletion test".into(),source:temp.path().to_string_lossy().into(),condition:Condition::All{conditions:vec![]},cron:"0 * * * *".into(),enabled:true,action:Action::Move,destination:temp.path().join("out").to_string_lossy().into(),min_age_seconds:0};
        let id = repository.rules.save(&rule).await.unwrap();
        sqlx::query("INSERT INTO rule_runs(rule_id,rule_name,trigger,start_time,status) VALUES(?,'Deletion test','manual','2026-10-07T00:00:00Z','success')").bind(id).execute(&repository.pool).await.unwrap();
        assert!(repository.rules.delete_disabled(id).await.is_err());
        assert_eq!(repository.rules.list().await.unwrap().len(),1);
        assert_eq!(repository.history().await.unwrap().len(),1);
        repository.rules.set_enabled(id,false).await.unwrap();
        // A stale UI cannot delete a rule enabled again before the request arrives.
        repository.rules.set_enabled(id,true).await.unwrap();
        assert!(repository.rules.delete_disabled(id).await.is_err());
        repository.rules.set_enabled(id,false).await.unwrap();
        repository.rules.delete_disabled(id).await.unwrap();
        assert!(repository.rules.list().await.unwrap().is_empty());
        assert!(repository.history().await.unwrap().is_empty());
        let count:i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rule_actions").fetch_one(&repository.pool).await.unwrap();
        assert_eq!(count,0);
        assert!(repository.rules.delete_disabled(id).await.is_err());
    }
}


