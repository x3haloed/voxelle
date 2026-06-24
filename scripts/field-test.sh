#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WEB_PORT="${VOXELLE_WEB_PORT:-5173}"
SIGNAL_PORT="${VOXELLE_SIGNAL_PORT:-9002}"
SSH_TARGET="${VOXELLE_LOCALHOST_RUN_TARGET:-nokey@localhost.run}"
LOG_DIR="${VOXELLE_FIELD_TEST_LOG_DIR:-$ROOT_DIR/logs/field-test}"
DRY_RUN=0

usage() {
  cat <<EOF
Usage: npm run field:test [-- --dry-run]

Starts a Voxelle friend-test rig:
  1. Vite web app on 127.0.0.1:${WEB_PORT}
  2. voxelle-signal relay on 127.0.0.1:${SIGNAL_PORT}
  3. localhost.run HTTPS tunnels for both services

Environment:
  VOXELLE_WEB_PORT             Web app port (default: 5173)
  VOXELLE_SIGNAL_PORT          Signaling relay port (default: 9002)
  VOXELLE_LOCALHOST_RUN_TARGET SSH target (default: nokey@localhost.run; use localhost.run for account/key auth)
  VOXELLE_FIELD_TEST_LOG_DIR   Log directory (default: logs/field-test)

Press Ctrl-C to stop all started processes.
EOF
}

for arg in "$@"; do
  case "$arg" in
    --dry-run)
      DRY_RUN=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      usage >&2
      exit 2
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_cmd npm
require_cmd cargo
require_cmd ssh

cat <<EOF
Voxelle field-test launcher

Commands:
  web:    npm run dev -w @voxelle/web -- --host 127.0.0.1 --port ${WEB_PORT}
  signal: cargo run -p voxelle-signal -- --host 127.0.0.1 --port ${SIGNAL_PORT}
  web tunnel:    ssh -o ServerAliveInterval=60 -o ExitOnForwardFailure=yes -R 80:localhost:${WEB_PORT} ${SSH_TARGET}
  signal tunnel: ssh -o ServerAliveInterval=60 -o ExitOnForwardFailure=yes -R 80:localhost:${SIGNAL_PORT} ${SSH_TARGET}

Logs: ${LOG_DIR}
EOF

if [[ "$DRY_RUN" == "1" ]]; then
  exit 0
fi

mkdir -p "$LOG_DIR"

