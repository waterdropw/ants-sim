# 昆虫脑架构重构规格

- 状态：提案（尚未实施）
- 依据：Berg et al. (2026), *Sexual dimorphism in the complete Drosophila male central nervous system connectome*, **Cell** 189, 5504–5526. DOI: 10.1016/j.cell.2026.08.015；本地副本 `PIIS0092867426009426.pdf`。
- 相关范围：`docs/research_scope.md`

## 1. 目标与证据边界

本规格以该论文给出的**全中枢神经系统、从感觉到运动的有向连接组织原则**为架构启发，改造 ants-sim 的抽象昆虫脑。论文报告雄果蝇 CNS 含 166,700 个神经元、11,710 个类型，并强调：感觉到运动的前馈信息流、从中央脑到腹神经索的下降通路瓶颈、由上行通路反馈形成的感觉—运动闭环；它还指出多数感觉/运动外围类型在雌雄间较保守，而高阶整合中心与连接重路由更集中地承载差异。

本项目**不**尝试复刻果蝇连接组、细胞数、性二态、神经递质预测或将果蝇回路直接等同于蚂蚁回路。蚂蚁与果蝇的神经元量级接近不足以支持同源性或定量外推。本规格只提取以下可迁移的建模原则：

1. 让感觉、整合、下降动作、身体/环境反馈成为显式接口，而非让每个脑模块直接改写 `heading`。
2. 将快速先天感觉通路与经验依赖的高阶通路分开，再在动作选择处会合。
3. 用可控的有向稀疏模块图表示连接，而不是把所有功能压进一个 MLP 或一个 FSM 分支。
4. 使学习、神经调质、结构可塑性和行为读出存在可追踪的因果链。
5. 每项机制必须有基线、配对消融、至少一个反事实任务与可复现的量化读出。

按 `docs/research_scope.md` 的约束，所有结果只可表述为“本模型、该参数、该读出下的机制内结果”；不得称为蚂蚁神经回路、物种级预测或果蝇连接组验证。

## 2. 当前基线审计

### 2.1 现有模块和数据流

| 层级 | 当前实现 | 实际职责 | 主要位置 |
|---|---|---|---|
| 感觉 | 8 方向信息素最大采样、食物/敌人邻近、前向锥视觉 | 输出浓度、世界坐标 bearing、距离和接触事件 | `src/ant/sensors.rs` |
| 快速控制 | 6 状态 FSM | 探索、趋化、归巢、警戒、防御和育幼；直接修改 `heading` | `src/ant/brain.rs` |
| 演化控制器 | 9→10→5 MLP、CPPN 发育 MLP | 直接输出转向、沉积和攻击 | `src/ant/brain.rs`, `src/genome.rs` |
| 脉冲控制器 | 9→10→5 LIF | 跨 tick 膜电位；W2 上 DA 门控 STDP | `src/ant/brain.rs`, `src/ant/mod.rs` |
| MB-like | 20→12 AL、12→64 KC、64→5 输出 | AL 侧抑制/振荡、KC 稀疏编码、KC→输出和 AL→KC 可塑性 | `src/ant/brain.rs`, `src/genome.rs` |
| CX-like | 16 单元环吸引子和 home-vector | 估计航向、路径积分；仅 `--brain cx` 时替代软件归巢向量 | `src/ant/brain.rs`, `src/ant/mod.rs` |
| 神经调质 | reward/punish dopamine、octopamine | DA 门控 SNN/MB 可塑性；OA 调节 FSM 阈值、探索和 CPG 频率 | `src/sim.rs`, `src/ant/mod.rs`, `src/ant/brain.rs` |
| 动作 | 六足 tripod 相位与速度缩放 | 开环步态相位，实际运动仍是二维位置更新 | `src/ant/mod.rs` |
| 社会/环境耦合 | 四通道场、育幼、共享能量、近战 | 间接协同；无近距离感觉—个体通信回路 | `src/world/`, `src/sim.rs` |

