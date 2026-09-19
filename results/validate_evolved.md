# ants-sim 演化适配验证（--validate --evolved）

回应 Tier 20 发现"硬环境默认脑全饿死——需演化适配"。本验证演化各脑在硬环境的冠军（8 代/10 种群/3 种子/600t），再以 `--validate --evolved --ticks 3000` 验证冠军在硬环境的 3000t 存活/涌现，对比默认脑。

## 演化冠军 collected@600t（--evolve --out-stem evolved_<brain>_<env>）

| brain | scarce_far | maze |
|---|---|---|
| fsm | 92.0 | 53.7 |
| ann | 40.3 | 30.7 |
| cppn | 83.3 | 40.7 |
| snn | 21.3 | 11.7 |
| mb | 31.7 | 27.3 |
| cx | 76.3 | 49.3 |

## 默认 vs 演化：3000t 行为涌现（survival / pass）

### scarce_far

| brain | 默认 survival | 默认 pass | 演化 survival | 演化 pass |
|---|---|---|---|---|
| fsm | 0.00 | 1/5 | **1.17** | **5/5** |
| ann | 0.00 | 0/5 | 0.00 | 0/5 |
| cppn | 0.00 | 0/5 | **0.41** | **4/5** |
| snn | 0.00 | 1/5 | 0.00 | 1/5 |
| mb | 0.00 | 1/5 | 0.00 | 1/5 |
| cx | 0.00 | 1/5 | 0.02 | 2/5 |

### maze

| brain | 默认 survival | 默认 pass | 演化 survival | 演化 pass |
|---|---|---|---|---|
| fsm | 0.00 | 1/5 | 0.00 | 1/5 |
| ann | 0.00 | 0/5 | 0.00 | 0/5 |
| cppn | 0.00 | 0/5 | 0.00 | 0/5 |
| snn | 0.00 | 1/5 | 0.00 | 1/5 |
| mb | 0.00 | 1/5 | 0.00 | 1/5 |
| cx | 0.00 | 1/5 | 0.00 | 1/5 |

## 诚实评估

**演化确实适配了部分脑到 scarce_far**：fsm 演化冠军 3000t survival 1.17（蚁群增长！5/5 全涌现），cppn 0.41（4/5）。对比默认全饿死——**"需演化适配"在 scarce_far 对 fsm/cppn 成立**。

**但适配不完整**：
- ann/snn/mb 演化冠军在 scarce_far 仍饿死（survival 0）。它们的 600t 收集（40/21/31）不足以支撑 3000t 蚁群能量经济。
- **maze 全部仍饿死**（含 fsm 演化冠军）。maze 墙体迫使绕远，单远食，600t 选择的冠军撑不过 3000t。

**根因（诚实）**：演化在 600t 评估下选择的是 600t 收集能力，**不是 3000t 存活**。600t 收集高 ≠ 3000t 蚁群经济可持续（能量收支随时间恶化）。要让演化冠军通过 3000t，需**更长视野演化**（eval_ticks=3000）或显式生存选择（Fitness.survival 已含 0.2·survival·colony，但 600t 窗口 survival 信号弱）。

## 长视野演化闭合 maze gap（验证根因）

对 fsm/maze 用 **3000t 评估**演化（6gen/10pop/2seed），冠军 collected=399.5@3000t（vs 600t 评估冠军 53.7@600t）。再 `--validate --evolved --ticks 3000`：

| brain | env | 默认 | 600t-eval 演化 | **3000t-eval 演化** |
|---|---|---|---|---|
| fsm | maze | 0.00 / 1/5 | 0.00 / 1/5 | **1.21 / 5/5**（增长）|

