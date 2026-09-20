# 昆虫脑架构重构实施计划

- 状态：计划；不代表已实现
- 规格来源：`docs/neural_architecture_spec.md`
- 研究范围：`docs/research_scope.md`
- 策略：先修正解释性与接口，再引入回路闭环；每阶段可独立回归、消融和停止。

## 1. 总体原则

1. **不大爆炸重写**：保留 `FSM`、`ANN`、`SNN`、`MB`、`CX` 作为可比较基线；新架构通过新增模式或适配层进入，只有在验证后才替换默认行为。
2. **先行为，后内部指标**：每项神经改动必须预先声明行为读出、负对照和失败判据。
3. **所有机制效应做配对比较**：相同 seed、相同初始化、相同环境；报告逐 seed 原始 CSV、均值、样本标准差和有效样本数。
4. **真值不可泄漏**：世界的真实目标角度、真实位置和真实朝向不得作为脑模块感觉输入；仅能用于环境更新和误差基准。
5. **并行安全优先**：蚂蚁并行步只生成局部事件。个体接触、共享场、交付、奖励和社会信号统一在串行 flush 结算。
6. **旧配置可读取**：新增 Genome 字段采用 `serde(default)`；保留旧 `--brain` 命令语义；新增实验均显式命名。

## 2. 交付路线图

| 阶段 | 名称 | 目标 | 依赖 | 退出准则 |
|---|---|---|---|---|
| 0 | 解释性基线 | 修正控制器、基因距离与遥测语义 | 无 | 不再将 ANN 误报为在线学习；演化距离覆盖全部神经表型 |
| 1 | 脑—动作接口 | 将直接改 heading 改为可审计动作命令 | 阶段 0 | 所有基线脑通过同一动作选择/运动接口，既有行为不回归 |
| 2 | 感觉编码与早期回路 | ORN/AL/PN/LH-like 分层及视觉罗盘观测 | 阶段 1 | 感觉真值与观测分离；可独立消融各早期层 |
| 3 | CX 闭环导航 | CX 产生导航动作，接收噪声罗盘与动作反馈 | 阶段 1、2 | 有遮挡/冲突/漂移的导航协议，CX→motor 效应可证伪 |
| 4 | MB 价值学习 | MBON 竞争、RPE 和 eligibility trace | 阶段 1、2 | 学习改变选择且通过 acquisition/reversal/负对照验证 |
| 5 | 社会感觉与动作反馈 | 触角接触、局部招募、闭环 CPG 简化反馈 | 阶段 1 | 与纯信息素条件可分离比较，保持并行确定性 |
| 6 | 集成评估 | 跨脑、跨环境、跨机制组合的证据矩阵 | 阶段 0–5 | 性能、确定性、消融、迁移与报告全部通过 |

每阶段结束都可停止，不要求完成后续阶段才发布当前结果。

## 3. 阶段 0：解释性基线与技术债修复

### 3.1 任务 0A：界定 ANN 生命周期学习语义

**现状**：`ann_decide` 使用固定的 `genome.ann_weights`；`learned_w` 仅由 SNN 使用。

**实施**：

- 保持 ANN 为固定演化基线，不让 ANN 读取 `learned_w`。
- 校正注释、`--compare-lit` / 报告相关措辞：在线 DA/STDP 权重变化仅属于 SNN 与 MB。
- 增加单元测试：ANN 连续运行不得修改 `learned_w`；SNN 在满足 dopamine/spike 前提下允许修改对应 W2。

**验收**：ANN、SNN 的学习状态读出互不混淆；所有脑模式报告能标记“无在线学习 / STDP / RPE”。

### 3.2 任务 0B：补全 Genome 表型距离

**实施**：

- 将 `mb_dopamine_reward_gain`、`mb_dopamine_punish_gain`、`mb_satiety_gain`、`mb_neurogenesis`、`cpg_freq` 加入 `Genome::trait_vec()`。
- 为 `mb_weights` 采用可选的、文档化距离策略：
  - 第一版：逐权重、和 ANN 权重一致的归一化；
  - 如发现维度支配距离：记录并比较块级 RMS/稀疏度/符号比例描述符。
- 更新距离恒等性、差异性和向量长度测试。

