use super::RulesRepository;
use crate::core::model::{Action, Rule};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleBundle { version: u32, rules: Vec<Rule> }
#[derive(Serialize)]
pub struct ImportResult { pub imported: usize }
impl RulesRepository {
    pub async fn export_json(&self) -> Result<String, String> {
        let mut rules = self.list().await?;
        for rule in &mut rules { rule.id = None; }
        serde_json::to_string_pretty(&RuleBundle { version:1, rules }).map_err(|e| e.to_string())
    }
    pub async fn import_json(&self, json: &str) -> Result<ImportResult, String> {
        if json.len() > 5 * 1024 * 1024 { return Err("JSON file exceeds 5 MiB".into()); }
        let mut bundle: RuleBundle = serde_json::from_str(json.trim_start_matches('\u{feff}')).map_err(|e| format!("Invalid rules JSON: {e}"))?;
        if bundle.version != 1 { return Err("Unsupported rules JSON version".into()); }
        if bundle.rules.len() > 1000 { return Err("A file can contain at most 1000 rules".into()); }
        for rule in &mut bundle.rules {
            rule.id = None; rule.enabled = false;
            rule.validate_import().map_err(|e| format!("Rule '{}': {e}",rule.name))?;
        }
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        for rule in &bundle.rules {
            let expr = serde_json::to_string(&rule.condition).map_err(|e| e.to_string())?;
            let id = sqlx::query("INSERT INTO rules(name,source,expr,cron,enabled,min_age_seconds) VALUES(?,?,?,?,0,?)").bind(&rule.name).bind(&rule.source).bind(expr).bind(&rule.cron).bind(rule.min_age_seconds as i64).execute(&mut *tx).await.map_err(|e| e.to_string())?.last_insert_rowid();
            sqlx::query("INSERT INTO rule_actions(rule_id,action_type,destination_template) VALUES(?,?,?)").bind(id).bind(if rule.action == Action::Move {0} else {1}).bind(&rule.destination).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        }
        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(ImportResult {imported:bundle.rules.len()})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{repository::Repository, core::model::Condition};
    #[tokio::test]
    async fn export_import_appends_disabled_rules_and_rejects_invalid_batch() {
        let temp=tempfile::tempdir().unwrap(); let repository=Repository::open(&temp.path().join("data")).await.unwrap();
        let rule=Rule {id:None,name:"Roundtrip".into(),source:temp.path().to_string_lossy().into(),condition:Condition::Extension{values:vec!["mp4".into()]},cron:"*/5 * * * *".into(),enabled:true,action:Action::Sort,destination:std::env::current_dir().unwrap().join("Sorted/{year}").to_string_lossy().into(),min_age_seconds:60};
        repository.rules.save(&rule).await.unwrap();
        let json=repository.rules.export_json().await.unwrap();
        let mut bundle:RuleBundle=serde_json::from_str(&json).unwrap(); assert!(bundle.rules[0].id.is_none());
        bundle.rules[0].source=temp.path().join("missing").to_string_lossy().into();
        assert_eq!(repository.rules.import_json(&serde_json::to_string(&bundle).unwrap()).await.unwrap().imported,1);
        let rules=repository.rules.list().await.unwrap(); assert_eq!(rules.len(),2); assert!(rules[0].enabled); assert!(!rules[1].enabled); assert_ne!(rules[0].id,rules[1].id); assert_eq!(rules[1].min_age_seconds,60); assert!(repository.rules.set_enabled(rules[1].id.unwrap(),true).await.is_err());
        bundle.rules.push(rule); bundle.rules[1].cron="invalid".into();
        assert!(repository.rules.import_json(&serde_json::to_string(&bundle).unwrap()).await.is_err()); assert_eq!(repository.rules.list().await.unwrap().len(),2);
        assert!(repository.rules.import_json(r#"{"version":99,"rules":[]}"#).await.is_err());
        assert!(repository.rules.import_json("bad JSON").await.is_err());
    }
}


