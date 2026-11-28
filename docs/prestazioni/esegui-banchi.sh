#!/usr/bin/env bash
# Benchmarks of interactive work (docs/prestazioni/README.md).
# Run from the repository root, on a machine left alone:
#   bash docs/prestazioni/esegui-banchi.sh [filter] [screen]
# filter: a bench name part (default: all benches, "bench_");
# screen: physical pixels and scale, e.g. 3840x2160x1.5 (default 1600x1000x1).
# Writes docs/prestazioni/misure/<date>-<commit>-<host>.csv (one row per case).
set -euo pipefail
filter="${1:-bench_}"
screen="${2:-1600x1000x1}"
commit="$(git rev-parse --short HEAD)"
dirty=""
git diff --quiet || dirty="-modificato"
host="$(hostname | tr -c 'A-Za-z0-9\n' '_' | head -c 20)"
out="docs/prestazioni/misure/$(date +%Y%m%d-%H%M)-${commit}${dirty}-${host}.csv"
echo "Banchi: ${filter}, schermo ${screen}, risultati in ${out}"
echo "Macchina: $(nproc) thread; $(uname -sm)"
BENCH_OUT="$PWD/${out}" BENCH_SCREEN="${screen}" CARGO_PROFILE_FAST_DEBUG=0 \
  cargo test --profile fast -p creabim-app --bin creabim-app "${filter}" \
  -- --ignored --nocapture --test-threads=1 2>&1 | grep -E "^BENCH|panicked|test result"
