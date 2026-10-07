use std::{fs::{self, File, OpenOptions}, io::{self, Write}, path::PathBuf};

/// Reuse one append handle per day; keep per-operation writes and per-run disk sync.
pub(super) struct DailyLog {
    directory: PathBuf,
    current: Option<(String, File)>,
}

impl DailyLog {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory, current: None }
    }

    pub fn write(&mut self, value: &serde_json::Value) -> io::Result<()> {
        self.write_on(&chrono::Local::now().format("%Y-%m-%d").to_string(), value)
    }

    fn write_on(&mut self, day: &str, value: &serde_json::Value) -> io::Result<()> {
        if self.current.as_ref().is_none_or(|(current, _)| current != day) {
            self.sync()?;
            fs::create_dir_all(&self.directory)?;
            let file = OpenOptions::new().append(true).create(true)
                .open(self.directory.join(format!("operations-{day}.jsonl")))?;
            self.current = Some((day.to_owned(), file));
        }
        let mut record = serde_json::to_vec(value)?;
        record.push(b'\n');
        self.current.as_mut().expect("Log opened above").1.write_all(&record)
    }

    pub fn sync(&self) -> io::Result<()> {
        if let Some((_, file)) = &self.current { file.sync_data()?; }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_records_rotates_days_and_preserves_existing_log() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("operations-2026-10-07.jsonl");
        fs::write(&first, "{\"old\":true}\n").unwrap();
        let mut log = DailyLog::new(dir.path().to_owned());
        log.write_on("2026-10-07", &serde_json::json!({"value": 1})).unwrap();
        log.write_on("2026-10-07", &serde_json::json!({"value": 2})).unwrap();
        log.write_on("2026-10-08", &serde_json::json!({"value": 3})).unwrap();
        log.sync().unwrap();
        assert_eq!(fs::read_to_string(first).unwrap().lines().count(), 3);
        assert_eq!(fs::read_to_string(dir.path().join("operations-2026-10-08.jsonl")).unwrap(), "{\"value\":3}\n");
    }
}