PIDS=()
CLEANED_UP=0
cleanup() {
  local status=$?
  if [[ "$CLEANED_UP" == "1" ]]; then
    exit "$status"
  fi
  CLEANED_UP=1
  if [[ ${#PIDS[@]} -gt 0 ]]; then
    echo
    echo "Stopping field-test processes..."
    for pid in "${PIDS[@]}"; do
      if kill -0 "$pid" >/dev/null 2>&1; then
        kill "$pid" >/dev/null 2>&1 || true
      fi
    done
  fi
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

start_bg() {
  local label="$1"
  local log="$2"
  shift 2
  echo "Starting ${label}..."
  (
    cd "$ROOT_DIR"
    exec "$@"
  ) >"$log" 2>&1 &
  local pid=$!
  PIDS+=("$pid")
  echo "  pid ${pid}, log ${log}"
  STARTED_PID="$pid"
}

wait_for_http() {
  local label="$1"
  local url="$2"
  local timeout="${3:-45}"
  local pid="${4:-}"

  if ! command -v curl >/dev/null 2>&1; then
    echo "curl not found; skipping ${label} readiness check."
    return 0
  fi

  for _ in $(seq 1 "$timeout"); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      echo "${label} is ready: ${url}"
      return 0
    fi
    if [[ -n "$pid" ]] && ! kill -0 "$pid" >/dev/null 2>&1; then
      echo "${label} exited before becoming ready. Check its log." >&2
      return 1
    fi
    sleep 1
  done

  echo "Timed out waiting for ${label}: ${url}" >&2
  return 1
}

extract_https_url() {
  local log="$1"
  sed -n -E 's/^.*tunneled with tls termination, (https:\/\/[^[:space:]]+).*$/\1/p' "$log" 2>/dev/null \
    | sed -E 's/[),.;]+$//' \
    | head -n 1
}

wait_for_tunnel_url() {
  local label="$1"
  local log="$2"
  local timeout="${3:-60}"
  local pid="${4:-}"
  local url=""

  for _ in $(seq 1 "$timeout"); do
    if grep -Eqi '(^|[[:space:]])(Permission denied|permission denied) \((publickey|keyboard-interactive|password)' "$log" 2>/dev/null; then
      echo "localhost.run denied SSH auth for ${label}." >&2
      echo "Try: VOXELLE_LOCALHOST_RUN_TARGET=nokey@localhost.run npm run field:test" >&2
      return 1
    fi
    url="$(extract_https_url "$log" || true)"
    if [[ -n "$url" ]]; then
      echo "$url"
      return 0
    fi
    if [[ -n "$pid" ]] && ! kill -0 "$pid" >/dev/null 2>&1; then
      echo "${label} tunnel exited before printing a URL. See ${log}" >&2
      return 1
    fi
    sleep 1
  done

  echo "Timed out waiting for ${label} localhost.run URL. See ${log}" >&2
  return 1
}

WEB_LOG="$LOG_DIR/web.log"
SIGNAL_LOG="$LOG_DIR/signal.log"
WEB_TUNNEL_LOG="$LOG_DIR/web-tunnel.log"
SIGNAL_TUNNEL_LOG="$LOG_DIR/signal-tunnel.log"

STARTED_PID=""
start_bg "web app" "$WEB_LOG" npm run dev -w @voxelle/web -- --host 127.0.0.1 --port "$WEB_PORT"
wait_for_http "web app" "http://127.0.0.1:${WEB_PORT}/" 45 "$STARTED_PID"

STARTED_PID=""
start_bg "signaling relay" "$SIGNAL_LOG" cargo run -p voxelle-signal -- --host 127.0.0.1 --port "$SIGNAL_PORT"
wait_for_http "signaling relay" "http://127.0.0.1:${SIGNAL_PORT}/info" 45 "$STARTED_PID"

STARTED_PID=""
start_bg "web localhost.run tunnel" "$WEB_TUNNEL_LOG" ssh -o ServerAliveInterval=60 -o ExitOnForwardFailure=yes -R "80:localhost:${WEB_PORT}" "$SSH_TARGET"
WEB_URL="$(wait_for_tunnel_url "web app" "$WEB_TUNNEL_LOG" 60 "$STARTED_PID")"

STARTED_PID=""
start_bg "signal localhost.run tunnel" "$SIGNAL_TUNNEL_LOG" ssh -o ServerAliveInterval=60 -o ExitOnForwardFailure=yes -R "80:localhost:${SIGNAL_PORT}" "$SSH_TARGET"
SIGNAL_URL="$(wait_for_tunnel_url "signaling relay" "$SIGNAL_TUNNEL_LOG" 60 "$STARTED_PID")"
RELAY_WS_URL="${SIGNAL_URL/#https:/wss:}/ws"

cat <<EOF

Ready.

Share with testers:
  App URL:   ${WEB_URL}

Use inside Voxelle:
  Relay URL: ${RELAY_WS_URL}

Checklist:
  1. Open the App URL yourself.
  2. Create a Space.
  3. In Invite, paste the Relay URL and click "Create Invite (copy link)".
  4. Open the generated host link in your browser so your side starts hosting the relay session.
  5. Send the invite link to one tester.
  6. Ask the tester to open it, join #general, and wait for connection status "connected".
  7. Both sides send a short unique message, then refresh and confirm messages remain.

Known limitation for this field test:
  The current web client manages one WebRTC peer connection per room tab. This launcher proves public serving
  and relay rendezvous, but true five-person group chat still needs a multi-peer transport pass.

Keep this terminal open. Press Ctrl-C to stop the app, relay, and tunnels.
Logs: ${LOG_DIR}
EOF

while true; do
  sleep 3600
done
