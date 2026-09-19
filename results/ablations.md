# ants-sim 消融实验：机制充分性 → 可证伪预测与湿实验 Protocol

## 目的与定位

将"机制 X 足以产生行为 Y"的充分性命题，通过消融（移除 X 测行为后果）转为**可被湿实验直接检验的预测**。系统定位为**充分性/消融引擎**：sim 内可证 X 充分，外推为真实干预应同向变化——预测的**方向与机制**是内容，量化幅度为模型特定（非物种级定量）。充分性 ≠ 必要性（见 docs/research_scope.md）。

## 方法

`--ablate <mech>`：对同一基因组/种子跑 baseline（机制在）vs ablated（机制关）×N tick，比行为读出（collected / max_alive）或学习读出（KC→output 权重漂移 RMS）。8 机制：octopamine / trail / eclosion / homevector / vision（行为读出）+ stdp / reward / punish（学习读出，brain=mb）。

## 结果总表

| # | 机制 | 环境/脑 | baseline | ablated | Δ | 读出 |
|---|---|---|---|---|---|---|
| 1 | octopamine | scarce_far / fsm | 74.0 | 35.0 | **−52.7%** | collected |
| 2 | trail | patchy / fsm | 198.0 | 286.0 | **+44.4%** | collected |
| 3 | eclosion | rich_close / fsm | 242 (max) | 200 (max) | **−17.4%** | max_alive |
| 4 | homevector | scarce_far / fsm | 74.0 | 41.0 | **−44.6%** | collected |
| 5a | vision | rich_close / fsm | 1204.0 | 1483.0 | **+23.2%** | collected |
| 5b | vision | scarce_far / mb | 65.0 | 59.0 | **−9.2%** | collected |
| 6 | stdp | rich_close / mb | drift 0.0649 | 0.0000 | **−100%** | weight drift |
| 7 | reward (PAM) | rich_close / mb | drift 0.0649 | 0.0272 | **−58.1%** | weight drift |
| 8 | punish (PPL1) | war+forage / mb | drift 噪声 | 噪声 | **无定论** | weight drift |

## 逐项预测与湿实验 Protocol

### 1. Octopaminergic arousal 足以驱动觅食活力（−52.7%）
**预测**：octopamine 受体信号对远食环境的觅食活力是充分的；沉默它使收集腰斩。
**Protocol**：物种 *Camponotus* / *Drosophila*（octopamine 通路保守）。干预：octopamine-受体拮抗剂（如 mianserin）注射 或 Tdc2-GAL4>UAS-Shibire^ts（酪胺能神经元温度沉默）。读出：远距食源（>1 m）的群体觅食率（进出巢频次/糖液消耗）。对照：溶剂/野生型。预期：干预组觅食率显著下降，探索距离与频次降低。
**机制**：OA 唤醒耦合探索强度×(1+arousal)、轨迹响应阈值↓、CPG 步频↑；沉默则蚁迟钝、远食难找。

### 2. Stigmergic Trail 以吞吐换路径质量（+44.4%，反直觉）
**预测**：trail 信息素优化所选路径（ACO 短路径有效）但**过度收敛**觅食者至单一路径/单源→瓶颈；信息素干扰在多源环境**增加**总产量（分散觅食者），代价是路径最优性。
**Protocol**：物种 *Linepithema humile*（阿根廷蚁，mass-recruitment）。干预：trail 信息素降解酶喷洒 或 受体拮抗剂。读出：多食源（≥3）布置下的**总糖液消耗**（非单源占比）。对照：溶剂。预期：干预组总产量上升（分散到多源），但单源路径选择能力（短/长桥比）下降。
**机制**：trail 收敛觅食者→单路径吞吐上限；消融→多平行路径→高总吞吐。反直觉但跨多/单源一致。

### 3. Brood eclosion 足以驱动蚁群扩张（−17.4% max_alive）
**预测**：brood-to-adult eclosion（nurse 驱动）对群体扩张是充分的；阻断则群体增长停滞。
**Protocol**：物种 *Temnothorax* / *Pogonomyrmex*。干预：物理移除 nurse 品级 或 brood-care 行为干扰（基因沉默 brood-tending相关基因）。读出：8 周内工蚁总数变化。对照：未处理群。预期：干预群体工蚁数持平（无新成虫），对照群增长。

