use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter};

pub struct StreamState {
    child: Arc<Mutex<Option<Child>>>,
}

impl StreamState {
    pub fn new() -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_streaming(&self) -> bool {
        self.child.lock().unwrap().is_some()
    }

    pub fn start(&self, app: &AppHandle, model_path: &str, step: u32, length: u32) -> Result<(), String> {
        let mut child_lock = self.child.lock().unwrap();
        if child_lock.is_some() {
            return Err("Already streaming".to_string());
        }

        let mut process = Command::new("/opt/homebrew/bin/whisper-stream")
            .args([
                "-m", model_path,
                "-l", "en",
                "--step", &step.to_string(),
                "--length", &length.to_string(),
                "--vad-thold", "0.0",
                "--keep-context",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start whisper-stream: {}", e))?;

        let stdout = process.stdout.take()
            .ok_or_else(|| "Failed to capture stdout".to_string())?;

        *child_lock = Some(process);
        drop(child_lock);

        let app_clone = app.clone();
        let child_arc = Arc::clone(&self.child);

        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(raw) => {
                        let clean = strip_ansi(&raw);
                        // Handle carriage-return in-place updates: take the last segment
                        let text = clean
                            .split('\r')
                            .filter(|s| !s.trim().is_empty())
                            .last()
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if !text.is_empty() {
                            let _ = app_clone.emit("stream-text", text.clone());
                            if let Ok(mut f) = OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open("/tmp/typr-stream")
                            {
                                let _ = writeln!(f, "{}", text);
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            *child_arc.lock().unwrap() = None;
            let _ = app_clone.emit("stream-stopped", ());
        });

        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut lock = self.child.lock().unwrap();
        if let Some(mut child) = lock.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }
}

fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.peek() {
                Some(&'[') => {
                    chars.next(); // consume '['
                    // Consume until ASCII letter (final byte of CSI sequence)
                    for c in chars.by_ref() {
                        if c.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                _ => {
                    chars.next(); // skip next char for other ESC sequences
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}
