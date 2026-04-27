#!/bin/bash
# typr-live -- Real-time voice transcription in your terminal
# 
# Usage:
#   typr-live              Start transcribing (Ctrl+C to stop)
#   typr-live --file       Also save to /tmp/typr-stream
#   typr-live --help       Show this help
#
# Text appears in real-time as you speak. Press Ctrl+C to stop.

MODEL="$HOME/Library/Application Support/com.typr.app/ggml-small.bin"
STREAM="/tmp/typr-stream"
STEP=3000        # Process audio every 3 seconds
LENGTH=10000     # Audio window length (10s)

# Check for model
if [ ! -f "$MODEL" ]; then
    echo "❌ Whisper model not found at: $MODEL"
    echo "Download it:"
    echo "  curl -L -o \"$MODEL\" https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"
    exit 1
fi

# Check for whisper-stream
if ! command -v whisper-stream &>/dev/null; then
    echo "❌ whisper-stream not found. Install: brew install whisper-cpp"
    exit 1
fi

save_to_file=false
if [ "$1" = "--file" ]; then
    save_to_file=true
    shift
fi

if [ "$1" = "--help" ] || [ "$1" = "-h" ]; then
    echo "typr-live -- Real-time voice transcription"
    echo ""
    echo "Usage:"
    echo "  typr-live          Start transcribing (Ctrl+C to stop)"
    echo "  typr-live --file   Also save to /tmp/typr-stream"
    echo ""
    echo "Speak naturally. Text appears as you talk."
    echo "Press Ctrl+C to stop."
    exit 0
fi

echo "🎤 Typr Live -- speak and see text in real-time"
echo "   Model: small | Step: ${STEP}ms | Ctrl+C to stop"
echo "──────────────────────────────────────────────────"

cleanup() {
    echo ""
    echo "──────────────────────────────────────────────────"
    echo "🛑 Stopped."
    if [ "$save_to_file" = true ] && [ -f "$STREAM" ]; then
        lines=$(wc -l < "$STREAM" | tr -d ' ')
        echo "📄 Saved $lines lines to $STREAM"
    fi
    exit 0
}
trap cleanup INT TERM

# whisper-stream uses \r and ANSI codes to update in-place on a TTY
# Just let it run directly -- it looks fine in a real terminal
if [ "$save_to_file" = true ]; then
    whisper-stream \
        -m "$MODEL" \
        -l en \
        --step "$STEP" \
        --length "$LENGTH" \
        -f "$STREAM" \
        2>/dev/null
else
    whisper-stream \
        -m "$MODEL" \
        -l en \
        --step "$STEP" \
        --length "$LENGTH" \
        2>/dev/null
fi