**验收**：仅修改上述任一字段时，`Genome::distance(a,b) > 0`；随机、突变、交叉和 niching 不发生维度错配。

### 3.3 任务 0C：统一脑模式 telemetry

**实施**：定义低开销 `BrainTelemetry`，包含：脑模式、感觉激活概览、内部状态概览、动作命令概览、学习更新数/范数；默认只聚合，不逐蚂蚁保存大数组。

**验收**：所有 `--brain` 可输出相同字段的“无/有”状态；不改变默认 simulation 结果；确定性检查通过。

**阶段 0 测试**：`cargo test`；`--verify-determinism`；FSM/ANN/SNN/MB/CX 各在 `rich_close` 与 `scarce_far` 跑 5 个配对 seed；性能基线至少 10k colony。

## 4. 阶段 1：显式脑—动作接口

### 4.1 数据结构与职责

新增但不立刻删除现有行为逻辑：

- `SensoryCode`：模块可消费的感觉观测；
- `ActionEvidence`：每个模块对 turn、speed、deposit、attack、task 的支持/反对证据；
- `ActionCommand`：竞争后的唯一动作命令；
- `MotorFeedback`：实际移动、碰撞、实际转角、能量和 CPG 相关反馈。

迁移顺序：FSM → ANN/SNN → MB → CX。迁移期间由 feature flag 选择 legacy 或统一接口，保证逐步回归定位。

### 4.2 动作选择器

- 输入：LH-like 快速反射、MB approach/avoidance、CX 导航、状态机安全约束、敌人接触硬约束。
- 输出：受限 `turn_drive`、`speed_drive`、四通道沉积、攻击和任务切换。
- 优先级：立即碰撞/死亡风险可以硬覆盖；其余使用清晰的加权竞争与抑制，不再根据调用顺序隐式覆盖 `heading`。

### 4.3 验证协议

1. **零差异迁移**：关闭新选择器时指纹与 legacy 行为逐字节一致。
2. **单来源刺激**：分别只给 Trail、home、alarm、视觉、敌人，验证动作方向和饱和范围。
3. **冲突刺激**：Trail 与 alarm、home 与 food、CX 与 LH 相反；记录每个来源证据和最终命令。
4. **消融**：逐一清零一个来源，其他不变；比较路线、交付、攻击和状态预算。

**通过标准**：无 NaN；边界内动作；冲突规则可从 telemetry 重建；既有 `--validate` 核心基线不低于预设容差。

## 5. 阶段 2：感觉编码与早期感觉回路

### 5.1 化学感受：ORN→AL→PN

**最小模型**：

- 每个抽象化学场映射为多个 ORN-like 响应单元；
- 响应使用饱和函数、适应状态和可控观测噪声；
- glomerulus-like 层保留侧抑制；
- PN-like 输出分为通道特异（单小球）和混合（多小球）投射；
- 快速 LH-like 路径读取 PN，产生先天趋避偏置；MB 读取 PN 组合，而非直接读取世界场值。

**禁止项**：不声称 ORN/PN 数量、受体命名或通道映射等同于蚂蚁/果蝇真实解剖。

### 5.2 视觉与罗盘观测

新增 `CompassObservation`：

- `bearing`、`confidence`、`available`、可选 landmark evidence；
- 噪声、系统偏差、遮挡时段和重获参数；
- world `heading` 只能用于生成该观测及计算 benchmark 误差。

保留现有“可见食物”作为独立视觉目标特征；不把它混入罗盘真值。

### 5.3 验证协议

- **适应**：恒定化学浓度下 ORN 响应衰减；浓度阶跃后恢复；关闭适应得到稳定基线。
- **混合物**：同强度单通道与双通道输入，检查多小球 PN 是否编码组合而非只取最大值。
- **噪声**：递增观测噪声时，方向误差增大但模型不崩溃。
- **遮挡**：视觉罗盘遮挡下 CX 只能使用积分；恢复后可重新锚定。
- **配对消融**：`orn`、`al_inhibition`、`pn_multichannel`、`lh_reflex` 四类开关分别比较。

**输出**：每 seed 的感觉强度、方向误差、交付、路线效率、短桥流量；内部编码只作为辅助读出。

## 6. 阶段 3：CX 闭环导航

### 6.1 实现内容

