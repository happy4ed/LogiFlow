#!/usr/bin/env bash
# Mac 실행 시험(기기 없는 러너) — 상주 앱이 뜨고 죽지 않는지 · Swift 판 설정 이어받기 · IPC 소켓 · 로그인 항목 ·
# 두 번째 실행이 설정 창을 여는지. 프로세스는 우리가 띄운 PID 로만 끈다.
# 사용: mac-smoke.sh <LogiFlowRs.app> <캡처 폴더>
set -euo pipefail
APP="$1"
SNAP="$2"
EXE="$APP/Contents/MacOS/LogiFlowRs"
SUP="$HOME/Library/Application Support/LogiFlowRs"
PLIST="$HOME/Library/LaunchAgents/com.happy4ed.logiflow.rs.plist"
mkdir -p "$SNAP"

"$EXE" --version

# 1) Swift 판(0.15.0) 설정 흉내 — 가장자리 자동 전환 끔 + 변 배치 2개(화면 id 12345 · 채널 2 오른쪽 · 채널 3 위).
defaults write com.happy4ed.logiflow edgeEnabled -bool false
JSON='[{"channel":1,"displayID":12345,"edge":"right"},{"channel":2,"displayID":12345,"edge":"top"}]'
HEX=$(printf '%s' "$JSON" | xxd -p | tr -d '\n')
defaults write com.happy4ed.logiflow edgeBindings -data "$HEX"
defaults read com.happy4ed.logiflow
rm -rf "$SUP" "$PLIST"

# 2) 로그인 항목 경로(--autostart)로 상주 앱.
"$EXE" --autostart > "$SNAP/agent-stdout.txt" 2>&1 &
AGENT=$!
for _ in $(seq 1 40); do [ -S "$SUP/agent.sock" ] && break; sleep 0.5; done
if [ ! -S "$SUP/agent.sock" ]; then echo "상주 앱 IPC 소켓이 안 생김"; cat "$SNAP/agent-stdout.txt"; kill "$AGENT" || true; exit 1; fi
sleep 3
kill -0 "$AGENT"
echo "--- config.txt"; cat "$SUP/config.txt"
grep -q '^edgeEnabled=0' "$SUP/config.txt" || { echo "edgeEnabled 이어받기 실패"; exit 1; }
python3 - "$SUP/config.txt" <<'PY'
import sys
lines = [l.rstrip('\r\n').lstrip('﻿') for l in open(sys.argv[1], encoding='utf-8-sig')]
att = sorted(l for l in lines if l.startswith('attach='))
want = ['attach=1\t12345\t1', 'attach=2\t12345\t2']
print('attach:', att)
sys.exit(0 if att == want else 1)
PY
echo "--- LaunchAgent"; cat "$PLIST"; plutil -lint "$PLIST"; grep -q -- '--autostart' "$PLIST"
grep -q "$EXE" "$PLIST"

# 3) 두 번째 실행(사람이 앱을 다시 엶) → 이미 떠 있는 상주 앱에 「창 열기」만 보내고 끝나야 한다 → 설정 창 프로세스.
"$EXE" &
SECOND=$!
for _ in $(seq 1 20); do kill -0 "$SECOND" 2>/dev/null || break; sleep 0.5; done
if kill -0 "$SECOND" 2>/dev/null; then echo "두 번째 실행이 끝나지 않음(상주 앱이 두 개)"; kill "$SECOND"; kill "$AGENT"; exit 1; fi
UI=""
for _ in $(seq 1 30); do UI=$(pgrep -P "$AGENT" || true); [ -n "$UI" ] && break; sleep 0.5; done
echo "설정 창 프로세스: ${UI:-없음}"
test -n "$UI"
for _ in $(seq 1 20); do ls "$SUP"/ui-*.sock >/dev/null 2>&1 && break; sleep 0.5; done
ls -la "$SUP"
sleep 4
screencapture -x "$SNAP/live-settings.png" || true
kill -0 "$AGENT"
kill -0 "$UI"

echo "--- 기록"; cat "$SUP/logs/logiflow-diag.log" || true
grep -q "Swift 판 설정 이어받음" "$SUP/logs/logiflow-diag.log"

kill "$UI" || true
kill "$AGENT" || true
sleep 1
defaults delete com.happy4ed.logiflow || true
echo "mac smoke ok"
