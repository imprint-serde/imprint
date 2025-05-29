#!/bin/bash
set -euo pipefail

# Configuration
BENCH_NAME="serde_bench"
WORKTREE_DIR="/tmp/bench-main"
REPORT_FILE="benchmark-report.md"

# Cleanup trap
cleanup() {
  echo "🧹 Cleaning up..."
  git worktree remove --force "$WORKTREE_DIR" 2>/dev/null || true
}
trap cleanup EXIT

# Check if critcmp is installed
if ! command -v critcmp >/dev/null 2>&1; then
  echo "❌ critcmp is not installed. Please run: cargo install critcmp"
  exit 1
fi

# Fetch and prepare main branch in a clean worktree
echo "📦 Checking out 'main' into temporary worktree..."
git fetch origin main
git worktree add --force "$WORKTREE_DIR" origin/main

# Run benchmarks on main branch
echo "🚀 Running benchmarks on 'main'..."
(
  cd "$WORKTREE_DIR"
  cargo bench --bench "$BENCH_NAME" -- --save-baseline main
)
mkdir -p ./target/criterion
cp -r "$WORKTREE_DIR/target/criterion" ./target/criterion

# Run benchmarks on current branch
echo "🚀 Running benchmarks on PR branch..."
cargo bench --bench "$BENCH_NAME" -- --save-baseline pr

# Compare with critcmp
echo "📊 Comparing benchmarks..."
critcmp main pr | ./scripts/format_critcmp.sh > "$REPORT_FILE"

echo "✅ Benchmark comparison saved to $REPORT_FILE"
