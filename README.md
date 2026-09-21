# ants-sim

蚂蚁个体智能 / 群体智能 / 行为模拟系统，含**基因 × 环境演化闭环**。Rust + egui 0.36。

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Language](https://img.shields.io/badge/language-Rust-orange.svg)
![egui](https://img.shields.io/badge/egui-0.36-8b5cf6.svg)
![Tests](https://img.shields.io/badge/tests-48%20pass-green.svg)

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
- [可复现模型内协议结果](#可复现模型内协议结果)
- [关键结果](#关键结果)
- [稳健演化与保留集复评](#稳健演化与保留集复评)
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
- **涌现行为**：双桥门线交通、实时行为状态预算（觅食/防御/育幼）、领地战争、育幼 stigmergy、竞争敏感性。
- **科学检查**：内部行为涌现电池（`--validate`）、配对多 seed 的模型内反事实敏感性（`--ablate`）、带定义差异的文献上下文对照（`--compare-lit`）、CX 数值路径积分基准（`--bench-cx`）。
- **可复现 + 可扩展**：rayon 并行下逐字节确定性；100k 蚂蚁 @ 136 tick/s。

## 快速开始

需要 Rust 工具链（如无：`brew install rustup-init && rustup default stable`）。

```bash
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
cargo run --release            # 打开 GUI
cargo test                     # 48 个回归与性质测试
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

本项目以工程尺度模块抽象感知—整合—行动—反馈闭环，支持可演化参数与独立消融验证；并非真实蚂蚁连接组或细胞类型的复刻。MB 的关联学习逻辑以果蝇蘑菇体结构和多巴胺价值信号研究为功能参照 [Aso et al., 2014](https://doi.org/10.7554/eLife.04577)、[Liu et al., 2012](https://doi.org/10.1038/nature11304)、[Aso et al., 2010](https://doi.org/10.1016/j.cub.2010.06.048)；CX-like 路径积分以环吸引子和方向整合研究为参照 [Seelig & Jayaraman, 2015](https://doi.org/10.1038/nature14446)。完整参考文献和外推边界见 `docs/research_scope.md`。

| 模块 | 功能参照 | 当前实现与可报告读出 |
|---|---|---|
| 蘑菇体（MB） | AL—KC—输出通路与关联学习 | 12 个 AL 单元汇入最多 64 个稀疏 KC，再投射到 5 个输出；支持双通道多巴胺门控 STDP、资格痕迹和神经发生。`--phase7` 输出逐试次突触与输出证据。 |
| 中央复合体样模块（CX-like） | 环吸引子方向表征与路径积分 | 16 单元环吸引子整合带噪罗盘观测和运动反馈，输出归巢向量；`--bench-cx` 仅量化抽象积分器的数值误差。 |
| 多模态感知整合 | 近距离视觉、化学与本体状态的汇合 | ORN 适应与噪声、AL 侧抑制、PN 单/混合通道及 LH-like 快速转向证据汇入 MB/integrated 控制器。 |
| 神经调质 | 多巴胺价值信号与章鱼胺—多巴胺层级调制 | reward/punish 标量分别门控接近/回避通路可塑性；OA-like 状态调制探索、线索响应、攻击和步频。对应关系仅为功能类比。 |
| 发育可塑性 | 经验相关的有效回路容量变化 | KC 有效数随觅食经验由 24 增长并上限为 64；这是模型内发育变量，不对应真实神经发生率。 |
| CPG 步态 | 交替三足步态 | 六腿两相位 tripod 振荡器，OA-like 唤醒调制频率与速度；不拟合真实关节动力学。 |
| 能量与分室化价值调制 | 饱食状态、接近/回避价值通路 | 奖励增益随能量缺口调整；接近与回避输出使用相反的 reward/punish 可塑性门控。 |

## 演化与选择

- **算子**：高斯变异 + BLX-α 交叉；能量经济学、ANN 权重、CPPN 基因、神经模块基因均可演化。
- **复合适应度**：`score = collected + 0.3·mean_def_frac·colony + 0.2·survival·colony`。
- **多样性保持**：`--niche` 适应度共享（基因型距离）+ `--novelty` 新奇度搜索（NSLC-lite，行为新颖度驱动）。
- **多水平选择**：`--evolve-ml` 按各蚁 lifetime 交付做个体级选择。
- **开放性**：基因型 + 行为双多样性度量，要求二者均不坍缩（open-ended 判据）。

## 验证体系

| 命令 | 验证内容 |
|---|---|
| `--validate [--evolved]` | 内部行为涌现电池：6 脑 × 5 环境，5 判据（survival/trail/task budget/growth/foraging） |
| `--ablate <mech> [--n-seeds N]` | 配对随机种子的模型内反事实敏感性；不会推断真实生物机制充分性（见 `results/ablations.md`） |
| `--compare-lit` | 模型读出与代表性文献锚点的描述性对照（显式显示定义/尺度差异） |
| `--bench-cx` | 抽象 CX 积分器的数值误差基准；不是沙漠蚁生物学拟合 |
| `--verify-determinism` | rayon 并行下逐字节确定性 |

所有“验证”均限于固定代码、参数、环境与读出的模型内命题；哪些读出可报告、哪些外部结论不可由本项目推出，见 [研究范围与证据边界](docs/research_scope.md)。

## 可复现模型内协议结果

以下结果来自仓库内的逐 seed CSV，而非对真实昆虫的行为、神经回路或生态适合度断言。每项均应按其预先定义的**开关—读出**关系理解；`docs/research_scope.md` 中列出的不可验证项（物种级定量一致、真实必要性/充分性、学习动力学拟合及田间生态适合度等）不因这些结果而得到验证。

### MB 内部可塑性协议

运行 `--phase7 --n-seeds 8` 在 8 个配对 seed 下得到 128 行逐试次突触读出（`results/phase7_mb_learning.csv`）。表中“完整”指 STDP 开启，“消融”指同一输入下关闭 STDP；权重是 MB 输出层绝对权重和，`approach_evidence` 是内部输出单元值，不是转向、选择或动物记忆行为。

| 预定义比较 | 完整模型均值 | STDP 消融均值 | 可验证结论 |
|---|---:|---:|---|
| 获取：接近权重和 | 12.235 | 10.800 | 在此抽象 CS—reward 序列中，开启 STDP 的输出层权重和高 1.435。 |
| 惩罚序列：回避权重和 | 5.116 | 4.000 | 在此抽象 CS—punish 序列中，开启 STDP 的回避输出层权重和高 1.116。 |
| 资格迹内延迟（6 tick）：接近权重和 | 12.235 | — | 作为延迟读出基准。 |
| 资格迹外延迟（12 tick）：接近权重和 | 11.933 | — | 相对 6 tick 低 0.302（2.5%）；与实现中的有限资格迹窗口一致。 |
| 复合线索：A 预训练后探测 B | 14.562 | — | 高于未预训练 A 的 B 对照（12.829），方向并非经典阻断所要求的对 B 学习减弱。 |
| 无奖励重呈（“消退”标签）：接近权重和 | 12.304 | 获取为 12.235 | 未观察到相对获取的下降，不能将该结果报告为模型内消退。 |

因此，当前数据支持“STDP 开关改变内部突触读出”和“延迟超过实现窗口后读出减弱”这两项模型内命题；不支持将协议标签直接升级为获取、消退、反转、阻断或效价记忆的**行为学**结论。若要验证这些标签，须预先定义基于行动输出的 CS 选择指标、方向性判据和适当的零/反向对照。

### 信息素与接触招募矩阵

运行 `--phase8 --env scarce_far --n-seeds 5 --ticks 3000 --colony 200` 产生 20 个配对条件行（`results/phase8_recruitment.csv`）。相同 seed 在 2×2 设计中切换 Trail 信息素与社会接触；`contact_events` 是模拟器记录的实际空间接触次数，而非接触开关本身。

| Trail 信息素 | 社会接触 | 收集量均值 | Trail 场总量均值 | 接触事件均值 | `recruitment_total` 最大值 |
|---|---|---:|---:|---:|---:|
| 关 | 关 | 91.8 | 0.000000 | 0.0 | 0.000251 |
| 关 | 开 | 99.8 | 0.000000 | 1004.2 | 0.000000 |
| 开 | 关 | 113.6 | 0.018783 | 0.0 | 0.000000 |
| 开 | 开 | 110.2 | 0.031758 | 1996.2 | 0.000001 |

该矩阵验证了操纵检查：开启 Trail 后场读出非零，开启接触后出现实际接触事件，且两者同时开启时接触事件最多。与此同时，所有条件的 `recruitment_total` 都接近零（最大值 0.000251），没有证据表明本设置下存在可分辨的招募增效、协同效应或对收集量的稳健正效应；这些结论必须明确报告为**未获支持**，而不是负向生物学结论。

## 关键结果

- **双桥交通测量**：`--bridge` 以蚂蚁运动线段跨越两臂门线的返巢流量计算短路偏置；不再由信息素峰值推断交通比例。
- **任务预算测量**：`--caste` 在 warm-up 后报告实时 `State` 的觅食/防御/育幼时间均值与标准差；它不是形态品级比例。
- **基因 × 环境差异（模型内）**：在当前进化预算与合成环境中，rich_close 冠军呈低探索参数，scarce/maze 冠军呈高探索参数，predator 冠军呈高攻击性与防御参数；这是该参数化下的 G×E 读出，不是物种适应性结论。
- **硬环境复评（模型内）**：在指定评估窗口，默认基线在 `scarce_far`/`maze` 的存活读出为零，而对应演化配置产生非零存活与收集读出（`--validate --evolved`）；保留 seed 的稳健性结果见下表。
- **CX 数值基准**：`--bench-cx` 在已知朝向、无感觉噪声的抽象步行中测得 0.0–0.3% 积分误差；该结果不能与真实沙漠蚁误差直接比较或视为生物学拟合。
- **控制器扫描（模型内）**：在当前 3k/10k tick、五环境和既定适应度定义下，FSM 总分最高，排序为 **fsm > cx > cppn > ann > mb > snn**；这是算法归纳偏置比较，而非真实神经架构优劣。
- **性能**：100k 蚂蚁 @ 136 tick/s（rayon 并行 + 双缓冲场 + 空间哈希）。
- **确定性**：per-ant RNG + 串行 flush，并行下逐字节可复现。

## 稳健演化与保留集复评

为检验演化结果是否仅来自特定随机轨迹，本实验对 integrated brain 在五类合成环境中分别执行多随机种子训练，并以与训练 seed 隔离的保留集进行冠军—默认基线复评。每个环境的训练预算为 `12 generations × 12 population × 3000 ticks × 3 training seeds`；部署复评为 `6000 ticks × 5 held-out seeds`。适应度定义为 `score = collected + 0.3·mean_def_frac·colony + 0.2·survival·colony`，因此应同时报告资源收集、防御状态和存活，而非仅比较单一分数。逐 seed 原始读数见 `results/phase9_robust_evolution.csv`；对应冠军配置见 `results/phase9_integrated_<env>.toml`。

| 环境 | 训练期最佳分数 | 默认基线：保留集均分 | 冠军：保留集均分 | 冠军：保留集最低存活率 | 当前判定 |
|---|---:|---:|---:|---:|---|
| `rich_close` | 4463.667 | 9418.880 | 9418.880 | 1.000 | 无增益证据 |
| `scarce_far` | 364.800 | 122.200 | 659.760 | 0.670 | 条件性采用 |
| `predator` | 663.800 | 667.840 | 667.840 | 0.015 | 无增益且存活不足 |
| `patchy` | 2250.467 | 306.000 | 3550.800 | 0.385 | 收集增益存在，但稳健性不足 |
| `maze` | 526.867 | 34.200 | 969.560 | 0.975 | 条件性采用 |

**判定规则。** 这是一个工程化的模型内筛选门：仅在 MB/招募/复评 CSV 均存在，且冠军保留集平均分严格高于默认基线、所有保留 seed 的最低存活率不低于 0.5 时，才标记为“条件性采用”。它不要求也不表示前述 MB 协议已验证动物学习、招募矩阵已验证社会招募。按此机械规则，`scarce_far` 的冠军均分为默认基线的约 5.40 倍，`maze` 为约 28.35 倍；两者通过当前门槛。`patchy` 的均分提升约 11.60 倍，但最低存活率为 0.385，故不应作为稳健部署配置。`rich_close` 与 `predator` 未观察到相对默认基线的部署增益。

**解释范围。** 这是一项固定实现、合成环境、有限训练预算和指定随机协议下的模型内泛化检查，不是局部适应或生态适合度的物种级检验。关于 G×E 外推的概念边界可参见 [Kawecki & Ebert, 2004](https://doi.org/10.1111/j.1461-0248.2004.00684.x)；本项目的完整证据范围、外部假设及文献清单见 `docs/research_scope.md`。

## 命令行参考

```bash
# 涌现行为
cargo run --release -- --headless --bridge --ticks 6000 --colony 500   # 双桥实际门线交通（短/长返巢流量）
cargo run --release -- --headless --scenario two --ticks 6000           # 双源最短路径
cargo run --release -- --headless --caste --env predator --ticks 3000 --colony 400  # 实时任务状态预算
cargo run --release -- --headless --brood --ticks 1000                  # 育幼 stigmergy
cargo run --release -- --headless --coevolve --rounds 10                # 两群协同进化（军备竞赛）

# 基因 × 环境演化
cargo run --release -- --headless --evolve-all --gens 8 --pop 10        # 5 环境各演化
cargo run --release -- --headless --evolve --brain cppn --env rich_close --gens 50 --pop 30 --niche  # 深度演化
cargo run --release -- --headless --evolve --brain fsm --env maze --ticks 3000 --out-stem evolved_fsm_maze_long  # 长视野演化
cargo run --release -- --headless --zoo --ticks 1000                    # 冠军 vs 默认
cargo run --release -- --headless --transfer --ticks 800                # 跨环境迁移矩阵
cargo run --release -- --headless --phase7 --n-seeds 8                   # MB 学习协议：获取/消退/反转/阻断/延迟/效价
cargo run --release -- --headless --phase8 --env scarce_far --n-seeds 5 # 信息素 × 接触招募 2×2 矩阵
cargo run --release -- --headless --phase9                               # 全环境多 seed 训练与保留集复评
cargo run --release -- --headless --phase10                              # 汇总协议证据并输出当前判定

# 验证 / 文献
cargo run --release -- --headless --validate --ticks 3000               # 行为涌现电池
cargo run --release -- --headless --validate --evolved --env scarce_far --ticks 3000  # 演化冠军在硬环境
cargo run --release -- --headless --ablate octopamine --env scarce_far --ticks 3000 --n-seeds 5 --out results/ablate_oa.csv  # 配对模型内敏感性
cargo run --release -- --headless --compare-lit                          # 文献锚点的描述性对照
cargo run --release -- --headless --bench-cx                             # 抽象 CX PI 数值误差

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
- `docs/research_scope.md` — 模型内证据、外部假设、限制与参考文献
- `results/ablations.md` — 8 个开关的模型内反事实敏感性协议与解释边界
- `CLAUDE.md` — 项目内部约定

## 许可证

[MIT](LICENSE) © 2026 ants-sim contributors
