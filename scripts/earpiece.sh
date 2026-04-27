#!/bin/bash
# earpiece.sh -- Skippy's earpiece v4.0
# 
# Tails /tmp/typr-stream, accumulates speech chunks,
# sends to Skippy (Anthropic API) for processing,
# speaks the response via macOS TTS.
#
# Usage:
#   earpiece.sh                  Start earpiece (requires Typr streaming)
#   earpiece.sh --voice Alex     Use specific TTS voice
#   earpiece.sh --quiet          Don't speak, just print responses
#   earpiece.sh --context "..."  Set the context/system prompt
#
# Ctrl+C to stop.

set -euo pipefail

STREAM="/tmp/typr-stream"
ANTHROPIC_KEY="${ANTHROPIC_API_KEY:-}"
ANTHROPIC_URL="${ANTHROPIC_BASE_URL:-https://api.anthropic.com}"
MODEL="claude-sonnet-4-6"
VOICE="Samantha"
QUIET=false
BUFFER_LINES=3  # Accumulate this many lines before sending to Skippy
CONTEXT="You are Skippy, an AI assistant listening to a live conversation via earpiece. You hear what Rico hears. Provide brief, useful coaching -- key points, suggestions, corrections, or things Rico should say. Keep responses under 2 sentences. Be direct and concise. If the audio is just background noise or irrelevant, respond with SKIP."

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        --voice) VOICE="$2"; shift 2 ;;
        --quiet) QUIET=true; shift ;;
        --context) CONTEXT="$2"; shift 2 ;;
        --buffer) BUFFER_LINES="$2"; shift 2 ;;
        --model) MODEL="$2"; shift 2 ;;
        --help|-h)
            echo "earpiece.sh -- Skippy's earpiece v4.0"
            echo ""
            echo "Options:"
            echo "  --voice NAME    TTS voice (default: Samantha)"
            echo "  --quiet         Print only, don't speak"
            echo "  --context TEXT  System prompt for Skippy"
            echo "  --buffer N      Lines to accumulate before processing (default: 3)"
            echo "  --model MODEL   Anthropic model (default: claude-sonnet-4-6)"
            echo ""
            echo "Requires: Typr streaming to /tmp/typr-stream"
            echo "          ANTHROPIC_API_KEY env var"
            exit 0
            ;;
        *) shift ;;
    esac
done

# Check prerequisites
if [ -z "$ANTHROPIC_KEY" ]; then
    # Try to get from zshrc
    ANTHROPIC_KEY=$(grep "ANTHROPIC_API_KEY" ~/.zshrc 2>/dev/null | head -1 | sed 's/.*="\(.*\)"/\1/' || true)
    if [ -z "$ANTHROPIC_KEY" ]; then
        echo "❌ ANTHROPIC_API_KEY not set"
        exit 1
    fi
fi

if [ ! -f "$STREAM" ]; then
    echo "❌ Stream file not found: $STREAM"
    echo "   Start Typr streaming first"
    exit 1
fi

echo "🎧 Earpiece v4.0 -- Skippy is listening"
echo "   Model: $MODEL | Voice: $VOICE | Buffer: $BUFFER_LINES lines"
echo "   Stream: $STREAM"
echo "──────────────────────────────────────────────────"

cleanup() {
    echo ""
    echo "──────────────────────────────────────────────────"
    echo "🛑 Earpiece stopped."
    exit 0
}
trap cleanup INT TERM

# Track where we are in the stream
last_line_count=$(wc -l < "$STREAM" | tr -d ' ')
buffer=""
buffer_count=0

send_to_skippy() {
    local text="$1"
    
    # Skip empty or noise-only text
    if [ -z "$text" ] || echo "$text" | grep -qiE '^\[?(BLANK_AUDIO|silence|noise)\]?$'; then
        return
    fi

    local response
    # Use python3 to build JSON safely (handles special chars in transcript)
    response=$(python3 -c "
import json, subprocess, sys

text = sys.argv[1]
model = sys.argv[2]
context = sys.argv[3]
url = sys.argv[4]
key = sys.argv[5]

payload = json.dumps({
    'model': model,
    'max_tokens': 150,
    'system': context,
    'messages': [
        {'role': 'user', 'content': f'You just heard this from the live audio feed:\n\n{text}\n\nRespond with brief coaching, or SKIP if nothing useful to say.'}
    ]
})

result = subprocess.run(
    ['curl', '-s', '--max-time', '15',
     '-H', f'x-api-key: {key}',
     '-H', 'anthropic-version: 2023-06-01',
     '-H', 'content-type: application/json',
     f'{url}/v1/messages',
     '-d', payload],
    capture_output=True, text=True
)

try:
    d = json.loads(result.stdout)
    reply = d.get('content', [{}])[0].get('text', '').strip()
    print(reply)
except:
    print('')
" "$text" "$MODEL" "$CONTEXT" "$ANTHROPIC_URL" "$ANTHROPIC_KEY" 2>/dev/null)

    # Skip if Skippy says SKIP or empty
    if [ -z "$response" ] || echo "$response" | grep -qi "^SKIP$"; then
        return
    fi

    echo "💬 $response"
    
    if [ "$QUIET" = false ]; then
        # Speak the response (non-blocking -- don't hold up the loop)
        say -v "$VOICE" "$response" &
    fi
}

# Main loop -- tail the stream and process new lines
while true; do
    current_count=$(wc -l < "$STREAM" 2>/dev/null | tr -d ' ')
    
    if [ "$current_count" -gt "$last_line_count" ]; then
        # New lines appeared
        new_lines=$(tail -n +$((last_line_count + 1)) "$STREAM" | head -n $((current_count - last_line_count)))
        last_line_count=$current_count
        
        # Add to buffer
        while IFS= read -r line; do
            # Skip blank audio markers and empty lines
            clean=$(echo "$line" | sed 's/\[BLANK_AUDIO\]//g' | tr -s ' ' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
            if [ -n "$clean" ]; then
                if [ -n "$buffer" ]; then
                    buffer="$buffer $clean"
                else
                    buffer="$clean"
                fi
                buffer_count=$((buffer_count + 1))
            fi
        done <<< "$new_lines"
        
        # When buffer has enough lines, send to Skippy
        if [ "$buffer_count" -ge "$BUFFER_LINES" ]; then
            echo "🎤 Heard: $(echo "$buffer" | head -c 100)..."
            send_to_skippy "$buffer"
            buffer=""
            buffer_count=0
        fi
    fi
    
    sleep 1
done
