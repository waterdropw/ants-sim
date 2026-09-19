# ants-sim

蚂蚁个体智能 / 群体智能 / 行为模拟 + **基因×环境演化闭环**。Rust + egui 0.36。
六层隔夜自治迭代全完成（git 已初始化，首次提交 169f236）。Tier 8–15 神经模块化与 MB 觅食活力修复续做。

## 分层架构
`viz → sim → world + ant` + `evolution` + `environment` + `config` + `genome(CPPN)`。

| 文件 | 职责 |
|---|---|
| `src/genome.rs` | 基因组(30+字段) + mutate(高斯)/crossover(BLX)/random/in_range + foraging_ann_seed + develop_cppn + distance + 10 单元测试 |
| `src/config.rs` | TOML 配置 (--config/--print-config) |
| `src/sim.rs` | tick 循环：化学步→rayon并行蚂蚁步→串行flush(沉积/攻击/食物递减/战争/brood); 含场景方法 + 性质测试 |
| `src/world/` | 网格、信息素场(双缓冲5点扩散+按通道衰减)、空间哈希、巢/食物/敌人/墙/brood |
| `src/ant/` | Ant(含 health/energy/colony_id/dead) + FSM(brain.rs,梯度上升chemotaxis) + ANN ann_decide + 感官(8方向梯度) |
| `src/environment.rs` | 5 预设(rich/scarce/predator/patchy/maze) + EnvMetrics |
| `src/evolution.rs` | evaluate/multi + GA run(niche+diversity) + run_multilevel(个体选择) |
| `src/viz/` | egui App: brain选择器 + 4通道缩略图 + sparkline + 场景编辑器 |

## 决策层 (`--brain`)
- `fsm`（默认）：子sumption FSM + 神经调质 + 梯度上升
- `ann`：固定 MLP(9→10→5)，权重可演化，门控输入+觅食种子
- `cppn`：间接编码——16 cppn_genes → 155 权重(余弦基发育)
- `mb`：蘑菇体(AL+MB+STDP+多巴胺+神经发生)——Tier 8/10/13/15/16/17/18，12瞌小球多模态→64稀疏KC→5输出；KC随经验 24→64。Tier 15 修复觅食活力(分级AL→turn反射+育幼eclosion)，evolved 3000t collected 304→2191。Tier 16 多巴胺RPE化：双通道(reward PAM/punish PPL1)门控STDP，reward→LTP、punish→LTD(基线遗忘+aversive增强)，伤害释放punish→aversive学习；ANN STDP 也纳入同款门控。Tier 17 饱食调制(奖励gain随能量缺口)+AL→KC Hebbian可塑性(基底KC,检测器保护)。Tier 18 分室化DAN(approach/avoidance opponent, PAM/PPL1分室)
- `cx`：中央复合体(环吸引子路径积分)——Tier 9，CX_N=16 神经积分读出(home_vector)，替代软件真值；演化后 out-of-box 597。`--bench-cx` 量化PI误差(0.0–0.3%) vs 沙漠蚁Cataglyphis
- 神经调质(Tier 11/16/18)：octopamine 经验驱动→探索/轨迹阈值/攻击性；dopamine 双通道(reward/punish)分室化门控 MB+ANN STDP；CPG(Tier 14)：6腿tripod+octopamine→步频→速度
- 演化(Tier 19)：novelty search(NSLC-lite, --novelty, 行为新颖度驱动选择) + 行为多样性度量(T18.2)；6脑全活力化(MB/SNN Tier15/19 修复, 不再垫底)

## 跑
```bash
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
cargo run --release                    # GUI
cargo test                             # 30 测试
cargo run --release -- --headless --report           # 重生成 REPORT.md
cargo run --release -- --headless --compare-lit      # 模型 vs 文献(含CX/MB神经模块)
cargo run --release -- --headless --bench-cx         # CX路径积分误差 vs Cataglyphis
cargo run --release -- --headless --export-html results/snap.html  # HTML 快照
cargo run --release -- --headless --bench --colony 100000           # 100k 性能
```
详见 README.md（全部命令）。结果在 REPORT.md + results/。

## 关键能力
- 演化: 高斯+BLX / niche适应度共享 / 多水平个体选择 / 协同进化(军备竞赛) / 开放性(多样性不坍缩)
- 神经模块(昆虫脑): MB蘑菇体(AL+MB+多巴胺STDP+神经发生) / CX环吸引子路径积分 / 多模态整合 / 神经调质(octopamine) / CPG三角步态
- stigmergy: Trail/Home/Alarm/Recruit + brood育幼 + 领地战争(群间近战)
- 验证: ACO双桥(峰值7×)/分工(~30%守卫)/确定性(逐字节)/文献定量对照/--validate 行为涌现电池(6脑×5环境, survival/trail/caste/growth/foraging)/--validate --evolved 演化适配硬环境(默认饿死→演化存活; 评估视野需匹配目标)/--ablate 8机制消融→可证伪预测+湿实验protocol(见 results/ablations.md, docs/research_scope.md)
- 工程: 30性质测试 / 100k@136tps / TOML+CSV+HTML / egui多通道GUI+场景编辑