当前 tick 顺序为：化学场更新 → 并行感知与决策 → 串行沉积/攻击/食物/奖励 flush → 能量、战争、育幼、出生等。该设计在 rayon 下可确定性复现，应保持这一工程属性。

### 2.2 已验证的实现约束

- 双桥使用实际运动段跨门线计数，且用 BFS 审计短、长路径几何；新神经模块不得将信息素浓度直接当作交通读出。
- Trail 衰减已接入化学场；基因组参数扫描必须实际改变它声称扫描的动态量。
- `--ablate` 的相同种子配对比较是模型内反事实协议；新增模块必须进入该协议或提供等价配对控制。
- 并行蚂蚁步只能读取 `World` 并写入每只蚂蚁的 pending 事件；跨个体效果须在确定性串行 flush 中汇总。

## 3. 论文启发到项目要求的映射

| 论文中可借鉴的原则 | 当前缺口 | 本规格要求 |
|---|---|---|
| 全链路感觉→运动分析 | 感觉、脑模块与运动相互直连，无法审计中间瓶颈 | 引入明确的 `SensoryCode`、`ActionCommand`、`MotorFeedback` 接口和每 tick telemetry |
| 前馈连接多为兴奋性、反馈更偏抑制性 | 只有局部 AL 抑制；没有模块级反馈语义 | 在高阶整合↔感觉/动作回路中显式声明激发、抑制或调制边类型；不要求生物递质拟合 |
| 下行/上行通路是脑—运动系统的瓶颈和整合器 | 任意脑模块可直接改 `heading`；身体反馈缺失 | 用下降动作读出汇聚所有脑模块；用上行运动/接触反馈输入状态估计和学习 |
| 感觉外围相对保守，高阶中心重路由/整合更灵活 | 所有参数均混在同一 Genome，不能区分感觉增益、整合连接与动作读出 | 将基因组参数按感觉、整合、动作、调质、可塑性分组；演化距离覆盖全部组 |
| 嗅觉存在单小球与多小球投射、并行投向高阶先天/学习中心 | Trail 标量直接进控制器；MB 的 AL 输入缺少 ORN/PN 语义 | 构造 ORN→glomerulus→PN 表示；将单通道快速反射和多通道整合分别送到 LH-like 与 MB-like 路径 |
| 运动产生可经上行通路影响中央感觉/动作处理 | CPG 开环；没有负载、触角接触或行动后反馈 | 增加低维 `MotorFeedback`；至少包含实际位移、碰撞/壁接触、步态相位与能量代价 |

## 4. 必须修复的一致性问题

### NAR-001：明确 ANN 是否具有在线学习

当前 `ann_decide` 使用 `ant.genome.ann_weights`，而 SNN 使用 `ant.learned_w`。因此 ANN 当前是纯演化基线，不能称为具有 DA/STDP 生命周期学习的控制器。

**要求**：近期默认选择“ANN 是固定演化基线”。文档、报告和比较表必须只将在线可塑性归于 SNN/MB。若未来启用 ANN 在线学习，必须单独定义非脉冲 eligibility 规则、更新时机、消融和任务，而不能隐式复用 SNN 表述。

### NAR-002：遗传距离必须覆盖神经相关字段

`Genome::trait_vec()` 未包含 `mb_weights`、两类 DA 增益、饱食增益、神经发生率和 `cpg_freq`。这会让 niche sharing 忽略主要神经表型差异。

**要求**：距离向量必须覆盖所有可演化字段。高维权重可使用逐权重归一化或固定维度的结构描述符，但必须记录选择方法并保持比较前后维度可解释。

### NAR-003：CX 罗盘观测与真实朝向分离

当前 CX 以 `ant.heading` 直接锚定 bump。它适合作为数值积分基线，但不是独立的感觉罗盘。

