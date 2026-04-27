# Typr Swarm Fix -- Get This Shit Right

## Current State
App builds and runs but terminal mode doesn't output anything to /tmp/typr-stream. Multiple patches have been applied piecemeal and things are broken.

## What Needs to Work (TEST ALL OF THESE)

### 1. Whisper Model
- Check if a whisper model is actually downloaded in ~/Library/Application Support/com.typr.app/ or wherever the app_dir is
- If not, download one programmatically (small model)
- The model download UI might be broken too

### 2. Terminal Mode Output
- When output mode is "terminal", transcribed text MUST be written to /tmp/typr-stream (append mode)
- The file should be created if it doesn't exist
- Each transcription should be a new line
- Test this actually works by checking the file after a transcription

### 3. All Three Output Modes Must Work
- **Clipboard**: transcribe → clipboard + Cmd+V paste into active app
- **Document**: transcribe → append to ~/Documents/Typr/typr-YYYY-MM-DD.txt
- **Terminal**: transcribe → append to /tmp/typr-stream

### 4. Mic Overlay Colors
- Ready: dark grey
- Recording + Document mode: GREEN (#4CAF50)
- Recording + Clipboard mode: ORANGE (#FF9800)  
- Recording + Terminal mode: RED (#F44336)
- Transcribing: BLUE (#2196F3)
- Must reset back to grey after transcription completes or fails

### 5. Clickable Mic Overlay
- Clicking the mic bubble must toggle recording on/off
- Uses window.__TAURI__.core.invoke('toggle_recording')

### 6. System Tray
- Tray icon visible in menu bar
- Menu items: toggle recording, output mode cycle, settings, quit
- Icon uses include_bytes! with image-png feature

### 7. Hotkey
- Cmd+Shift+Space toggles recording (both press and release must work)
- Needs Input Monitoring permission on macOS

### 8. State Machine
- Recording state must ALWAYS reset to Ready after transcription (success or failure)
- Never get stuck in Transcribing state

## Debugging Steps
1. First, check if the app_dir exists and what's in it: ls ~/Library/Application\ Support/com.typr.app/
2. Check if any whisper model .bin file exists there
3. If no model, the transcription silently fails -- that's probably why terminal mode produces nothing
4. Add better error logging throughout the pipeline
5. Check recorder.rs do_transcribe() error handling -- errors should be logged clearly

## Build
```bash
cd /Volumes/ThunderBolt/Development/typr
npx tauri build 2>&1 | tail -20
```
- DMG bundling fails -- IGNORE IT, the .app bundle works
- Binary sidecar must exist: src-tauri/binaries/whisper-cpp-aarch64-apple-darwin
- After build: pkill -f typr; rm -rf /Applications/Typr.app; cp -R src-tauri/target/release/bundle/macos/Typr.app /Applications/; open /Applications/Typr.app

## Test After Build
1. Launch app
2. Check tray icon appears
3. Open settings, verify model is downloaded
4. Set output to Terminal
5. touch /tmp/typr-stream && tail -f /tmp/typr-stream in another terminal
6. Click mic, say "hello world", click mic again
7. Verify text appears in /tmp/typr-stream
8. Switch to Clipboard mode, verify paste works
9. Switch to Document mode, verify file created in ~/Documents/Typr/
