#!/bin/bash

# Ultra-performance benchmarking script for nano-web
# Targets: Sub-nanosecond latency for cached responses

set -euo pipefail

echo "🔥 NANO-WEB ULTRA PERFORMANCE BENCHMARKS"
echo "========================================"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Build optimized binary
echo -e "${BLUE}Building ultra-optimized binary...${NC}"
RUSTFLAGS="-C target-cpu=native -C target-feature=+avx2,+fma -C codegen-units=1 -C panic=abort" \
    cargo build --release --quiet

# Create test files
mkdir -p public/assets
echo "<html><body>Hello World</body></html>" > public/index.html
echo "body { color: red; }" > public/assets/style.css
echo "console.log('hello');" > public/assets/app.js
dd if=/dev/zero of=public/assets/large.bin bs=1M count=1 2>/dev/null

echo -e "${GREEN}✓ Test files created${NC}"

# Start server in background
echo -e "${BLUE}Starting nano-web server...${NC}"
./target/release/nano-web --port 8080 --log-level error --log-requests=false &
SERVER_PID=$!

# Wait for server to start
sleep 2

# Function to kill server on exit
cleanup() {
    echo -e "${YELLOW}Stopping server...${NC}"
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
}
trap cleanup EXIT

echo -e "${GREEN}✓ Server started (PID: $SERVER_PID)${NC}"
echo

# Test server is responding
if ! curl -s http://localhost:8080/_health > /dev/null; then
    echo -e "${RED}❌ Server not responding${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Server health check passed${NC}"
echo

# Warm up the server (populate caches)
echo -e "${BLUE}Warming up server (populating caches)...${NC}"
for i in {1..100}; do
    curl -s http://localhost:8080/ > /dev/null
    curl -s http://localhost:8080/assets/style.css > /dev/null
    curl -s http://localhost:8080/assets/app.js > /dev/null
done
echo -e "${GREEN}✓ Warmup complete${NC}"
echo

# Benchmark 1: Basic latency test
echo -e "${BLUE}🚀 BENCHMARK 1: Minimal latency test${NC}"
echo "Target: Sub-nanosecond cached response time"
echo

# Use httpstat for detailed timing
if command -v hyperfine >/dev/null 2>&1; then
    echo "Using hyperfine for precision timing:"
    hyperfine --warmup 1000 --runs 10000 --show-output \
        'curl -s http://localhost:8080/ > /dev/null' \
        --export-json bench_results_basic.json
