#!/bin/bash
# Full cross-brain × cross-env evolution sweep at 3k and 10k eval ticks.
# 6 brains × 5 envs × 2 horizons = 60 runs. 6gen/8pop/2seed.
cd ~/repos/ants-sim
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
BIN=./target/release/ants-sim
BRAINS="fsm ann cppn snn mb cx"
ENVS="rich_close scarce_far predator patchy maze"
OUT=results/full_evolve_3k_10k.csv
echo "brain,env,horizon,score,collected" > "$OUT"
run_one() {
  local b=$1 e=$2 h=$3
  local log="results/evolve_${b}_${e}_${h}.log"
  $BIN --headless --evolve --brain "$b" --env "$e" --gens 6 --pop 8 --ticks "$h" --n-seeds 2 --out-stem "evolved_${b}_${e}_${h}" > "$log" 2>&1
  local best=$(grep -E "^BEST score=" "$log" | head -1)
  local score=$(echo "$best" | sed -E 's/.*score=([0-9.]+).*/\1/')
  local col=$(echo "$best" | sed -E 's/.*collected=([0-9.]+).*/\1/')
  echo "$b,$e,$h,$score,$col" >> "$OUT"
  echo "  [$b/$e/${h}] score=$score collected=$col"
}
export -f run_one
export BIN OUT
echo "=== Full evolve sweep start $(date +%H:%M:%S) ==="
# Run 3k first (faster), then 10k. 4 parallel.
for h in 3000 10000; do
  echo "--- horizon=$h $(date +%H:%M:%S) ---"
  for b in $BRAINS; do
    for e in $ENVS; do
      echo "$b $e $h"
    done
  done | xargs -P 4 -L 1 bash -c 'run_one "$@"' _
done
echo "=== Full evolve sweep DONE $(date +%H:%M:%S) ==="
echo "---- summary ----"
column -t -s',' "$OUT"
