#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use typr_lib::audio;
use typr_lib::downloader;
use typr_lib::recorder::{Recorder, RecordingState};
use typr_lib::settings::Settings;
use typr_lib::stream::StreamState;
use typr_lib::transcribe_local;

struct AppState {
    recorder: Recorder,
    settings: Mutex<Settings>,
    app_dir: PathBuf,
    stream: StreamState,
    // Set in setup() after tray menu is built
    toggle_item: Mutex<Option<MenuItem<tauri::Wry>>>,
}

fn get_app_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.typr.app")
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(state: State<AppState>, settings: Settings) -> Result<(), String> {
    settings.save(&state.app_dir)?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
fn list_microphones() -> Vec<audio::MicDevice> {
    audio::list_microphones()
}

#[tauri::command]
fn get_recording_state(state: State<AppState>) -> RecordingState {
    state.recorder.get_state()
}

#[tauri::command]
fn check_model_downloaded(state: State<AppState>, model_size: String) -> bool {
    let model_file = transcribe_local::model_filename(&model_size);
    state.app_dir.join(&model_file).exists()
}

#[tauri::command]
async fn download_model(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    model_size: String,
) -> Result<(), String> {
    let url = transcribe_local::model_download_url(&model_size);
    let model_file = transcribe_local::model_filename(&model_size);
    let dest = state.app_dir.join(&model_file);
    downloader::download_model(app, &url, &dest).await
}

#[tauri::command]
async fn toggle_recording(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    do_toggle_recording(&app, &state).await
}

#[tauri::command]
fn start_stream(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let (model_path, step, length) = {
        let s = state.settings.lock().unwrap();
        let model_file = transcribe_local::model_filename(&s.whisper_model);
        (
            state.app_dir.join(model_file).to_string_lossy().to_string(),
            s.stream_step,
            s.stream_length,
        )
    };
    state.stream.start(&app, &model_path, step, length)?;
    update_overlay_streaming(&app, true);
    if let Some(item) = state.toggle_item.lock().unwrap().as_ref() {
        let _ = item.set_text("Stop Streaming");
    }
    Ok(())
}

#[tauri::command]
fn stop_stream(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.stream.stop()?;
    update_overlay_streaming(&app, false);
    if let Some(item) = state.toggle_item.lock().unwrap().as_ref() {
        let _ = item.set_text("Start Streaming");
    }
    Ok(())
}

#[tauri::command]
fn is_streaming(state: State<AppState>) -> bool {
    state.stream.is_streaming()
}

/// Shared logic for toggle recording (hotkey only -- tray now controls streaming).
async fn do_toggle_recording(
    app: &tauri::AppHandle,
    state: &AppState,
) -> Result<String, String> {
    let current_state = state.recorder.get_state();
    println!("[Typr] do_toggle_recording called, current state: {:?}", current_state);
    match current_state {
        RecordingState::Ready => {
            let (mic, output_mode) = {
                let s = state.settings.lock().unwrap();
                (s.microphone.clone(), s.output_mode.clone())
            };
            state.recorder.start_recording(app, &mic, &output_mode)?;
            Ok("recording".to_string())
        }
        RecordingState::Recording => {
            let settings = state.settings.lock().unwrap().clone();
            let result = state
                .recorder
                .stop_and_transcribe(app, &settings, &state.app_dir)
                .await?;
            Ok(result)
        }
        RecordingState::Transcribing => {
            Err("Currently transcribing, please wait".to_string())
        }
    }
}

/// Toggle streaming state -- used by tray click and tray menu.
fn do_toggle_streaming(app: &tauri::AppHandle, state: &AppState) -> Result<String, String> {
    if state.stream.is_streaming() {
        state.stream.stop()?;
        update_overlay_streaming(app, false);
        if let Some(item) = state.toggle_item.lock().unwrap().as_ref() {
            let _ = item.set_text("Start Streaming");
        }
        // Notify frontend
        let _ = app.emit_to("main", "stream-stopped", ());
        Ok("stopped".to_string())
    } else {
        let (model_path, step, length) = {
            let s = state.settings.lock().unwrap();
            let model_file = transcribe_local::model_filename(&s.whisper_model);
            (
                state.app_dir.join(model_file).to_string_lossy().to_string(),
                s.stream_step,
                s.stream_length,
            )
        };
        state.stream.start(app, &model_path, step, length)?;
        update_overlay_streaming(app, true);
        if let Some(item) = state.toggle_item.lock().unwrap().as_ref() {
            let _ = item.set_text("Stop Streaming");
        }
        Ok("streaming".to_string())
    }
}

fn update_overlay_streaming(app: &tauri::AppHandle, streaming: bool) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        let class = if streaming { "mic streaming" } else { "mic" };
        let js = format!("document.getElementById('mic').className = '{}';", class);
        let _ = overlay.eval(&js);
    }
}

