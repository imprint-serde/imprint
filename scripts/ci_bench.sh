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

cat <<EOF > "$REPORT_FILE"
## 📊 Benchmark Comparison Report

This pull request includes Criterion benchmarks comparing performance to the \`main\` branch.
This comment will automatically update as the benchmarks are re-run on each commit.

The table below shows **relative ratios** and **timing stats** for each benchmark group:

\`\`\`
$(critcmp main pr)
\`\`\`

✅ Benchmarks completed successfully.

🧠 **Notes**:
- These benchmarks are not a pass/fail gate and are informative only.
- Use this as a signal to review performance-sensitive changes.
- Results may be unreliable due to GHA runner hardware variance.
- If results indicate a significant performance regression, run the benchmarks locally to confirm.

_Reported by the benchmark CI bot_
EOF

echo "✅ Benchmark comparison saved to $REPORT_FILE"
