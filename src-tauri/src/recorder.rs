use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::AudioRecorder;
use crate::cleanup::cleanup_text;
use crate::output::write_to_document;
use crate::paste::paste_text;
use crate::settings::Settings;
use crate::transcribe_local;
use crate::transcribe_groq;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum RecordingState {
    Ready,
    Recording,
    Transcribing,
}

fn update_overlay(app: &AppHandle, state: &RecordingState, output_mode: &str) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        let class = match state {
            RecordingState::Ready => "mic".to_string(),
            RecordingState::Recording => {
                let mode_class = match output_mode {
                    "document" => "recording-document",
                    "terminal" => "recording-terminal",
                    _ => "recording-clipboard",
                };
                format!("mic {}", mode_class)
            }
            RecordingState::Transcribing => "mic transcribing".to_string(),
        };
        let js = format!("document.getElementById('mic').className = '{}';", class);
        let _ = overlay.eval(&js);
    }
}

pub struct Recorder {
    state: Arc<Mutex<RecordingState>>,
    audio_recorder: Arc<Mutex<AudioRecorder>>,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(RecordingState::Ready)),
            audio_recorder: Arc::new(Mutex::new(AudioRecorder::new())),
        }
    }

    pub fn get_state(&self) -> RecordingState {
        self.state.lock().unwrap().clone()
    }

    pub fn start_recording(&self, app: &AppHandle, mic_name: &str, output_mode: &str) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        if *state != RecordingState::Ready {
            return Err("Already recording or transcribing".to_string());
        }

        let mut recorder = self.audio_recorder.lock().unwrap();
        recorder.start(mic_name)?;

        *state = RecordingState::Recording;
        let _ = app.emit("recording-state", RecordingState::Recording);
        update_overlay(app, &RecordingState::Recording, output_mode);
        Ok(())
    }

    pub async fn stop_and_transcribe(
        &self,
        app: &AppHandle,
        settings: &Settings,
        app_dir: &PathBuf,
    ) -> Result<String, String> {
        // Stop recording
        {
            let mut state = self.state.lock().unwrap();
            if *state != RecordingState::Recording {
                return Err("Not currently recording".to_string());
            }
            *state = RecordingState::Transcribing;
            let _ = app.emit("recording-state", RecordingState::Transcribing);
            update_overlay(app, &RecordingState::Transcribing, "");
        }

        // Ensure state always resets to Ready even on error paths.
        // This prevents the recorder from getting permanently stuck in Transcribing.
        let result = self.do_transcribe(app, settings, app_dir).await;

        // Always reset state
        {
            let mut state = self.state.lock().unwrap();
            *state = RecordingState::Ready;
            let _ = app.emit("recording-state", RecordingState::Ready);
            update_overlay(app, &RecordingState::Ready, "");
        }

        result
    }

    async fn do_transcribe(
        &self,
        app: &AppHandle,
        settings: &Settings,
        app_dir: &PathBuf,
    ) -> Result<String, String> {
        let temp_path = app_dir.join("temp_recording.wav");

        // Save audio
        {
            let mut recorder = self.audio_recorder.lock().unwrap();
            recorder.stop_and_save(&temp_path)?;
        }

        // Transcribe
        let raw_text = match settings.engine.as_str() {
            "local" => {
                let model_path = app_dir.join(transcribe_local::model_filename(&settings.whisper_model));
                transcribe_local::transcribe_local(app, &model_path, &temp_path).await
            }
            "cloud" => {
                transcribe_groq::transcribe_groq(&settings.groq_api_key, &temp_path).await
            }
            _ => Err(format!("Unknown engine: {}", settings.engine)),
        };

        // Cleanup temp file (best-effort, regardless of transcription success)
        let _ = std::fs::remove_file(&temp_path);

        let raw_text = raw_text?;

        // Clean up text
        let cleaned = cleanup_text(&raw_text);

        // Route output based on configured mode (non-fatal: log error but don't block state reset)
        if !cleaned.is_empty() {
            match settings.output_mode.as_str() {
                "document" => {
                    if let Err(e) = write_to_document(&settings.output_dir, &cleaned) {
                        eprintln!("[Typr] Document write failed: {}", e);
                    }
                }
                "terminal" => {
                    // Print to stdout (visible if launched from terminal)
                    println!("{}", cleaned);
                    // Also append to /tmp/typr-stream so other processes can tail -f it
                    let stream_path = std::path::Path::new("/tmp/typr-stream");
                    if let Err(e) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(stream_path)
                        .and_then(|mut f| {
                            use std::io::Write;
                            writeln!(f, "{}", cleaned)
                        })
                    {
                        eprintln!("[Typr] Stream write failed: {}", e);
                    }
                }
                _ => {
                    // clipboard (default)
                    if let Err(e) = paste_text(&cleaned) {
                        eprintln!("[Typr] Auto-paste failed (Accessibility permission?): {}", e);
                    }
                }
            }
        }

        Ok(cleaned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_ready() {
        let recorder = Recorder::new();
        assert_eq!(recorder.get_state(), RecordingState::Ready);
    }
}