fn output_mode_label(mode: &str) -> &'static str {
    match mode {
        "document" => "Output: Document",
        "terminal" => "Output: Terminal",
        _ => "Output: Clipboard",
    }
}

fn main() {
    let app_dir = get_app_dir();
    let settings = Settings::load(&app_dir);
    let initial_hotkey = settings.hotkey.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            recorder: Recorder::new(),
            settings: Mutex::new(settings),
            app_dir,
            stream: StreamState::new(),
            toggle_item: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            list_microphones,
            get_recording_state,
            check_model_downloaded,
            download_model,
            toggle_recording,
            start_stream,
            stop_stream,
            is_streaming,
        ])
        .setup(move |app| {
            // Create the overlay window (small mic icon, top-right, always on top)
            let monitor = app.primary_monitor().ok().flatten();
            let (x, y) = if let Some(m) = monitor {
                let size = m.size();
                let scale = m.scale_factor();
                let logical_w = size.width as f64 / scale;
                ((logical_w - 60.0) as i32, 10_i32)
            } else {
                (1380, 10)
            };

            let overlay = WebviewWindowBuilder::new(
                app,
                "overlay",
                WebviewUrl::App("src/overlay.html".into()),
            )
            .title("")
            .inner_size(50.0, 50.0)
            .position(x as f64, y as f64)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .shadow(false)
            .build();

            match overlay {
                Ok(_) => println!("[Typr] Overlay window created"),
                Err(e) => eprintln!("[Typr] Failed to create overlay: {}", e),
            }

            // --- System tray menu ---
            let toggle_item = MenuItemBuilder::with_id("toggle", "Start Streaming")
                .build(app)?;
            let settings_item = MenuItemBuilder::with_id("settings", "Settings")
                .build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit Typr")
                .build(app)?;

            let initial_output_mode = app.state::<AppState>().settings.lock().unwrap().output_mode.clone();
            let initial_output_label = output_mode_label(&initial_output_mode);
            let output_item = MenuItemBuilder::with_id("output-mode", initial_output_label)
                .build(app)?;

            let tray_menu = MenuBuilder::new(app)
                .item(&toggle_item)
                .separator()
                .item(&output_item)
                .separator()
                .item(&settings_item)
                .separator()
                .item(&quit_item)
                .build()?;

            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-icon.png"))
                .expect("Failed to load tray icon");

            // Store the toggle item in AppState so commands can update its text
            *app.state::<AppState>().toggle_item.lock().unwrap() = Some(toggle_item.clone());

            let output_item_handle = output_item.clone();
            let _tray = TrayIconBuilder::with_id("main")
                .icon(icon)
                .icon_as_template(true)
                .menu(&tray_menu)
                .tooltip("Typr - Voice Dictation")
                .on_tray_icon_event(|tray, event| {
                    // Left-click toggles streaming
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        let app = tray.app_handle().clone();
                        let state = app.state::<AppState>();
                        match do_toggle_streaming(&app, state.inner()) {
                            Ok(result) => println!("[Typr] Tray click: {}", result),
                            Err(e) => eprintln!("[Typr] Tray click error: {}", e),
                        }
                    }
                })
                .on_menu_event(move |app_handle, event| {
                    match event.id().as_ref() {
                        "toggle" => {
                            let state = app_handle.state::<AppState>();
                            match do_toggle_streaming(app_handle, state.inner()) {
                                Ok(result) => println!("[Typr] Menu toggle streaming: {}", result),
                                Err(e) => eprintln!("[Typr] Menu toggle error: {}", e),
                            }
                        }
                        "output-mode" => {
                            let state = app_handle.state::<AppState>();
                            let new_mode = {
                                let mut s = state.settings.lock().unwrap();
                                let next = match s.output_mode.as_str() {
                                    "clipboard" => "document",
                                    "document" => "terminal",
                                    _ => "clipboard",
                                };
                                s.output_mode = next.to_string();
                                next.to_string()
                            };
                            let s = state.settings.lock().unwrap().clone();
                            let _ = s.save(&state.app_dir);
                            let _ = output_item_handle.set_text(output_mode_label(&new_mode));
                            println!("[Typr] Output mode changed to: {}", new_mode);
                        }
                        "settings" => {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app_handle.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            println!("[Typr] System tray created");

            let handle = app.handle().clone();

            println!("[Typr] Registering global shortcut: {}", initial_hotkey);

            match app.global_shortcut().on_shortcut(
                initial_hotkey.as_str(),
                move |_app, shortcut, event| {
                    println!("[Typr] Hotkey event: {:?} state={:?}", shortcut, event.state);
                    let handle = handle.clone();
                    let state = handle.state::<AppState>();
                    let mode = state.settings.lock().unwrap().recording_mode.clone();
                    println!("[Typr] Recording mode: {}", mode);

                    match event.state {
                        ShortcutState::Pressed => {
                            tauri::async_runtime::spawn(async move {
                                let state = handle.state::<AppState>();
                                match mode.as_str() {
                                    "toggle" => {
                                        println!("[Typr] Toggle mode: calling do_toggle_recording");
                                        match do_toggle_recording(&handle, state.inner()).await {
                                            Ok(result) => println!("[Typr] Toggle result: {}", result),
                                            Err(e) => eprintln!("[Typr] Toggle error: {}", e),
                                        }
                                    }
                                    "push-to-talk" => {
                                        let current = state.recorder.get_state();
                                        println!("[Typr] PTT mode, current state: {:?}", current);
                                        if current == RecordingState::Ready {
                                            let (mic, output_mode) = {
                                                let s = state.settings.lock().unwrap();
                                                (s.microphone.clone(), s.output_mode.clone())
                                            };
                                            match state.recorder.start_recording(&handle, &mic, &output_mode) {
                                                Ok(_) => println!("[Typr] Recording started"),
                                                Err(e) => eprintln!("[Typr] Start recording error: {}", e),
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            });
                        }
                        ShortcutState::Released => {
                            if mode == "push-to-talk" {
                                tauri::async_runtime::spawn(async move {
                                    let state = handle.state::<AppState>();
                                    let current = state.recorder.get_state();
                                    if current == RecordingState::Recording {
                                        let settings =
                                            state.settings.lock().unwrap().clone();
                                        match state.recorder.stop_and_transcribe(
                                            &handle,
                                            &settings,
                                            &state.app_dir,
                                        ).await {
                                            Ok(result) => println!("[Typr] Transcription: {}", result),
                                            Err(e) => eprintln!("[Typr] Transcription error: {}", e),
                                        }
                                    }
                                });
                            }
                        }
                    }
                },
            ) {
                Ok(_) => println!("[Typr] Global shortcut registered successfully"),
                Err(e) => eprintln!("[Typr] ERROR: Failed to register global shortcut: {}", e),
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
