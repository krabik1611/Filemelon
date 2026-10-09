pub mod files;
pub mod model;
pub mod template;
pub mod scheduler;
mod log;
use crate::repository::Repository;
use model::{Action, Rule, RuleRun, timestamp};
use std::{collections::{HashMap, HashSet}, fs, path::PathBuf, sync::{Arc, Mutex as StdMutex, atomic::{AtomicBool, Ordering}}, time::SystemTime};
use tokio::sync::Mutex;

#[derive(Clone, serde::Serialize)]
pub struct ActiveRunStatus { pub run_id: i64, pub rule_id: i64, pub rule_name: String, pub stopping: bool }
struct Running { run_id: i64, rule_id: i64, rule_name: String, cancel: Arc<AtomicBool> }
struct ActiveGuard { core: Arc<Core>, run_id: i64 }
impl Drop for ActiveGuard {
    fn drop(&mut self) {
        let mut active = self.core.active.lock().unwrap_or_else(|e| e.into_inner());
        if active.as_ref().is_some_and(|run| run.run_id == self.run_id) { *active = None; }
    }
}
pub struct Core { pub(crate) schedules: StdMutex<scheduler::ScheduleCache>, pub repository: Repository, pub gate: Mutex<()>, log: StdMutex<log::DailyLog>, active: StdMutex<Option<Running>> }
impl Core {
    pub async fn open(data_dir: PathBuf) -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self { repository: Repository::open(&data_dir).await?, gate: Mutex::new(()), log: StdMutex::new(log::DailyLog::new(data_dir.join("logs"))), active: StdMutex::new(None), schedules: StdMutex::new(HashMap::new()) }))
    }
    pub fn invalidate_schedule(&self, id: i64) { self.schedules.lock().unwrap_or_else(|e| e.into_inner()).remove(&id); }
    pub async fn next_runs(&self) -> Result<Vec<scheduler::NextRun>, String> {
        let rules = self.repository.rules.list().await?;
        let now = chrono::Local::now();
        let cache = self.schedules.lock().unwrap_or_else(|e| e.into_inner());
        Ok(rules.iter().map(|rule| scheduler::next_run(rule, &cache, &now)).collect())
    }
    pub fn active_run(&self) -> Option<ActiveRunStatus> {
        self.active.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|run| ActiveRunStatus { run_id:run.run_id,rule_id:run.rule_id,rule_name:run.rule_name.clone(),stopping:run.cancel.load(Ordering::Acquire) })
    }
    // Never take the execution gate here: Stop must be available during copying or hashing.
    pub fn cancel_run(&self, run_id: i64) -> Result<(), String> {
        let active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        let run = active.as_ref().filter(|run| run.run_id == run_id).ok_or("This run has already finished")?;
        run.cancel.store(true, Ordering::Release);
        Ok(())
    }
    fn activate(self: &Arc<Self>, run_id: i64, rule: &Rule, cancel: Arc<AtomicBool>) -> ActiveGuard {
        *self.active.lock().unwrap_or_else(|e| e.into_inner()) = Some(Running {run_id,rule_id:rule.id.unwrap(),rule_name:rule.name.clone(),cancel});
        ActiveGuard {core:self.clone(),run_id}
    }
    pub fn log(&self, value: serde_json::Value) -> Result<(), String> {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).write(&value).map_err(|e| e.to_string())
    }
    fn sync_log(&self) -> Result<(), String> {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).sync().map_err(|e| e.to_string())
    }
    pub async fn run_now(self: &Arc<Self>, id: i64) -> Result<Vec<RuleRun>, String> {
        let _lock = self.gate.lock().await;
        let rules = self.repository.rules.list().await?;
        let target = rules.iter().find(|r| r.id == Some(id)).ok_or("Rule not found")?;
        if !target.enabled { return Err("Enable the rule before running it".into()); }
        self.execute(&rules, &[id], "manual").await?;
        self.repository.history().await
    }
    async fn execute(self: &Arc<Self>, all: &[Rule], ids: &[i64], trigger: &str) -> Result<(), String> {
        let selected: Vec<_> = all.iter().filter(|r| r.enabled && ids.contains(&r.id.unwrap_or_default())).collect();
        let cancel = Arc::new(AtomicBool::new(false));
        let mut snapshots = None;
        let mut roots = HashMap::new();
        let mut claimed = HashSet::new();
        for rule in selected {
            if cancel.load(Ordering::Acquire) { break; }
            let run_id = sqlx::query("INSERT INTO rule_runs(rule_id,rule_name,trigger,start_time,status) VALUES(?,?,?,?,'running')").bind(rule.id).bind(&rule.name).bind(trigger).bind(timestamp()).execute(&self.repository.pool).await.map_err(|e| e.to_string())?.last_insert_rowid();
            let _active = self.activate(run_id, rule, cancel.clone());
            let mut processed = 0i64; let mut skipped = 0i64; let mut failed = 0i64; let mut errors = Vec::new();
            if snapshots.is_none() {
                let rules = all.to_vec(); let due = ids.to_vec(); let token = cancel.clone();
                let captured = tokio::task::spawn_blocking(move || {
                    let mut roots = HashMap::new(); let mut sources = HashMap::new(); let mut snapshots = HashMap::new();
                    for rule in rules.iter().filter(|r| r.enabled) {
                        if token.load(Ordering::Acquire) { break; }
                        roots.insert(rule.id.unwrap(), fs::canonicalize(&rule.source).map_err(|e| e.to_string()));
                        if !due.contains(&rule.id.unwrap()) { continue; }
                        let paths = sources.entry(rule.source.clone()).or_insert_with(|| {
                            let mut paths = Vec::new();
                            for entry in fs::read_dir(&rule.source).map_err(|e| e.to_string())? {
                                if token.load(Ordering::Acquire) { break; }
                                let entry = entry.map_err(|e| e.to_string())?;
                                if entry.file_type().map_err(|e| e.to_string())?.is_file() { paths.push(entry.path()); }
                            }
                            Ok::<_, String>(Arc::new(paths))
                        });
                        snapshots.insert(rule.id.unwrap(), paths.clone());
                    }
                    (roots, snapshots)
                }).await;
                match captured {
                    Ok((captured_roots, captured_paths)) => { roots = captured_roots; snapshots = Some(captured_paths); }
                    Err(e) => { failed += 1; errors.push(e.to_string()); snapshots = Some(HashMap::new()); }
                }
            }
            if !cancel.load(Ordering::Acquire) {
                let paths = snapshots.as_mut().unwrap().remove(&rule.id.unwrap()).unwrap_or_else(|| Err("Source scan failed".into()));
                let root = roots.get(&rule.id.unwrap()).cloned().unwrap_or_else(|| Err("Source is unavailable".into()));
                match paths.and_then(|paths| root.map(|root| (paths, root))) {
                    Err(e) => { failed += 1; errors.push(e); }
                    Ok((paths, root)) => {
                        for path in paths.iter() {
                            if cancel.load(Ordering::Acquire) { break; }
                            let meta = match fs::symlink_metadata(path) { Ok(m) if m.is_file() && !m.file_type().is_symlink() => m, _ => { skipped += 1; continue; } };
                            let canonical = root.join(path.file_name().ok_or("Invalid source filename")?);
                            if claimed.contains(&canonical) { skipped += 1; continue; }
                            // Resolve each source once per batch rather than once per file per rule.
                            let owner = all.iter().filter(|r| r.enabled).find(|r| roots.get(&r.id.unwrap()).and_then(|p| p.as_ref().ok()) == Some(&root) && r.condition.matches(path, &meta, SystemTime::now()));
                            if owner.and_then(|r| r.id) != rule.id || !files::stable(&meta, rule.min_age_seconds) { skipped += 1; continue; }
                            claimed.insert(canonical);
                            let destination = if rule.action == Action::Sort { template::render(&rule.destination, path, &meta) } else { Ok(rule.destination.clone()) };
                            let result = if rule.action == Action::Delete {
                                if let Err(e) = self.log(serde_json::json!({"time":timestamp(),"run_id":run_id,"rule_id":rule.id,"action":rule.action,"source":path,"status":"intent","destination":null})) { failed += 1; errors.push(format!("Logging intent failed: {e}")); break; }
                                let source = path.clone();
                                let recycle_source = source.clone();
                                tokio::task::spawn_blocking(move || files::recycle_file(&recycle_source)).await.map_err(|e| files::MoveError::Failed(e.to_string())).and_then(|r| r.map(|_| (source, "recycled")))
                            } else { match destination {
                                Ok(dir) => {
                                    if let Err(e) = self.log(serde_json::json!({"time":timestamp(),"run_id":run_id,"rule_id":rule.id,"action":rule.action,"source":path,"directory":dir,"status":"intent"})) { failed += 1; errors.push(format!("Logging intent failed: {e}")); break; }
                                    let age = rule.min_age_seconds; let source = path.clone(); let token = cancel.clone();
                                    tokio::task::spawn_blocking(move || files::move_file_cancellable(&source, &PathBuf::from(dir), age, &token)).await.map_err(|e| files::MoveError::Failed(e.to_string())).and_then(|result| result)
                                }
                                Err(e) => Err(files::MoveError::Failed(e)),
                            }};
                            let record = match result {
                                Ok((destination, kind)) => { if kind == "unchanged" { skipped += 1; } else { processed += 1; } serde_json::json!({"time":timestamp(),"run_id":run_id,"rule_id":rule.id,"action":rule.action,"source":path,"destination":destination,"status":kind}) }
                                Err(files::MoveError::Cancelled) => serde_json::json!({"time":timestamp(),"run_id":run_id,"rule_id":rule.id,"source":path,"status":"cancelled"}),
                                Err(files::MoveError::Failed(e)) => { failed += 1; if errors.len() < 20 { errors.push(e.clone()); } serde_json::json!({"time":timestamp(),"run_id":run_id,"rule_id":rule.id,"source":path,"status":"failed","error":e}) }
                            };
                            if let Err(e) = self.log(record) { failed += 1; errors.push(format!("Logging failed: {e}")); break; }
                        }
                    }
                }
            }
            if let Err(e) = self.sync_log() { failed += 1; errors.push(format!("Logging sync failed: {e}")); }
            let status = if cancel.load(Ordering::Acquire) { "cancelled" } else if failed == 0 { "success" } else if processed > 0 { "partial" } else { "failed" };
            sqlx::query("UPDATE rule_runs SET end_time=?,status=?,processed=?,skipped=?,failed=?,error=? WHERE id=?").bind(timestamp()).bind(status).bind(processed).bind(skipped).bind(failed).bind(if errors.is_empty() { None } else { Some(errors.join("; ")) }).bind(run_id).execute(&self.repository.pool).await.map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod integration_tests {
    use super::*;
    use model::{Action, Condition, Rule};
    use std::{fs::File, time::Duration};
    fn rule(source: &std::path::Path, destination: &std::path::Path) -> Rule {
        Rule { id:None,name:"Images".into(),source:source.to_string_lossy().into(),condition:Condition::Extension{values:vec!["txt".into()]},cron:"0 * * * *".into(),enabled:true,action:Action::Move,destination:destination.to_string_lossy().into(),min_age_seconds:0 }
    }
    #[tokio::test]
    async fn migrations_roundtrip_priority_and_history() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source"); fs::create_dir(&source).unwrap();
        let core = Core::open(temp.path().join("data")).await.unwrap();
        let first = core.repository.rules.save(&rule(&source,&temp.path().join("first"))).await.unwrap();
        let second = core.repository.rules.save(&rule(&source,&temp.path().join("second"))).await.unwrap();
        let file = source.join("a.txt"); fs::write(&file,b"hello").unwrap();
        File::options().write(true).open(&file).unwrap().set_times(fs::FileTimes::new().set_modified(SystemTime::now()-Duration::from_secs(10))).unwrap();
        core.run_now(second).await.unwrap(); assert!(file.exists());
        assert_eq!(core.repository.history().await.unwrap()[0].skipped,1);
        core.run_now(first).await.unwrap(); assert!(!file.exists()); assert!(temp.path().join("first/a.txt").exists());
        let history = core.repository.history().await.unwrap(); assert_eq!(history[0].processed,1); assert_eq!(history[0].status,"success"); assert!(history[0].end_time.is_some());
        core.repository.rules.set_enabled(first,false).await.unwrap(); assert!(core.run_now(first).await.is_err());
        let stored = core.repository.rules.list().await.unwrap(); assert_eq!(stored[1].id,Some(second));
        assert!(temp.path().join("data/logs").exists());
    }
    #[tokio::test]
    async fn invalid_rules_do_not_commit_and_stale_runs_are_recovered() {
        let temp = tempfile::tempdir().unwrap(); let data = temp.path().join("data");
        let core = Core::open(data.clone()).await.unwrap();
        let mut r = rule(temp.path(),&temp.path().join("out")); r.cron="invalid".into();
        assert!(core.repository.rules.save(&r).await.is_err()); assert!(core.repository.rules.list().await.unwrap().is_empty());
        r.cron="0 * * * *".into(); let id = core.repository.rules.save(&r).await.unwrap();
        sqlx::query("INSERT INTO rule_runs(rule_id,rule_name,trigger,start_time,status) VALUES(?,'test','manual',?,'running')").bind(id).bind(timestamp()).execute(&core.repository.pool).await.unwrap();
        core.repository.pool.close().await;
        let reopened = Core::open(data).await.unwrap(); assert_eq!(reopened.repository.history().await.unwrap()[0].status,"interrupted");
    }
}






#[cfg(test)]
mod cancellation_tests {
    use super::*;
    #[tokio::test]
    async fn stop_ignores_execution_gate_and_rejects_stale_run_ids() {
        let temp = tempfile::tempdir().unwrap(); let core = Core::open(temp.path().join("data")).await.unwrap();
        let rule = Rule {id:Some(7),name:"test".into(),source:String::new(),condition:model::Condition::All{conditions:vec![]},cron:"0 * * * *".into(),enabled:true,action:Action::Move,destination:String::new(),min_age_seconds:0};
        let token=Arc::new(AtomicBool::new(false));
        let guard=core.activate(11,&rule,token.clone());
        let _gate=core.gate.lock().await;
        assert!(core.cancel_run(10).is_err()); assert!(!token.load(Ordering::Acquire));
        core.cancel_run(11).unwrap(); assert!(token.load(Ordering::Acquire)); assert!(core.active_run().unwrap().stopping);
        drop(guard); assert!(core.active_run().is_none()); assert!(core.cancel_run(11).is_err());
        let fresh=Arc::new(AtomicBool::new(false)); let _next=core.activate(12,&rule,fresh.clone());
        assert!(core.cancel_run(11).is_err()); assert!(!fresh.load(Ordering::Acquire));
    }
    #[tokio::test]
    async fn cancelled_status_migration_preserves_existing_history() {
        use sqlx::{Connection, Row};
        let mut conn = sqlx::SqliteConnection::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql(include_str!("../../../migrations/0001_init.sql")).execute(&mut conn).await.unwrap();
        sqlx::raw_sql(include_str!("../../../migrations/0002_rule_engine.sql")).execute(&mut conn).await.unwrap();
        sqlx::raw_sql("INSERT INTO rules(name,source,expr,cron) VALUES('test','source','{}','0 * * * *'); INSERT INTO rule_runs(rule_id,rule_name,trigger,start_time,end_time,status,processed) VALUES(1,'test','manual','before','after','success',42);").execute(&mut conn).await.unwrap();
        sqlx::raw_sql(include_str!("../../../migrations/0003_cancelled_runs.sql")).execute(&mut conn).await.unwrap();
        let row = sqlx::query("SELECT * FROM rule_runs WHERE id=1").fetch_one(&mut conn).await.unwrap();
        assert_eq!(row.get::<String,_>("status"),"success"); assert_eq!(row.get::<i64,_>("processed"),42); assert_eq!(row.get::<String,_>("end_time"),"after");
        sqlx::query("INSERT INTO rule_runs(rule_id,rule_name,trigger,start_time,status) VALUES(1,'test','manual',?,'cancelled')").bind(timestamp()).execute(&mut conn).await.unwrap();
        let id: i64=sqlx::query_scalar("SELECT id FROM rule_runs WHERE status='cancelled'").fetch_one(&mut conn).await.unwrap(); assert_eq!(id,2);
    }
}