**要求**：引入 `CompassObservation`，其中可控地含噪声、偏置、遮挡与重获；真实朝向仅用于世界运动和基准误差，不得直接作为 CX 的感觉注入。`--bench-cx` 必须分别报告无噪声积分、带噪声罗盘和遮挡恢复结果。

## 5. 目标抽象架构

目标不是全脑仿真，而是如下可审计的低维有向模块图：

```text
World
  ├─ chemical / visual / contact / body signals
  ▼
SensoryCode
  ├─ ORN-like channel responses
  ├─ visual-compass / landmark features
  └─ contact + motor feedback
  ▼
Early sensory circuits
  ├─ AL-like glomeruli → PN-like projections
  └─ fast LH-like innate reflex
  ▼
Higher integration
  ├─ MB-like sparse context/value learning
  ├─ CX-like state estimate + navigation policy
  └─ internal state: energy, age, OA/DA, task context
  ▼
Action selection / descending pathway
  ├─ competing approach, avoidance, homing, explore, defend commands
  └─ explicit command confidence / inhibition
  ▼
Motor layer
  ├─ turn, speed, deposit, attack, task switch
  └─ CPG-like gait state
  ▼
MotorFeedback + environment effects ───────────────────────────┘
```

### 5.1 `SensoryCode`

**最小内容**：每个化学通道的多受体响应、方向特征、视觉罗盘观测、地标/食物特征、触角/墙接触、近邻接触、身体能量和动作反馈。

**约束**：

- 物理世界仍可保留四个抽象信息素场；受体数量不代表真实受体家族。
- 应支持饱和、适应、观测噪声和遮挡；这些参数必须可关停以提供数值基线。
- 传给 MB 的是编码向量，而不是世界真值坐标或直接目标向量。

### 5.2 AL/PN/LH-like 快速通路

- ORN-like 输入汇聚到固定数量的 glomerulus-like 单元；保留局部抑制与增益控制。
- PN-like 投射区分单通道特异路径和多通道整合路径。
- LH-like 通路只产生先天偏置，不直接覆盖所有行为；其输出作为 `ActionCommand` 的一个证据来源。
- 必须能够单独消融 ORN、AL 抑制、PN 多通道整合和 LH 快速反射。

### 5.3 MB-like 学习通路

- 保留 AL→KC 稀疏编码、KC→MBON 式读出和 DA 调制。
- 添加显式 approach 与 avoidance 价值读出及竞争/抑制，避免仅由输出下标区分价性。
- 最终转向不能长期绕过 MB；LH 负责快速先天反射，MB 提供经学习的价值偏置，两者在动作选择器中合成。
- 用预测误差而不是单纯 pickup/delivery 脉冲门控学习：

\[
\delta_t = r_t + \gamma V_{t+1} - V_t
\]

  其中 \(V\) 是由当前情境编码估计的价值，\(\delta\) 为正负 DA-like 调制信号。必须具有可清除的 eligibility trace，支持延迟后果归因。

### 5.4 CX-like 导航通路

- 环吸引子接收 `CompassObservation` 与角速度/自运动线索。
- 路径积分产生状态估计，并通过 CX→动作读出产生相对方向的转向建议；不可只作为软件 `home_vector` 的替代存储。
- 地标与罗盘冲突、罗盘遮挡和路径积分漂移必须是独立可控的条件。
- CX 可作为任何脑模式可复用的导航子模块；`--brain cx` 仍可保留为仅使用该模块的对照，而不是唯一能使用 CX 的模式。

### 5.5 下降动作与上行动作反馈

定义固定、可记录的 `ActionCommand`：

- `turn_drive`：[-1, 1]；
- `speed_drive`：非负；
- `deposit`：按四化学通道分量；
- `attack_drive`、`task_switch_drive`；
- 每个候选动作的 approach/avoidance/uncertainty 证据。

