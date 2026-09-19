# ants-sim

蚂蚁个体智能 / 群体智能 / 行为模拟系统，含**基因 × 环境演化闭环**。Rust + egui 0.36。

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Language](https://img.shields.io/badge/language-Rust-orange.svg)
![egui](https://img.shields.io/badge/egui-0.36-8b5cf6.svg)
![Tests](https://img.shields.io/badge/tests-30%20pass-green.svg)

把蚂蚁的「身体即计算 + 信息素涌现 + 群体分布式智能 + 基因-环境适配」做成**可运行、可观测、可复现**的仿真：6 种可演化决策脑（FSM/ANN/CPPN/SNN/蘑菇体/中央复合体）、信息素场双缓冲扩散、最短路径 ACO 涌现、跨环境演化与特化、以及与经典昆虫行为学文献的定量对照。

---

## 目录

- [特性](#特性)
- [快速开始](#快速开始)
- [架构](#架构)
- [决策层（`--brain`）](#决策层--brain)
- [昆虫脑神经模块](#昆虫脑神经模块)
- [演化与选择](#演化与选择)
- [验证体系](#验证体系)
- [关键结果](#关键结果)
- [命令行参考](#命令行参考)
- [性能](#性能)
- [可复现性](#可复现性)
- [目录结构](#目录结构)
- [文档](#文档)
- [许可证](#许可证)

---

## 特性

- **6 种决策脑**：手写子行为 FSM（默认）、直接编码 ANN、间接编码 CPPN、LIF 脉冲 SNN、蘑菇体 MB、中央复合体 CX，均可经 `--brain` 切换并参与演化。
- **基因 × 环境闭环**：30+ 字段基因组，高斯变异 + BLX-α 交叉，5 种环境预设，冠军基因随环境特化。
- **神经模块化昆虫脑**：蘑菇体（AL+MB+多巴胺门控 STDP+神经发生）、中央复合体环吸引子路径积分、多模态整合、章鱼胺/多巴胺神经调质、CPG 三角步态。
- **涌现行为**：ACO 双桥最短路径、工/兵/育幼分工、领地战争、育幼 stigmergy、协同进化军备竞赛。
- **科学验证**：行为涌现电池（`--validate`）、机制消融 → 可证伪预测 + 湿实验 protocol（`--ablate`）、文献定量对照（`--compare-lit`）、CX 路径积分误差基准（`--bench-cx`）。
- **可复现 + 可扩展**：rayon 并行下逐字节确定性；100k 蚂蚁 @ 136 tick/s。

## 快速开始

需要 Rust 工具链（如无：`brew install rustup-init && rustup default stable`）。

```bash
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
cargo run --release            # 打开 GUI
cargo test                     # 30 个性质测试
cargo run --release -- --headless --report   # 重生成 REPORT.md
```

## 架构

分层：`viz → sim → world + ant`，加 `evolution`（选择）、`environment`（环境）、`config`（配置）、`genome`（可演化基因组）。

| 模块 | 职责 |
|---|---|
| `src/genome.rs` | 基因组（30+ 字段）+ `mutate`(高斯)/`crossover`(BLX-α)/`random`/`in_range` + `foraging_ann_seed` + `develop_cppn` + `distance`（基因型距离） |
| `src/config.rs` | TOML 配置（`--config`/`--print-config`） |
| `src/sim.rs` | tick 循环：化学步 → rayon 并行蚂蚁步 → 串行 flush（沉积/攻击/食物递减/战争/brood） |
| `src/world/` | 网格、信息素场（双缓冲 5 点扩散 + 按通道衰减）、空间哈希、巢/食物/敌人/墙/brood |
| `src/ant/` | Ant（health/energy/colony_id/dead）+ 6 决策脑 + 感官（8 方向梯度 + 视觉锥） |
| `src/environment.rs` | 5 环境预设（rich/scarce/predator/patchy/maze）+ EnvMetrics |
| `src/evolution.rs` | `evaluate`/`multi` + GA `run`（niche + 多样性）+ `run_multilevel`（个体级选择）+ novelty search |
| `src/viz/` | egui App：brain 选择器 + 4 通道缩略图 + sparkline + 场景编辑器 |

## 决策层（`--brain`）

| brain | 类型 | 说明 |
|---|---|---|
| `fsm`（默认） | 手写 | 子 sumption FSM + 神经调质标量；分级 trail 软偏置 + 梯度上升 chemotaxis |
| `ann` | 直接编码 | 固定 MLP（9→10→5），权重可演化，门控输入 + 觅食种子 |
| `cppn` | 间接编码 | 16 个 `cppn_genes` 经余弦基发育成 155 权重（基因型 ≪ 表型） |
| `snn` | 脉冲 | LIF（9→10→5）+ STDP 可学习权重；育幼 early-return + 视觉奔食输入 |
| `mb` | 神经模块 | 蘑菇体：AL(12 瞌小球多模态)→64 稀疏 KC→5 输出；双通道多巴胺门控 STDP + 神经发生 |
| `cx` | 神经模块 | 中央复合体：CX_N=16 环吸引子路径积分，神经化归巢向量（替代软件真值） |

## 昆虫脑神经模块

真实蚂蚁神经系统的关键结构，做成**可演化、可独立验证**的模块：

| Tier | 模块 | 生物对应 | 关键机制 |
|---|---|---|---|
| 8 | 蘑菇体 MB | AL + MB + STDP | 12 瞌小球收敛 → 64 稀疏 KC → 5 输出；多巴胺门控 STDP |
| 9 | 中央复合体 CX | 环吸引子 + CPU4 | CX_N=16 朝向单元神经化路径积分 |
| 10 | 多模态整合 | 视觉 + 距离 → MB | 视觉/距离 4 通道汇入 AL 瞌小球 |
| 11 | 神经调质 | 多巴胺 / 章鱼胺 | octopamine 经验驱动 → 探索强度/轨迹阈值/攻击性 |
| 13 | 发育可塑性 | MB 神经发生 | KC 随觅食经验从 24 长到 64（一生内） |
| 14 | CPG 步态 | 三角步态 | 6 腿 tripod 相位 + octopamine → 步频 → 速度 |
| 16 | 多巴胺 RPE 化 | PAM / PPL1 双 DAN | reward→LTP、punish→LTD，伤害驱动 aversive 学习 |
| 17 | 饱食调制 | 能量门控 | 奖励 gain 随能量缺口上升（饥饿学得更多） |
| 18 | 分室化 DAN | approach/avoidance | 正/负关联在分室输出上并行学习 |

## 演化与选择

- **算子**：高斯变异 + BLX-α 交叉；能量经济学、ANN 权重、CPPN 基因、神经模块基因均可演化。
- **复合适应度**：`score = collected + 0.3·mean_def_frac·colony + 0.2·survival·colony`。
- **多样性保持**：`--niche` 适应度共享（基因型距离）+ `--novelty` 新奇度搜索（NSLC-lite，行为新颖度驱动）。
- **多水平选择**：`--evolve-ml` 按各蚁 lifetime 交付做个体级选择。
- **开放性**：基因型 + 行为双多样性度量，要求二者均不坍缩（open-ended 判据）。

## 验证体系

| 命令 | 验证内容 |
|---|---|
| `--validate [--evolved]` | 行为涌现电池：6 脑 × 5 环境，5 判据（survival/trail/caste/growth/foraging） |
| `--ablate <mech>` | 8 机制消融 → 可证伪预测 + 湿实验 protocol（见 `results/ablations.md`） |
| `--compare-lit` | 模型 vs 文献定量对照（含 CX/MB 神经模块） |
| `--bench-cx` | CX 路径积分误差 vs 沙漠蚁 Cataglyphis |
| `--verify-determinism` | rayon 并行下逐字节确定性 |

## 关键结果

- **ACO 双桥**：短/长路径 Trail 峰值比 ~7×，短路径占比 ~88%（对齐 Deneubourg/Goss 80–90%）。
- **分工品级**：forager ~70%、guard+nurse ~30%（对齐 Gordon 的社会昆虫分工区间）。
- **基因 × 环境特化**：rich_close 冠军低探索（剥削近食）、scarce/maze 冠军高探索、predator 冠军高 aggression + 防御特化。
- **演化适配硬环境**：默认脑在 scarce_far/maze 饿死 → 演化冠军存活并增长（`--validate --evolved`）。
- **CX 路径积分**：漂移 0.0–0.3%，比真实沙漠蚁（8–12%）更准（无传感噪声，定性缩放一致）。
- **全脑 × 环境 × 视野大扫（Tier 24）**：10k 视野 FSM 全环境第一（总分 64437 = 2.4× 第二名 CX）；架构排名 **fsm > cx > cppn > ann > mb > snn** 在 3k/10k 两个视野下稳定；CX 是最强神经脑（路径积分随视野 1.5–3× 缩放）。
- **性能**：100k 蚂蚁 @ 136 tick/s（rayon 并行 + 双缓冲场 + 空间哈希）。
- **确定性**：per-ant RNG + 串行 flush，并行下逐字节可复现。

## 命令行参考

```bash
# 涌现行为
cargo run --release -- --headless --bridge  --ticks 6000 --colony 500   # 教科书双桥 ACO
cargo run --release -- --headless --scenario two --ticks 6000           # 双源最短路径
cargo run --release -- --headless --caste --colony 400                  # 工/兵分工比
cargo run --release -- --headless --brood --ticks 1000                  # 育幼 stigmergy
cargo run --release -- --headless --coevolve --rounds 10                # 两群协同进化（军备竞赛）

# 基因 × 环境演化
cargo run --release -- --headless --evolve-all --gens 8 --pop 10        # 5 环境各演化
cargo run --release -- --headless --evolve --brain cppn --env rich_close --gens 50 --pop 30 --niche  # 深度演化
cargo run --release -- --headless --evolve --brain fsm --env maze --ticks 3000 --out-stem evolved_fsm_maze_long  # 长视野演化
cargo run --release -- --headless --zoo --ticks 1000                    # 冠军 vs 默认
cargo run --release -- --headless --transfer --ticks 800                # 跨环境迁移矩阵

# 验证 / 文献
cargo run --release -- --headless --validate --ticks 3000               # 行为涌现电池
cargo run --release -- --headless --validate --evolved --env scarce_far --ticks 3000  # 演化冠军在硬环境
cargo run --release -- --headless --ablate octopamine --env scarce_far --ticks 3000    # 消融 → 可证伪预测
cargo run --release -- --headless --compare-lit                          # 文献定量对照
cargo run --release -- --headless --bench-cx                             # CX PI 误差 vs Cataglyphis

# 工程
cargo run --release -- --headless --bench --colony 100000               # 100k 性能基准
cargo run --release -- --headless --export-html results/snap.html       # HTML 快照
cargo run --release -- --headless --report                              # 生成 REPORT.md
cargo run --release -- --headless --verify-determinism --ticks 1000     # 确定性
```

## 性能

| 规模 | 吞吐 |
|---|---|
| 10k 蚂蚁 | ~1086 tick/s |
| 100k 蚂蚁 | ~136 tick/s |

性能关键：rayon 并行蚂蚁步、双缓冲信息素场、空间哈希近邻查询（战争 O(n·k) 替代 O(n²)）、`thin` LTO + `codegen-units=1`。

## 可复现性

- 固定种子（默认 seed=42，可用 `--seed` 覆盖）。
- 每蚁自带 `ChaCha8Rng`（seed = world_seed + index），并行确定性。
- 信息素沉积/攻击在并行步入队，主循环串行 flush（无并发写竞争）。
- `--verify-determinism` 双跑指纹逐字节一致。

## 目录结构

```
src/
├── main.rs            # CLI 入口 + headless 模式分发
├── genome.rs          # 基因组 + 演化算子
├── config.rs          # TOML 配置
├── environment.rs     # 5 环境预设
├── evolution.rs       # 适应度 + GA + 多水平选择 + 新奇度搜索
├── sim.rs             # tick 循环
├── world/             # 网格 / 信息素场 / 空间哈希 / 实体
├── ant/               # Ant + 决策脑（FSM/ANN/SNN/MB/CX）+ 感官
└── viz/               # egui 可视化
results/               # 演化产物 + 验证报告 + 消融 protocol
docs/                  # research_scope.md 等研究文档
```

## 文档

- `REPORT.md` — 结果报告（冠军矩阵 / 迁移 / 文献对照 / Tier 24 全扫）
- `PROGRESS.md` — 逐层开发日志
- `docs/research_scope.md` — 可验证 vs 不可验证范围界定
- `results/ablations.md` — 8 机制消融 + 湿实验 protocol
- `CLAUDE.md` — 项目内部约定

## 许可证

[MIT](LICENSE) © 2026 ants-sim contributors
