#!/bin/bash
# typr-ctl.sh -- Skippy's control interface for Typr
# Usage: typr-ctl.sh <command> [args]
#
# Commands:
#   start       - Launch Typr if not running
#   stop        - Quit Typr
#   record      - Start recording (toggle on)
#   finish      - Stop recording and transcribe (toggle off)
#   listen <N>  - Record for N seconds, then stop and return transcription
#   status      - Check if Typr is running and current state
#   mode <mode> - Switch output mode (clipboard|document|terminal)
#   read        - Read latest transcription from /tmp/typr-stream
#   tail        - Read last N lines from stream
#   clear       - Clear the stream file

set -euo pipefail

TYPR_APP="/Applications/Typr.app"
TYPR_BIN="$TYPR_APP/Contents/MacOS/typr"
TYPR_CONFIG="$HOME/Library/Application Support/com.typr.app/config.json"
TYPR_STREAM="/tmp/typr-stream"
TYPR_LOG="/tmp/typr-console.log"

cmd="${1:-help}"
shift || true

toggle_hotkey() {
    osascript -e 'tell application "System Events" to key code 49 using {command down, shift down}' 2>/dev/null
}

case "$cmd" in
    start)
        if pgrep -f "$TYPR_BIN" >/dev/null 2>&1; then
            echo "Typr already running (pid $(pgrep -f "$TYPR_BIN"))"
        else
            # Always launch with logging so we can debug
            touch "$TYPR_STREAM"
            "$TYPR_BIN" > "$TYPR_LOG" 2>&1 &
            sleep 2
            if pgrep -f "$TYPR_BIN" >/dev/null 2>&1; then
                echo "Typr started (pid $(pgrep -f "$TYPR_BIN"))"
                echo "Stream: tail -f $TYPR_STREAM"
                echo "Logs: tail -f $TYPR_LOG"
            else
                echo "ERROR: Typr failed to start. Check $TYPR_LOG"
                exit 1
            fi
        fi
        ;;

    stop)
        if pgrep -f "$TYPR_BIN" >/dev/null 2>&1; then
            # Kill only the actual Typr binary, not tail/grep/etc
            kill $(pgrep -xf "$TYPR_BIN") 2>/dev/null
            echo "Typr stopped"
        else
            echo "Typr not running"
        fi
        ;;

    record)
        if ! pgrep -f "$TYPR_BIN" >/dev/null 2>&1; then
            echo "ERROR: Typr not running. Use: typr-ctl.sh start"
            exit 1
        fi
        toggle_hotkey
        sleep 0.5
        echo "Recording started"
        ;;

    finish)
        if ! pgrep -f "$TYPR_BIN" >/dev/null 2>&1; then
            echo "ERROR: Typr not running"
            exit 1
        fi
        toggle_hotkey
        echo "Recording stopped, transcribing..."
        # Wait for transcription (whisper takes a few seconds)
        for i in $(seq 1 30); do
            sleep 1
            # Check if new content appeared in stream
            if [ -f "$TYPR_STREAM" ]; then
                new_line=$(tail -1 "$TYPR_STREAM" 2>/dev/null)
                if [ -n "$new_line" ]; then
                    echo "Transcription: $new_line"
                    break
                fi
            fi
            # Also check log for completion
            if grep -q "Toggle result:" "$TYPR_LOG" 2>/dev/null; then
                last_result=$(grep "Toggle result:" "$TYPR_LOG" | tail -1 | sed 's/.*Toggle result: //')
                if [ "$last_result" != "recording" ] && [ -n "$last_result" ]; then
                    echo "Transcription: $last_result"
                    break
                fi
            fi
        done
        ;;

    listen)
        duration="${1:-5}"
        if ! pgrep -f "$TYPR_BIN" >/dev/null 2>&1; then
            echo "ERROR: Typr not running. Use: typr-ctl.sh start"
            exit 1
        fi
        # Capture line count before recording
        before=0
        if [ -f "$TYPR_STREAM" ]; then
            before=$(wc -l < "$TYPR_STREAM")
        fi
        # Start recording
        toggle_hotkey
        sleep 0.5
        echo "Recording for ${duration}s..."
        sleep "$duration"
        # Stop recording
        toggle_hotkey
        echo "Transcribing..."
        # Wait up to 30s for whisper to finish
        for i in $(seq 1 30); do
            sleep 1
            if [ -f "$TYPR_STREAM" ]; then
                after=$(wc -l < "$TYPR_STREAM")
                if [ "$after" -gt "$before" ]; then
                    # New lines appeared -- grab them
                    tail -n +$((before + 1)) "$TYPR_STREAM"
                    exit 0
                fi
            fi
        done
        echo "ERROR: Transcription timed out"
        exit 1
        ;;

    status)
        if pgrep -f "$TYPR_BIN" >/dev/null 2>&1; then
            pid=$(pgrep -f "$TYPR_BIN")
            mode=$(python3 -c "import json; c=json.load(open('$TYPR_CONFIG')); print(f'mode={c.get(\"recordingMode\",\"?\")}, output={c.get(\"outputMode\",\"?\")}, engine={c.get(\"engine\",\"?\")}')" 2>/dev/null || echo "config unreadable")
            echo "Typr running (pid $pid) -- $mode"
            if [ -f "$TYPR_STREAM" ]; then
                lines=$(wc -l < "$TYPR_STREAM")
                echo "Stream: $TYPR_STREAM ($lines lines)"
            else
                echo "Stream: no file yet"
            fi
        else
            echo "Typr not running"
        fi
        ;;

    mode)
        new_mode="${1:-clipboard}"
        if [ "$new_mode" != "clipboard" ] && [ "$new_mode" != "document" ] && [ "$new_mode" != "terminal" ]; then
            echo "ERROR: Invalid mode. Use: clipboard, document, terminal"
            exit 1
        fi
        python3 -c "
import json
with open('$TYPR_CONFIG') as f:
    c = json.load(f)
c['outputMode'] = '$new_mode'
with open('$TYPR_CONFIG', 'w') as f:
    json.dump(c, f, indent=2)
print(f'Output mode set to: $new_mode')
print('Restart Typr for changes to take effect: typr-ctl.sh stop && typr-ctl.sh start')
"
        ;;

    read)
        if [ -f "$TYPR_STREAM" ]; then
            tail -1 "$TYPR_STREAM"
        else
            echo "No transcriptions yet"
        fi
        ;;

    tail)
        n="${1:-5}"
        if [ -f "$TYPR_STREAM" ]; then
            tail -n "$n" "$TYPR_STREAM"
        else
            echo "No transcriptions yet"
        fi
        ;;

    clear)
        > "$TYPR_STREAM" 2>/dev/null
        echo "Stream cleared"
        ;;

    help|*)
        echo "typr-ctl.sh -- Skippy's Typr control interface"
        echo ""
        echo "Commands:"
        echo "  start         Launch Typr"
        echo "  stop          Quit Typr"
        echo "  record        Start recording"
        echo "  finish        Stop recording and show transcription"
        echo "  listen <N>    Record for N seconds, return transcription"
        echo "  status        Check Typr status"
        echo "  mode <mode>   Set output mode (clipboard|document|terminal)"
        echo "  read          Read latest transcription"
        echo "  tail [N]      Read last N transcriptions"
        echo "  clear         Clear stream file"
        ;;
esac