1. 把当前 `cx_integrate` 的真实 heading 锚定改为 `CompassObservation`。
2. 从 ring bump + home vector 解码相对目标方向。
3. 增加 `CXActionEvidence`，经下降动作路径影响 `turn_drive` 与探索/返巢置信度。
4. 添加 `MotorFeedback.actual_turn`、`actual_distance` 与接触事件，使积分基于动作执行结果而不是假定命令完美执行。
5. `--brain cx` 作为 CX-only 对照；MB/SNN/FSM 可以通过开关使用 CX 导航证据。

### 6.2 必须测试的任务

| 任务 | 条件 | 主要读出 | 关键对照 |
|---|---|---|---|
| 开阔归巢 | 无地标、低噪声 | home-vector 角/距离误差、交付率 | 软件向量基线 |
| 罗盘遮挡 | 中途 `available=false` | 误差增长率、恢复时间 | 始终可见 |
| 罗盘偏置 | 固定 bearing bias | 系统路线偏差 | 零偏置 |
| 地标—罗盘冲突 | 两线索相反 | 转向权重、最终路线 | 单线索 |
| 动作扰动 | 实际转角/速度与命令不同 | 积分鲁棒性 | 无扰动 |

**通过标准**：各任务能产生方向明确但不被预先硬编码的差异；消融 `cx_motor_readout` 与 `cx_integrator` 的行为后果可区分；不得用真值角度绕过观测层。

## 7. 阶段 4：MB 价值学习、RPE 与资格迹

### 7.1 MBON 竞争

- 将现有输出重构为显式 `approach_value`、`avoidance_value`、`deposit_value`、`defense_value`；
- 在 MBON 层或动作选择器中实现竞争/互抑制；
- LH 的先天信号可提供先验，但不能永久绕过 MB 输出的动作影响。

### 7.2 RPE 与 eligibility trace

新增每蚂蚁状态：`value_estimate`、有限长度/衰减的 eligibility trace、正负 RPE。奖励计算发生在串行 flush 后，但 trace 在本 tick 感觉—动作阶段累积，以正确处理延迟后果。

建议更新：

\[
e_{t+1}=\lambda e_t + \phi_t
\]

\[
\delta_t=r_t+\gamma V(s_{t+1})-V(s_t)
\]

\[
\Delta w=\eta\,\delta_t\,e_t
\]

`r_t` 仍是抽象食物/伤害/能量后果；不得声称是果蝇具体 DAN 放电模型。

### 7.3 必须测试的学习任务

1. **acquisition**：线索 A 与食物后果配对；A 的 approach 增加。
2. **extinction**：移除食物后，A 的 learned approach 随试次下降。
3. **reversal**：A/B 后果对换；比较重学速度与初学速度。
4. **blocked control**：已预测的 A 后加入 B，B 的学习弱于未被阻断的 B。
5. **delay credit**：线索—行动—延迟奖励间隔递增；比较有无 eligibility trace。
6. **valence split**：相同感觉线索分别与奖励/伤害关联，approach 与 avoidance 输出方向相反。

每个任务必须包含：无 DA、无 STDP/RPE、打乱后果、随机 seed 配对对照。

## 8. 阶段 5：触角社会感觉与动作反馈

### 8.1 第一版社会接触协议

- 用空间哈希发现接触候选，但仅在串行阶段创建/确认事件；
- 支持最小事件：`contact`、`recruit_offer`、`recruit_accept`、`threat_signal`、`colony_identity_match`；
- 先使用离散的抽象身份/状态，不模拟真实 cuticular hydrocarbon 化学；
- 可选 leader/follower 状态，但不能让一个蚂蚁直接写入另一个蚂蚁的神经状态。

### 8.2 CPG/身体闭环最小扩展

- 将碰壁/接触、能量代价和实际速度输入 `MotorFeedback`；
- OA 调节速度时同时记录稳定性/碰撞/能量代价；
- 不做完整腿部动力学；保持当前 tripod 相位作为低维身体状态。

### 8.3 验证协议

| 比较 | 任务 | 行为读出 |
|---|---|---|
| 纯信息素 vs 纯接触 vs 两者 | 稀疏食源招募 | 首次发现时间、招募成功率、交付、接触次数 |
| identity 开/关 | 两群竞争 | 误招募、同群协同、攻击误触发 |
| OA 速度增益扫描 | maze/障碍环境 | 通行时间、碰撞、能量、存活 |
| contact 消融 | leader/follower 任务 | 跟随持续时间、路线效率、失败率 |