动作选择器必须应用竞争、抑制和边界，而不是由各脑模块直接覆写 `heading`。运动后构造 `MotorFeedback`：实际移动距离、实际转角、墙/障碍接触、步态相位、能量消耗、成功/失败的动作执行。它是 CX 状态估计、CPG 调制及 MB eligibility 的共同输入。

### 5.6 社会感觉与通信

保留信息素 stigmergy，但增加可选的低维近邻触角通路：接触、同巢身份、招募意向、威胁/食源状态。第一阶段不实现真实化学身份识别、不实现蜂群式全局通信，也不允许并行更新中的跨蚂蚁可变共享状态；近邻事件必须排队后串行结算。

## 6. 观测、配置与兼容性

### 6.1 Telemetry

新模块必须按 tick 或固定采样窗口输出：

- 感觉编码范数/活跃度、各通道适应量；
- AL/PN/KC/MBON/CX 活跃度的低维汇总；
- `ActionCommand` 各来源与最终胜者；
- RPE、eligibility 范数和权重漂移；
- compass 误差、路径积分误差、碰撞和步态/能量指标；
- 个体接触、招募/拒绝事件以及跨事件的行为结果。

不以内部神经活性替代行为指标。所有神经读出必须同时与至少一个外部可观测量（交付、路线误差、交通、存活、任务预算或接触成功率）关联报告。

### 6.2 配置与演化

- 所有新增参数给出默认值、范围、序列化默认值、突变、交叉、随机初始化和有效性检查。
- 新增参数组应带前缀：`sensory_*`、`al_*`、`mb_*`、`cx_*`、`motor_*`、`social_*`。
- 旧 TOML 必须通过 `serde(default)` 加载；配置打印必须显示实际生效值。
- 每次增加可演化字段时，同时更新 `trait_vec()` 与相关多样性报告。

### 6.3 性能与确定性

- 默认 3k/10k 群体实验不可出现数量级性能退化；高维神经轨迹仅在显式 telemetry 模式开启。
- 不引入依赖 rayon 任务调度顺序的随机源。
- 每个新增 protocol 都必须通过 `--verify-determinism` 或等价的双跑指纹测试。

## 7. 不在本规格范围内

- 逐神经元复刻 166,700 个果蝇 CNS 神经元或 11,710 个类型；
- 果蝇性二态/`fruitless`/`doublesex` 的遗传实现；
- 将果蝇嗅觉、听觉、味觉、视觉细胞类型直接认作蚂蚁同源细胞；
- 真实触角化学、复眼光学、肌肉动力学、神经递质动力学或神经解剖校准；
- 把本模型的模块消融表述为真实脑区、分子或细胞类型的必要性/充分性证明。

## 8. 验收标准

1. 代码层：各层之间只经公开数据结构交互；新的快速反射、MB、CX 与动作选择器均可独立启停。
2. 科学层：每项新增回路至少具有一个机制任务、一个配对消融和一个不支持其解释的负对照条件。
3. 行为层：不以训练/演化后的单一得分验收；至少报告跨种子平均、标准差、样本量、流量/路线/存活等行为读出。
4. 解释层：报告清晰区分“世界真值”“感觉观测”“内部状态估计”“动作命令”和“实际运动”。
5. 工程层：`cargo test`、格式检查、指定协议的确定性检查和既有双桥几何审计均通过。

## 9. 参考定位

- Berg et al. (2026)，摘要与全文第 2 页：全 CNS 连接组规模及高阶中心集中差异。
- Berg et al. (2026)，全文第 4 页 / Figure 2：感觉到运动的前馈组织、下降/上行通路的瓶颈与反馈整合、端到端最大流分析。
- Berg et al. (2026)，全文第 11–12 页 / Figure 5：感觉检测与动作生成之间的递归感觉—运动回路。
- Berg et al. (2026)，全文第 11–12 页 / Figure 6：单/多小球 AL 投射、LH 与 MB 等高阶嗅觉路径、奖惩 DA 信号作为高阶差异的背景。
- `docs/research_scope.md`：模型内证据和外推边界。
