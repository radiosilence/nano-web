#!/usr/bin/env bash
# Reproducible benchmark for nano-web. Captures oha JSON per scenario so builds
# can be compared apples-to-apples with bench/compare.py.
#
#   bench/run.sh <label>            # build, serve, bench, write bench/results/<label>/
#   BENCH_NO_BUILD=1 bench/run.sh x # skip cargo build (use existing target/release)
#   BENCH_CONN=50 BENCH_DUR=10s bench/run.sh x   # override load params
#
# Load params are fixed across runs (env-overridable) so two labels are comparable
# only when run with identical CONN/DUR on the same machine, idle.

set -euo pipefail

LABEL="${1:?usage: bench/run.sh <label>}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${BENCH_PORT:-38080}"
CONN="${BENCH_CONN:-50}"
DUR="${BENCH_DUR:-10s}"
BIN="${BENCH_BIN:-$ROOT/target/release/nano-web}"
FIXTURES="$ROOT/bench/fixtures"
OUT="$ROOT/bench/results/$LABEL"

oha() { mise x -- oha "$@"; }

build() {
  [[ "${BENCH_NO_BUILD:-0}" == "1" ]] && { echo "→ skip build"; return; }
  echo "→ cargo build --release"
  (cd "$ROOT" && mise x -- cargo build --release --quiet)
}

# Deterministic fixture set: small HTML, compressible JS/CSS, incompressible 1MB blob.
make_fixtures() {
  rm -rf "$FIXTURES"
  mkdir -p "$FIXTURES/assets"
  {
    echo '<!doctype html><html><head><meta charset="utf-8"><title>bench</title>'
    echo '<link rel="stylesheet" href="/assets/style.css"></head><body>'
    printf '<p>%s</p>\n' {1..40}
    echo '<script src="/assets/app.js"></script></body></html>'
  } > "$FIXTURES/index.html"
  # ~80KB of compressible JS (plain loop — avoids `yes|head` SIGPIPE under pipefail)
  : > "$FIXTURES/assets/app.js"
  for ((i = 0; i < 1200; i++)); do
    echo 'function f(){return Math.random()*42;} // padding padding padding'
  done > "$FIXTURES/assets/app.js"
  # ~12KB compressible CSS
  : > "$FIXTURES/assets/style.css"
  for ((i = 0; i < 200; i++)); do
    echo '.a{color:#abcdef;margin:0;padding:0;display:flex;align-items:center}'
  done > "$FIXTURES/assets/style.css"
  # 1MB incompressible blob (random → real transfer cost, no compression cheat)
  dd if=/dev/urandom of="$FIXTURES/assets/blob.bin" bs=1024 count=1024 2>/dev/null
}

SERVER_PID=""
cleanup() { [[ -n "$SERVER_PID" ]] && kill "$SERVER_PID" 2>/dev/null || true; }
trap cleanup EXIT

start_server() {
  echo "→ serve $BIN on :$PORT (engine=${BENCH_ENGINE:-hyper})"
  "$BIN" serve "$FIXTURES" --port "$PORT" --log-level error \
    --engine "${BENCH_ENGINE:-hyper}" >/dev/null 2>&1 &
  SERVER_PID=$!
  for _ in $(seq 1 50); do
    curl -fsS "http://127.0.0.1:$PORT/_health" >/dev/null 2>&1 && return 0
    sleep 0.1
  done
  echo "✗ server failed to come up" >&2; exit 1
}

# scenario <name> <oha-args...>
scenario() {
  local name="$1"; shift
  echo "  • $name"
  # Write to a temp then move, so a concurrent compare.py never sees a half-written
  # (or freshly-truncated, empty) result file.
  oha -c "$CONN" -z "$DUR" --no-tui --output-format json --latency-correction \
    "$@" > "$OUT/$name.json.tmp"
  mv "$OUT/$name.json.tmp" "$OUT/$name.json"
}

main() {
  build
  make_fixtures
  mkdir -p "$OUT"
  start_server

  # warm caches / JIT TCP path
  for _ in $(seq 1 200); do curl -fsS "http://127.0.0.1:$PORT/" >/dev/null; done

  local base="http://127.0.0.1:$PORT"
  local etag
  etag="$(curl -fsS -D - -o /dev/null "$base/" | tr -d '\r' | awk -F': ' 'tolower($1)=="etag"{print $2}')"

  scenario index_identity "$base/"
  scenario index_brotli   -H "Accept-Encoding: br"   "$base/"
  scenario js_brotli      -H "Accept-Encoding: br"   "$base/assets/app.js"
  scenario css_gzip       -H "Accept-Encoding: gzip" "$base/assets/style.css"
  scenario blob_1mb       "$base/assets/blob.bin"
  scenario head_index     -m HEAD "$base/"
  scenario notfound       "$base/does-not-exist"
  [[ -n "$etag" ]] && scenario cond_304 -H "If-None-Match: $etag" "$base/"

  echo "→ wrote $OUT"
  echo "→ compare with: bench/compare.py <other-label> $LABEL"
}

main