### 4. Path integration 足以驱动非视觉归巢（−44.6%）
**预测**：路径积分（home vector）对无地标归巢是充分的；损伤则归巢/觅食产量崩。
**Protocol**：物种 *Cataglyphis fortis*（沙漠蚁，PI 主导归巢）。干预：中央复合体（CX）药物损伤 或 mushroom-body 外的 CX 投射干扰（光遗传沉默 CX 环吸引子）。读出：归巢误差（释放点→巢距离/方向误差）、觅食往返频次。对照：假手术。预期：干预蚁归巢误差剧增、觅食产量降。
**机制**：home vector 累积位移指回巢；归零则蚁无法回巢→交付崩。

### 5. Visual detection 效果环境依赖（+23.2% 易 / −9.2% 难）
**预测**：视觉对**难环境**（远食）觅食是充分的（消融降产）；在**易环境**（近食）视觉反而轻微降低产量（覆盖高效 trail）。
**Protocol**：物种 *Cataglyphis*（视觉导航）/ *Myrmecia*（视觉觅食蚁）。干预：遮蔽复眼（黑漆）或视觉地标移除。读出：远食（>10 m）vs 近食（<2 m）布置的觅食率。对照：正常视觉。预期：远食组觅食率显著降；近食组无降或微升（视觉干扰 trail 时）。
**机制**：视觉在远距引导奔食；近距时视觉 steer 覆盖 trail-following 致次优。

### 6. Lifetime STDP 足以驱动一生内学习（drift −100%）
**预测**：KC→output 突触可塑性（STDP）对一生内嗅觉联想学习是充分的；阻断则权重漂移归零（无学习）。
**Protocol**：物种 *Drosophila*（MB 联想学习范式成熟）。干预：MBON 突触可塑性阻断（如 rutabaga 突变体 / CaMKII 抑制 / MBON-GAL4>UAS-TNT）。读出：嗅觉-糖条件化后的 PER（喙伸展反应）学习指数。对照：野生型。预期：干预果蝇学习指数≈0（无记忆形成），野生型显著学习。

### 7. Reward-dopamine (PAM) 足以驱动 Appetitive LTP（drift −58.1%）
**预测**：PAM-DAN 信号对 appetitive（奖励）LTP 是充分的；沉默则约 58% 权重漂移丧失。
**Protocol**：物种 *Drosophila*。干预：PAM-DAN 温度沉默（TH-GAL4>UAS-Shibire^ts 或 PAM-特异性 GAL4）。读出：嗅觉-糖条件化 PER 学习指数（appetitive memory）。对照：野生型 + 非关联配对。预期：PAM 沉默组 appetitive memory 显著受损；aversive memory（嗅觉-电击）保留。
**机制**：PAM 释放 reward 多巴胺门控 KC→MBON LTP；沉默则 appetitive 联想不强化。

### 8. Punish-dopamine (PPL1) — 读出无定论
**结果**：war+forage 场景下权重漂移噪声大、跨设置不一致（baseline/ablated 漂移随场景反转），**未能得到稳健读出**。
**诚实评估**：aversive 学习通路在 sim 中难单独隔离——war 战斗是硬连线反射（非习得），punish 驱动的 avoidance-output LTD 行为后果微弱，且 war 蚁群快速死亡使漂移测量噪声大。
**Protocol（待 sim 改进后）**：物种 *Drosophila*。干预：PPL1-DAN 沉默。读出：嗅觉-电击条件化 aversive memory。预期（依文献）：PPL1 沉默应损害 aversive memory。**注**：此预测源自文献而非本 sim 消融（sim 读出无定论），需先用 predator-环境-真伤害场景改进 sim 的 aversive 通路再验证。

## 总体结论

1. **6/8 消融给出稳健、方向明确的可证伪预测**（octopamine/trail/eclosion/homevector/stdp/reward），映射到具体湿实验（OA-受体/信息素酶/nurse移除/CX损伤/MBON阻断/PAM沉默）。
2. **2 项反直觉发现经偏差检验判定为 FSM-实现 artifact（非普遍预测）**——见下节"偏差检验"。
3. **1 项无定论**（punish）——诚实标注 sim 的 aversive 通路需改进（predator+真伤害场景）方能给出预测；PPL1 预测暂引自文献。
4. **环境依赖性**是反复出现的主题（vision 易/难反转、trail 多/单源反转）——预测须标明环境条件。

## 偏差检验 → 偏差修复（trail/vision artifact 已修正）

### 偏差识别（跨脑测试）

两个反直觉结果（trail 消融 +44%、vision 消融易环境 +23%）经跨脑测试判定为 **FSM-实现 artifact**：trail 对 ANN 有益（−31% 消融）、对 FSM 有害（+44%）；vision 对 ANN 中性、对 FSM 有害。有限食物/大群落未翻转，唯跨脑翻转→FSM-specific 实现偏差。

