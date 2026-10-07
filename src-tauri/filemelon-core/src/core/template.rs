use super::model::local_date;
use chrono::{Datelike, Local};
use std::{fs::Metadata, path::Path};
const TOKENS: &[&str] = &["filename", "extension", "year", "month", "day", "created.year", "created.month", "created.day"];
pub fn validate(template: &str) -> Result<(), String> {
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        if rest[..start].contains('}') { return Err("Unmatched template brace".into()); }
        let end = rest[start..].find('}').ok_or("Unmatched template brace")? + start;
        if !TOKENS.contains(&&rest[start + 1..end]) { return Err(format!("Unknown template token: {}", &rest[start + 1..end])); }
        rest = &rest[end + 1..];
    }
    if rest.contains('}') { return Err("Unmatched template brace".into()); }
    Ok(())
}
pub fn render(template: &str, path: &Path, meta: &Metadata) -> Result<String, String> {
    validate(template)?;
    let now = Local::now();
    let mut result = template.to_string();
    for (token, value) in [
        ("filename", path.file_stem().unwrap_or_default().to_string_lossy().to_string()),
        ("extension", path.extension().unwrap_or_default().to_string_lossy().to_lowercase()),
        ("year", format!("{:04}", now.year())), ("month", format!("{:02}", now.month())), ("day", format!("{:02}", now.day())),
    ] { result = result.replace(&format!("{{{token}}}"), &value); }
    if result.contains("{created.") {
        let created = local_date(meta.created().map_err(|e| format!("Creation time unavailable: {e}"))?);
        for (token, value) in [("created.year", format!("{:04}", created.year())), ("created.month", format!("{:02}", created.month())), ("created.day", format!("{:02}", created.day()))] { result = result.replace(&format!("{{{token}}}"), &value); }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn rejects_unknown_and_unbalanced_tokens() {
        assert!(validate("{modified.year}").is_err());
        assert!(validate("{year").is_err());
        assert!(validate("x}{year}").is_err());
        assert!(validate("{created.year}/{extension}").is_ok());
    }
}

