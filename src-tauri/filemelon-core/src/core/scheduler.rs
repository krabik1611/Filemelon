use super::{Core, model::{schedule, timestamp, Rule}};
use chrono::{DateTime, Local};
use std::{collections::HashMap, sync::Arc, time::Duration};
pub(super) type ScheduleCache = HashMap<i64, (String, Option<DateTime<Local>>)>;
#[derive(serde::Serialize)]
pub struct NextRun { pub rule_id:i64, pub time:Option<String>, pub waiting:bool, pub error:Option<String> }
pub(super) fn next_run(rule:&Rule, cache:&ScheduleCache, now:&DateTime<Local>) -> NextRun {
    let id=rule.id.unwrap();
    if !rule.enabled { return NextRun {rule_id:id,time:None,waiting:false,error:None}; }
    let time=match cache.get(&id).filter(|(expr,_)| expr==&rule.cron) {
        Some((_,time))=>*time,
        None=>match schedule(&rule.cron) {
            Ok(schedule)=>schedule.after(now).next(),
            Err(e)=>return NextRun {rule_id:id,time:None,waiting:false,error:Some(e)},
        },
    };
    NextRun {rule_id:id,time:time.map(|t| t.to_rfc3339()),waiting:time.is_some_and(|t|t<=*now),error:None}
}
// Parse only when the expression changes or its cached occurrence is due.
fn advance_schedule(rule: &Rule, cache: &mut ScheduleCache, now: &DateTime<Local>) -> bool {
    let id = rule.id.unwrap();
    let cached = cache.get(&id).filter(|(expr, _)| expr == &rule.cron);
    let due = cached.is_some_and(|(_, time)| time.is_some_and(|time| time <= *now));
    if cached.is_none() || due {
        let Ok(parsed) = schedule(&rule.cron) else { return false; };
        cache.insert(id, (rule.cron.clone(), parsed.after(now).next()));
    }
    due
}
pub async fn serve(core: Arc<Core>) {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let _guard = core.gate.lock().await;
        let rules = match core.repository.rules.list().await {
            Ok(rules) => rules,
            Err(e) => { let _ = core.log(serde_json::json!({"time":timestamp(),"status":"scheduler_error","error":e})); drop(_guard); tokio::time::sleep(Duration::from_secs(30)).await; continue; }
        };
        let now = Local::now(); let mut due = Vec::new();
        {
            let mut next=core.schedules.lock().unwrap_or_else(|e|e.into_inner());
            next.retain(|id, _| rules.iter().any(|r| r.id == Some(*id) && r.enabled));
            for rule in rules.iter().filter(|r| r.enabled) {
                let id=rule.id.unwrap();
                if advance_schedule(rule, &mut next, &now) { due.push(id); }
            }
        }
        if !due.is_empty() {
            if let Err(e)=core.execute(&rules,&due,"cron").await {let _=core.log(serde_json::json!({"time":timestamp(),"status":"scheduler_error","error":e}));}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::{Action,Condition};
    #[test]
    fn advances_due_occurrence_once_and_keeps_future_cache() {
        let rule=Rule{id:Some(1),name:String::new(),source:String::new(),condition:Condition::All{conditions:vec![]},cron:"0 * * * *".into(),enabled:true,action:Action::Move,destination:String::new(),min_age_seconds:0};
        let now=Local::now(); let mut cache=HashMap::new();
        assert!(!advance_schedule(&rule, &mut cache, &now));
        let future=cache[&1].1;
        assert!(!advance_schedule(&rule, &mut cache, &now));
        assert_eq!(cache[&1].1, future);
        cache.insert(1, (rule.cron.clone(), Some(now-chrono::Duration::seconds(1))));
        assert!(advance_schedule(&rule, &mut cache, &now));
        assert!(cache[&1].1.unwrap()>now);
        assert!(!advance_schedule(&rule, &mut cache, &now));
        let mut changed=rule.clone();changed.cron="*/5 * * * *".into();
        assert!(!advance_schedule(&changed, &mut cache, &now));
        assert_eq!(cache[&1].0, changed.cron);
    }
    #[test] fn next_run_uses_scheduler_cache_and_handles_disabled_rules() {
        let mut rule=Rule{id:Some(1),name:String::new(),source:String::new(),condition:Condition::All{conditions:vec![]},cron:"0 * * * *".into(),enabled:true,action:Action::Move,destination:String::new(),min_age_seconds:0};
        let now=Local::now(); let mut cache=HashMap::new();
        let next=next_run(&rule,&cache,&now);assert!(next.time.is_some());assert!(!next.waiting);
        let overdue=now-chrono::Duration::minutes(1);cache.insert(1,(rule.cron.clone(),Some(overdue)));
        let next=next_run(&rule,&cache,&now);assert!(next.waiting);assert_eq!(next.time,Some(overdue.to_rfc3339()));
        rule.enabled=false;assert!(next_run(&rule,&cache,&now).time.is_none());
        rule.enabled=true;rule.cron="invalid".into();assert!(next_run(&rule,&cache,&now).error.is_some());
    }
}


