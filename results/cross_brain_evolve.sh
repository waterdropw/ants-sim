#!/bin/bash
# 全 brain × 5 环境 跨环境演化大跑 (6 brains × 5 envs = 30 runs)
cd ~/repos/ants-sim
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
BIN=./target/release/ants-sim
BRAINS="fsm ann cppn snn mb cx"
ENVS="rich_close scarce_far predator patchy maze"
OUT=results/cross_brain_evolution_summary.txt

echo "brain|env|best_score|collected|mean_def_frac" > "$OUT"
echo "=== 跨环境演化大跑开始 $(date +%H:%M:%S) ==="

for b in $BRAINS; do
  for e in $ENVS; do
    log="results/evolve_${b}_${e}.log"
    echo "[$(date +%H:%M:%S)] evolve $b / $e ..."
    $BIN --headless --evolve --brain "$b" --env "$e" --gens 8 --pop 10 --ticks 600 --n-seeds 3 > "$log" 2>&1
    best=$(grep -E '^BEST score=' "$log" | head -1)
    score=$(echo "$best" | sed -E 's/.*score=([0-9.]+).*/\1/')
    collected=$(echo "$best" | sed -E 's/.*collected=([0-9.]+).*/\1/')
    def=$(echo "$best" | sed -E 's/.*mean_def_frac=([0-9.]+).*/\1/')
    echo "$b|$e|$score|$collected|$def" >> "$OUT"
    echo "    -> $best"
  done
done

echo "=== 大跑完成 $(date +%H:%M:%S) ==="
echo "---- 汇总 ----"
column -t -s'|' "$OUT"