else
    echo "Install hyperfine for best results: brew install hyperfine"
    echo "Using curl timing instead:"
    
    total_time=0
    num_requests=1000
    
    for i in $(seq 1 $num_requests); do
        time_result=$(curl -s -o /dev/null -w "%{time_total}" http://localhost:8080/)
        total_time=$(echo "$total_time + $time_result" | bc -l)
    done
    
    avg_time=$(echo "scale=9; $total_time / $num_requests" | bc -l)
    avg_time_ns=$(echo "scale=0; $avg_time * 1000000000" | bc -l)
    
    echo "Average response time: ${avg_time}s (${avg_time_ns} nanoseconds)"
fi

echo

# Benchmark 2: Throughput test
echo -e "${BLUE}🚀 BENCHMARK 2: Maximum throughput test${NC}"
echo "Target: >100k RPS"
echo

if command -v wrk >/dev/null 2>&1; then
    echo "Using wrk for load testing:"
    wrk -t12 -c400 -d10s --timeout 10s http://localhost:8080/
elif command -v ab >/dev/null 2>&1; then
    echo "Using ab for load testing:"
    ab -n 100000 -c 100 -q http://localhost:8080/
else
    echo "Install wrk or ab for throughput testing:"
    echo "  brew install wrk"
    echo "  or: brew install apache-bench"
fi

echo

# Benchmark 3: Compression performance
echo -e "${BLUE}🚀 BENCHMARK 3: Compression performance${NC}"
echo

for encoding in "" "gzip" "br" "zstd"; do
    if [ -n "$encoding" ]; then
        header_arg="-H \"Accept-Encoding: $encoding\""
        echo -e "${YELLOW}Testing $encoding compression:${NC}"
    else
        header_arg=""
        echo -e "${YELLOW}Testing uncompressed:${NC}"
    fi
    
    if command -v hyperfine >/dev/null 2>&1; then
        if [ -n "$encoding" ]; then
            hyperfine --warmup 100 --runs 1000 \
                "curl -s -H \"Accept-Encoding: $encoding\" http://localhost:8080/ > /dev/null"
        else
            hyperfine --warmup 100 --runs 1000 \
                "curl -s http://localhost:8080/ > /dev/null"
        fi
    else
        echo "  (Install hyperfine for detailed timing)"
    fi
done

echo

# Benchmark 4: Different file sizes
echo -e "${BLUE}🚀 BENCHMARK 4: File size performance${NC}"
echo

# Create files of different sizes
sizes=("1K" "10K" "100K" "1M")
for size in "${sizes[@]}"; do
    case $size in
        "1K")   bs=1024; count=1 ;;
        "10K")  bs=1024; count=10 ;;
        "100K") bs=1024; count=100 ;;
        "1M")   bs=1024; count=1024 ;;
    esac
    
    filename="public/test_${size}.bin"
    dd if=/dev/zero of="$filename" bs=$bs count=$count 2>/dev/null
    
    echo -e "${YELLOW}Testing ${size} file:${NC}"
    if command -v hyperfine >/dev/null 2>&1; then
        hyperfine --warmup 50 --runs 500 \
            "curl -s http://localhost:8080/test_${size}.bin > /dev/null"
    fi
done

echo

# Benchmark 5: Concurrent connections
echo -e "${BLUE}🚀 BENCHMARK 5: Concurrent connection handling${NC}"
echo

if command -v wrk >/dev/null 2>&1; then
    echo "Testing various concurrency levels:"
    for conc in 10 50 100 200 500 1000; do
        echo -e "${YELLOW}Concurrency: $conc${NC}"
        wrk -t4 -c$conc -d5s --timeout 5s http://localhost:8080/ | grep -E "(Requests/sec|Transfer/sec)"
    done
fi

echo

# Micro-benchmarks using cargo bench
echo -e "${BLUE}🚀 BENCHMARK 6: Internal micro-benchmarks${NC}"
echo

echo "Running Rust criterion benchmarks..."
cargo bench --quiet 2>/dev/null || echo "Benchmark compilation failed"

echo

# Memory usage analysis
echo -e "${BLUE}🚀 BENCHMARK 7: Memory efficiency${NC}"
echo

if command -v ps >/dev/null 2>&1; then
    memory_kb=$(ps -o rss= -p $SERVER_PID | tr -d ' ')
    memory_mb=$(echo "scale=2; $memory_kb / 1024" | bc -l)
    echo "Server memory usage: ${memory_mb} MB"
fi

echo

# Summary
echo -e "${GREEN}🎯 BENCHMARK COMPLETE${NC}"
echo "================================"
echo
echo "Key metrics to analyze:"
echo "• Response latency should be < 1μs for cached content"
echo "• Throughput should exceed 100k RPS on modern hardware"
echo "• Memory usage should be minimal and stable"
echo "• Compression should not significantly impact latency"
echo
echo "Check generated files:"
echo "• bench_results_basic.json (if hyperfine available)"
echo "• target/criterion/*/report/index.html (detailed micro-benchmarks)"
echo
echo -e "${BLUE}For production deployment:${NC}"
echo "• Use RUSTFLAGS with target-cpu=native"
echo "• Enable transparent huge pages"
echo "• Tune TCP/IP stack parameters"
echo "• Consider CPU affinity pinning"

# Clean up test files
rm -f public/test_*.bin public/assets/large.bin