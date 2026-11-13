#!/usr/bin/env bash
# Smoke test on a running emulator: install the APK, launch it, and fail if
# it crashed (Java exception, Rust panic, ANR) or is no longer running.
# Usage: scripts/android-smoke.sh <apk> <output-dir>
set -euo pipefail

apk="$1"
out="$2"
pkg="io.github.r0madeus.foomodern"
mkdir -p "$out"

adb wait-for-device
adb install -r -g "$apk"
adb logcat -c
# The launcher intent, so the activity name does not matter.
adb shell monkey -p "$pkg" -c android.intent.category.LAUNCHER 1 >/dev/null
sleep 25

adb exec-out screencap -p > "$out/screenshot.png" || true
adb logcat -d > "$out/logcat.txt" || true

status=0
if ! adb shell pidof "$pkg" >/dev/null; then
  echo "::error::$pkg is not running 25 s after launch"
  status=1
fi
if grep -nE "FATAL EXCEPTION|panicked at|ANR in $pkg|Fatal signal" "$out/logcat.txt"; then
  echo "::error::crash found in logcat (see the android-smoke artifact)"
  status=1
fi
grep -nE "RustStdoutStderr|Tauri|$pkg" "$out/logcat.txt" | tail -n 40 || true
exit "$status"