所有接触协议必须单独标识为抽象社会信号模型，不能外推为真实蚂蚁触角通信机制。

## 9. 阶段 6：集成评估与发布门槛

### 9.1 测试矩阵

| 维度 | 最小覆盖 |
|---|---|
| 脑模式 | FSM、ANN、SNN、MB、CX、集成脑 |
| 环境 | rich、scarce、predator、patchy、maze，以及导航/学习/接触专项场景 |
| 机制 | ORN、AL、PN、LH、CX sensor/motor、MB RPE、OA、CPG feedback、contact |
| 对照 | baseline、单模块配对消融、打乱输入/后果、软件真值数值基线 |
| 重复 | 默认至少 5 seeds；报告有效样本数、均值、样本标准差、逐 seed CSV |

### 9.2 非回归门槛

- `cargo fmt --check`、`cargo test`、`git diff --check` 通过；
- `--verify-determinism` 在新模块关闭及关键新场景下通过；
- 既有双桥 BFS 几何和真实门线流量测试通过；
- 新增协议保存原始每 seed 结果，不用汇总内部神经指标替代行为结果；
- 10k colony 性能不超过阶段 0 基线的预定义回退阈值；如回退，报告 profiler 结果和功能收益，不静默接受；
- 失败或负结果照样保留在结果中，禁止仅展示选中参数/种子。

### 9.3 决策门

每阶段评审时回答：

1. 新模块是否改变量化行为，且方向跨 seed 稳定？
2. 该变化能否被预先声明的消融反转或削弱？
3. 是否存在更简单的解释（如真值泄漏、参数未接线、读出定义错误、采样偏差）？
4. 代价是否超过科学收益？若是，冻结该模块并保留为实验分支。

只有四项都达到要求，才进入下一阶段或把模块纳入默认集成脑。

## 10. 建议的实现顺序和文件触点

| 工作包 | 优先文件 | 预期改动 |
|---|---|---|
| 阶段 0 | `src/genome.rs`, `src/ant/brain.rs`, `src/main.rs` | trait distance、脑模式标签、学习语义与测试 |
| 阶段 1 | `src/ant/mod.rs`, `src/ant/brain.rs`, `src/sim.rs` | 感觉/动作/反馈结构、串行事件接口 |
| 阶段 2 | `src/ant/sensors.rs`, `src/ant/brain.rs`, `src/genome.rs`, `src/world/` | ORN/AL/PN/LH、罗盘观测、配置与消融 |
| 阶段 3 | `src/ant/brain.rs`, `src/ant/mod.rs`, `src/main.rs` | CX→motor、导航 benchmark、误差 telemetry |
| 阶段 4 | `src/ant/brain.rs`, `src/ant/mod.rs`, `src/sim.rs`, `src/genome.rs` | MBON、RPE、trace、学习任务 |
| 阶段 5 | `src/sim.rs`, `src/world/spatial_hash.rs`, `src/ant/` | 接触事件、社会状态、身体反馈 |
| 阶段 6 | `src/main.rs`, `src/evolution.rs`, `src/viz/` | 协议汇总、CSV、可视化、性能/确定性 |

## 11. 明确不做的捷径

- 不因论文给出完整果蝇连接组，就把大量细胞类型名称复制为本项目结构；
- 不因神经元数量级接近，就主张果蝇—蚂蚁回路对应；
- 不用世界真值为 CX/MB 造高分；
- 不以单 seed、单环境、单冠军证明机制；
- 不将程序开关叫作脑区、基因或药理操作；
- 不为维持旧排行榜而跳过新模块的负结果或消融。

## 12. 预期文档与结果产物

实现阶段完成时更新（或按用户明确请求创建）以下已有/目标产物：

- `docs/neural_architecture_spec.md`：规格与范围变更；
- 本计划：阶段状态、已完成项与偏离说明；
- `results/`：每项专项协议的逐 seed CSV 与汇总；
- `REPORT.md`：只加入通过决策门的模型内结论；
- `docs/research_scope.md`：仅在模块已经实现且验证后更新可报告结论与边界。