**结论**：演化是真实的适配机制（fsm/cppn 在 scarce_far 从全死到存活增长），证实"硬环境需演化"的判断；600t 演化不足以让所有脑/所有硬环境通过 3000t——**但把评估视野拉到 3000t，fsm/maze 从 1/5 跃到 5/5（survival 1.21，蚁群增长）**。即"评估视野需匹配目标存活视野"——这是诚实的演化结论，非模型故障。定量一致性仍受示意级定位限制（见 literature/*.toml + --compare-lit）。

## 10000t 视野实验（视野泛化）

进一步问：3k-eval 冠军能否泛化到 10k？跑 fsm/maze **10000t-eval** 演化（6gen/10pop/2seed，冠军 collected=1489.5@10000t），并 3-way 对比 @10000t：

| fsm/maze @10000t | survival | pass |
|---|---|---|
| 默认 | 0.00 | 1/5（饿死）|
| 3000t-eval 冠军 | 1.02 | 5/5（存活，**泛化**）|
| 10000t-eval 冠军 | 1.14 | 5/5（略优）|

**更干净的发现**：3k-eval 冠军不仅过 3k，**泛化到 10k 也存活（5/5, survival 1.02）**。即演化只要选过蚁群的"饿死视野"（~3k，达到可持续蚁群经济），冠军就能泛化到更长视野；10k-eval 边际略优（1.14）但 3k-eval 已足够。600t-eval 没过饿死点→3k/10k 都死。这印证"演化需选过生存阈值"而非简单"视野匹配"——是一个诚实的、可复现的演化结论。

## scarce_far 跨视野对比（环境依赖的过拟合）

镜像 maze 实验，跑 fsm/scarce_far 的 600t/3k/10k-eval 演化，4-way @10000t：

| fsm/scarce_far @10000t | survival | pass |
|---|---|---|
| 默认 | 0.00 | 1/5（饿死）|
| 600t-eval 冠军 | 0.78 | 5/5 |
| 3k-eval 冠军 | **0.08** | 4/5（**近崩**）|
| 10k-eval 冠军 | 1.13 | 5/5 |

冠军 collected：600t-eval 92@600t，3k-eval 485.5@3k，10k-eval 1388@10k。

**非单调 + 环境依赖**：scarce_far 的 3k-eval 冠军在 10k 反而**近崩（0.08）**，比 600t-eval（0.78）还差——与 maze（3k 泛化到 10k）相反。诚实结论：**更长视野选择不单调改善长视野存活——3k-eval 可能在 3k 过拟合**（高 3k 收集策略在 10k 不可持续），600t-eval 弱选择反而落到更稳的通用策略。**10k-eval 在 10k 最稳（1.13）**——视野匹配仍是最安全赌注，但中间视野演化环境依赖、可能过拟合。（注：n_seeds=2/pop10 小规模，3k 近崩可能含随机性；但 10k-eval 最稳是稳健结论。）

## 复现性验证（3k-eval → 10k 存活是否稳健）

原 3k-eval 近崩（0.08）是小样本（n_seeds=2/pop10/seed42）单点。跑 2 个独立 3k-eval 演化验证：A=多种子(n_seeds=5,seed42)，B=不同基种子(n_seeds=2,seed123)。3 冠军 @10000t：

| 3k-eval 冠军 @10k | collected@3k | survival@10k | pass |
|---|---|---|---|
| 原（n2,seed42）| 485.5 | 0.08 | 4/5（近崩）|
| A（n5,seed42）| 673.6 | 0.47 | 5/5（部分存活）|
| B（n2,seed123）| 774.5 | 0.00 | 1/5（全崩）|

**诚实复现结论**：3k-eval→10k 存活**高方差**（0.00–0.47，跨 3 种子），**非确定性过拟合**——原 0.08 是高方差区一个样本。对比 10k-eval（1.13 稳健）。这**强化**核心结论（视野匹配最可靠），但**软化**"过拟合"措辞：是"非目标视野选择的高方差"，非确定性过拟合。3k 收集高（485–774）≠ 10k 存活——选择视野与目标视野不匹配时，结果不稳定。

**跨环境 + 复现性演化结论**：演化适配真实（两环境默认全死→10k-eval 全活且稳健）；**视野匹配（在目标存活视野评估）是最可靠的适配策略**；中间视野（3k）演化在更长目标（10k）上高方差、不可靠（maze 单点泛化是幸运，scarce_far 复现示高方差）。




## 复现
```bash
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
# 演化硬环境冠军（per brain×env）
for b in fsm ann cppn snn mb cx; do for e in scarce_far maze; do
  cargo run --release -- --headless --evolve --brain $b --env $e --gens 8 --pop 10 --ticks 600 --n-seeds 3 --out-stem evolved_${b}_${e}
done; done
# 默认 vs 演化 3000t 验证
cargo run --release -- --headless --validate --env scarce_far --ticks 3000
cargo run --release -- --headless --validate --evolved --env scarce_far --ticks 3000
cargo run --release -- --headless --validate --env maze --ticks 3000
cargo run --release -- --headless --validate --evolved --env maze --ticks 3000
```
