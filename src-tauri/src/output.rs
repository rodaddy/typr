use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use chrono::Local;

/// Append a timestamped transcription entry to the daily document file.
///
/// Output format:
/// ```text
/// [2026-04-26 18:15:00] Hello this is a test transcription.
/// ```
/// File name: `typr-YYYY-MM-DD.txt` (one file per day, append mode).
pub fn write_to_document(output_dir: &str, text: &str) -> Result<(), String> {
    let dir = expand_tilde(output_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let now = Local::now();
    let date_str = now.format("%Y-%m-%d").to_string();
    let timestamp = now.format("%Y-%m-%d %H:%M:%S").to_string();

    let filename = dir.join(format!("typr-{}.txt", date_str));
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&filename)
        .map_err(|e| e.to_string())?;

    writeln!(file, "[{}] {}", timestamp, text).map_err(|e| e.to_string())
}

fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(&path[2..])
    } else {
        PathBuf::from(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/Documents/Typr/");
        assert!(expanded.to_string_lossy().contains("Documents/Typr"));
        assert!(!expanded.to_string_lossy().starts_with('~'));
    }

    #[test]
    fn test_expand_absolute_path() {
        let path = "/tmp/typr-test";
        let expanded = expand_tilde(path);
        assert_eq!(expanded, PathBuf::from(path));
    }

    #[test]
    fn test_write_to_document_creates_file() {
        let dir = temp_dir().join("typr_test_output");
        let _ = fs::remove_dir_all(&dir);

        let dir_str = dir.to_string_lossy().to_string();
        write_to_document(&dir_str, "Test transcription").unwrap();

        let date_str = Local::now().format("%Y-%m-%d").to_string();
        let expected_file = dir.join(format!("typr-{}.txt", date_str));
        assert!(expected_file.exists());

        let contents = fs::read_to_string(&expected_file).unwrap();
        assert!(contents.contains("Test transcription"));
        assert!(contents.starts_with('['));

        let _ = fs::remove_dir_all(&dir);
    }
}