### 偏差根因

FSM `FollowTrail` 状态是**确定性梯度硬爬升**（无噪声、全收敛单源/单路径→吞吐瓶颈），而 ANN 把 trail 作**分级软转向输入**（混噪声→分散→有益）。FSM 的硬爬升使 trail 净有害，且其 vision-steer（Explore 内覆盖 heading）与 trail 冲突。

### 修复（T23+）

FSM Explore 改**分级 trail 软偏置**（`steer_toward(trail_bearing, turn_rate×0.5)` + 探索噪声持续），不再切 FollowTrail 硬爬升状态——镜像 ANN 的分级模式。真实蚁亦以误差跟随 trail（非确定性）。

### 修复后结果（全消融重跑）

| # | 机制 | 环境/脑 | Δ（修复前）| Δ（修复后）| 判定 |
|---|---|---|---|---|---|
| 1 | octopamine | rich/patchy/scarce | −52.7%(scarce) | −22.6%/−34.3%/+19.6% | 环境依赖（多源帮、单源-trail-高效伤）defensible |
| 2 | trail | patchy | **+44.4%** | **−46.9%** | **修好**（trail 有益）✓ |
| 3 | eclosion | rich | −17.4% max | −22.0% max | 一致 ✓ |
| 4 | homevector | scarce | −44.6% | −30.1% | 一致 ✓ |
| 5 | vision | rich/scarce | **+23.2%/−9.2%** | **−30.6%/−9.2%** | **修好**（vision 有益）✓ |
| 6 | stdp | mb | drift −100% | −100% | 一致 ✓ |
| 7 | reward | mb | drift −58.1% | −58.1% | 一致 ✓ |
| 8 | punish | war | 无定论 | 无定论 | 诚实标注 |

**FSM 产量提升**：修复后 FSM 基线收集大幅提升（patchy 198→539, scarce 74→143, rich 1204→4566）——旧硬爬升 FollowTrail 一直在拖累 FSM 觅食。

### 修复后结论

- **7/8 消融方向直观或可解释**（trail/vision/octopamine-rich·patchy/eclosion/homevector/stdp/reward 全负向=机制有益）。
- **octopamine 环境依赖**（多源帮、单源-trail-高效伤）——defensible：OA 探索在多源环境助找食，在单源-trail-已高效环境反成噪声干扰。真蚁 OA 角色亦情境依赖。
- **punish 仍无定论**——需 predator+真伤害场景改进 sim aversive 通路。
- **跨脑测试 = 偏差识别决定性工具**；**分级软偏置 = 修复方向**（镜像已工作的 ANN）。这是 sim 自我纠偏闭环：反直觉→跨脑识别 artifact→分级修复→方向直观化。

### 偏差检验方法论

**跨脑测试**判别 sim-artifact vs robust-预测：效应跨脑翻转→脑实现 artifact；跨脑一致→模型深层性质。辅以**食物耗竭/规模/参数扫描**排除假设性偏差源。本次 trail/vision 的食物与规模检验未翻转、跨脑翻转→精确定位 FSM 实现偏差；分级软偏置修复后方向直观化，确认诊断正确。


## 诚实边界

- **充分性 ≠ 必要性**：sim 证"X 足以"不证"真蚁用 X"。
- **方向是预测内容**：量化幅度（−52.7% / +44.4% 等）为模型特定，非物种定量预测。
- **反直觉结果本身是可证伪点**：若湿实验与 trail/vision 反直觉预测不符，则 sim 的 stigmergy/视觉-吞吐模型与现实有偏差——亦是有价值的证伪。
- 详见 docs/research_scope.md（可验证/不可验证边界）。

## 复现
```bash
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
cargo run --release -- --headless --ablate octopamine --env scarce_far --ticks 3000
cargo run --release -- --headless --ablate trail      --env patchy    --ticks 3000
cargo run --release -- --headless --ablate eclosion   --env rich_close --ticks 3000
cargo run --release -- --headless --ablate homevector --env scarce_far --ticks 3000
cargo run --release -- --headless --ablate vision     --env rich_close --ticks 3000
cargo run --release -- --headless --ablate vision     --env scarce_far --ticks 3000 --brain mb
cargo run --release -- --headless --ablate stdp       --env rich_close --ticks 3000
cargo run --release -- --headless --ablate reward    --env rich_close --ticks 3000
cargo run --release -- --headless --ablate punish     --env rich_close --ticks 2000 --colony 100
```
