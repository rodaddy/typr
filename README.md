# Typr

Real-time voice transcription app with local whisper.cpp engine, live streaming transcript, and AI earpiece coaching pipeline.

Built on [Tauri 2](https://tauri.app/) (Rust + TypeScript) with [whisper.cpp](https://github.com/ggerganov/whisper.cpp) for on-device speech recognition. No cloud APIs required.

## Features

- **Live Streaming Transcript** -- real-time text as you speak, right in the app window
- **Local Whisper Engine** -- whisper.cpp runs on Apple Silicon Metal GPU, fully offline
- **Three Output Modes**
  - 🟢 **Document** -- append timestamped transcriptions to daily files
  - 🟠 **Clipboard** -- paste transcribed text into any active app
  - 🔴 **Terminal** -- stream to `/tmp/typr-stream` for piping to other tools
- **System Tray** -- menu bar icon with quick controls, left-click to toggle
- **Global Hotkey** -- Cmd+Shift+Space to start/stop recording
- **Earpiece Pipeline** -- AI coaching mode: live audio → transcription → LLM → TTS response

## Architecture

```
Microphone → whisper-stream (local, Metal GPU) → /tmp/typr-stream
                                                       ↓
                                              earpiece.sh (optional)
                                                       ↓
                                              LLM API → TTS → speaker/earpiece
```

## Requirements

- macOS (Apple Silicon recommended)
- [Rust](https://rustup.rs/) + [Node.js](https://nodejs.org/)
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) (`brew install whisper-cpp`)

## Quick Start

```bash
# Install whisper.cpp
brew install whisper-cpp

# Clone and build
git clone https://github.com/Skippy-the-Magnificent-one/typr.git
cd typr
npm install
npx tauri build

# Install the app
cp -R src-tauri/target/release/bundle/macos/Typr.app /Applications/

# Download a whisper model (first run)
# Open Typr → Engine → Download
```

## CLI Tools

### typr-live
Real-time transcription in your terminal:
```bash
typr-live              # Start transcribing (Ctrl+C to stop)
typr-live --file       # Also save to /tmp/typr-stream
```

### typr-ctl
Remote control for Typr:
```bash
typr-ctl start         # Launch Typr
typr-ctl stop          # Quit Typr
typr-ctl listen 10     # Record 10 seconds, return transcription
typr-ctl status        # Check status
typr-ctl mode terminal # Switch output mode
```

### earpiece
AI coaching pipeline:
```bash
earpiece                    # Start earpiece (listens + responds via TTS)
earpiece --quiet            # Text-only, no speech
earpiece --context "..."    # Custom system prompt
earpiece --buffer 3         # Process every 3 transcript lines
```

## Configuration

Config file: `~/Library/Application Support/com.typr.app/config.json`

| Setting | Default | Description |
|---------|---------|-------------|
| engine | local | Transcription engine (local/cloud) |
| whisperModel | small | Whisper model size (base.en/small/medium) |
| recordingMode | toggle | Toggle or push-to-talk |
| outputMode | clipboard | Output destination (clipboard/document/terminal) |
| streamStep | 2000 | Stream processing interval (ms) |
| streamLength | 8000 | Audio window length (ms) |
| hotkey | CmdOrCtrl+Shift+Space | Global hotkey |

## Models

| Model | Size | Speed | Accuracy |
|-------|------|-------|----------|
| base.en | 142 MB | Fastest | Good |
| small | 466 MB | Fast | Better |
| medium | 1.5 GB | Moderate | Best |

Models are downloaded to `~/Library/Application Support/com.typr.app/` on first use.

## Credits

Originally forked from [albertshiney/typr](https://github.com/albertshiney/typr). Extended with live streaming, output modes, system tray, earpiece pipeline, and CLI tools.

## License

MIT
