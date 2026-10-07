use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use std::{fs::Metadata, path::Path, str::FromStr, time::SystemTime};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    All { conditions: Vec<Condition> },
    Any { conditions: Vec<Condition> },
    Extension { values: Vec<String> },
    NameContains { value: String },
    Size { min: Option<u64>, max: Option<u64> },
    ModifiedAge { seconds: u64 },
    CreatedAge { seconds: u64 },
}
impl Condition {
    pub fn matches(&self, path: &Path, meta: &Metadata, now: SystemTime) -> bool {
        match self {
            Self::All { conditions } => conditions.iter().all(|c| c.matches(path, meta, now)),
            Self::Any { conditions } => conditions.iter().any(|c| c.matches(path, meta, now)),
            Self::Extension { values } => values.iter().any(|v| path.extension().unwrap_or_default().to_string_lossy().eq_ignore_ascii_case(v.trim_start_matches('.'))),
            Self::NameContains { value } => path.file_name().unwrap_or_default().to_string_lossy().to_lowercase().contains(&value.to_lowercase()),
            Self::Size { min, max } => min.is_none_or(|v| meta.len() >= v) && max.is_none_or(|v| meta.len() <= v),
            Self::ModifiedAge { seconds } => meta.modified().ok().and_then(|t| now.duration_since(t).ok()).is_some_and(|d| d.as_secs() >= *seconds),
            Self::CreatedAge { seconds } => meta.created().ok().and_then(|t| now.duration_since(t).ok()).is_some_and(|d| d.as_secs() >= *seconds),
        }
    }
    pub fn validate(&self, depth: usize) -> Result<(), String> {
        if depth > 16 { return Err("Condition nesting exceeds 16 levels".into()); }
        match self {
            Self::All { conditions } | Self::Any { conditions } => {
                if conditions.len() > 100 { return Err("Too many conditions".into()); }
                for c in conditions { c.validate(depth + 1)?; }
            }
            Self::Extension { values }
        if values.is_empty() || values.iter().any(|v| v.trim_matches('.').is_empty()) => return Err("Specify at least one extension".into()),
            Self::Size { min: Some(min), max: Some(max) }
        if min > max => return Err("Minimum size exceeds maximum".into()),
            _ => {}
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Action { Move, Sort }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: Option<i64>, pub name: String, pub source: String, pub condition: Condition,
    pub cron: String, pub enabled: bool, pub action: Action, pub destination: String,
    pub min_age_seconds: u64,
}
pub fn schedule(expr: &str) -> Result<cron::Schedule, String> {
    let parts: Vec<_> = expr.split_whitespace().collect();
    let normalized = if parts.len() == 5 { format!("0 {expr}") } else { expr.to_string() };
    if !(5..=7).contains(&parts.len()) { return Err("Cron requires 5, 6 or 7 fields".into()); }
    cron::Schedule::from_str(&normalized).map_err(|e| e.to_string())
}
impl Rule {
    pub fn validate(&self) -> Result<(), String> { self.validate_config(true) }
    pub fn validate_import(&self) -> Result<(), String> { self.validate_config(false) }
    fn validate_config(&self, require_source: bool) -> Result<(), String> {
        if self.name.trim().is_empty() { return Err("Rule name is required".into()); }
        if !Path::new(&self.source).is_absolute() || (require_source && !Path::new(&self.source).is_dir()) { return Err("Source must be an existing absolute directory".into()); }
        if !Path::new(&self.destination).is_absolute() { return Err("Destination must be absolute".into()); }
        if self.min_age_seconds > i64::MAX as u64 { return Err("File age is too large".into()); }
        self.condition.validate(0)?;
        let cron = schedule(&self.cron)?;
        if cron.upcoming(Local).next().is_none() { return Err("Schedule has no future execution".into()); }
        if self.action == Action::Sort { super::template::validate(&self.destination)?; }
        Ok(())
    }
}
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct RuleRun {
    pub id: i64, pub rule_id: i64, pub rule_name: String, pub trigger: String,
    pub start_time: String, pub end_time: Option<String>, pub status: String,
    pub processed: i64, pub skipped: i64, pub failed: i64, pub error: Option<String>,
}
pub fn timestamp() -> String { Utc::now().to_rfc3339() }
pub fn local_date(time: SystemTime) -> DateTime<Local> { DateTime::<Utc>::from(time).with_timezone(&Local) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn cron_accepts_standard_and_seconds_formats() {
        assert!(schedule("*/5 * * * *").is_ok()); assert!(schedule("0 */5 * * * *").is_ok()); assert!(schedule("invalid").is_err());
    }
    #[test] fn conditions_handle_case_groups_and_size() {
        let temp = tempfile::tempdir().unwrap(); let path = temp.path().join("Screenshot.JPG"); std::fs::write(&path, b"12345").unwrap(); let meta = path.metadata().unwrap();
        let condition = Condition::All { conditions: vec![Condition::Extension { values:vec![".jpg".into()] }, Condition::NameContains { value:"SCREEN".into() }, Condition::Size { min:Some(5), max:Some(5) }] };
        assert!(condition.matches(&path,&meta,SystemTime::now()));
        assert!(!Condition::Any { conditions:vec![] }.matches(&path,&meta,SystemTime::now()));
        assert!(Condition::Size { min:Some(10), max:Some(5) }.validate(0).is_err());
    }
}



