# ants-sim 研究范围：模型内证据、外部假设与边界

本文界定 ants-sim 的证据范围。它是一个**抽象机制比较、反事实扰动与假设生成试验台**，不是特定物种、神经回路或生态系统的保真模型。

## 系统定位

模型使用抽象物种、无量纲 tick、工程尺度神经模块和合成环境；基因组字段是控制参数，不是自然等位基因。因此它能可靠回答的是：

1. **模型内机制依赖**：在固定实现、基因组、环境、读出和随机协议下，启用/关闭模块 X 是否改变读出 Y？
2. **模型内反事实比较**：不同控制器、环境或评估窗口下，结果的相对脆弱性和稳健性如何？
3. **可检验假设生成**：把模型模式转写为带物种、干预、对照和读出的方向性湿实验假设。

消融不是充分性检验：关闭 X 后 Y 变化，只说明 X 是该实现中的影响因素。构造性充分性还需在缺失背景中加入 X 后重建 Y；真实必要性还需要物种内、机制特异的因果干预。

## 当前可报告的模型内读出

| 模块/协议 | 模型内可报告结论 | 禁止的直接等同 |
|---|---|---|
| `--bridge` | 用两条门线的实际进/返程过线数计算短路返程流量占比；可比较启用/关闭 Trail 后的模型交通模式。 | Trail 峰值、浓度比或其变换值不是蚂蚁交通比例。 |
| `--caste` | 对 `Explore`、`FollowTrail`、`CarryReturn`、`Alarm`、`Defend`、`Nurse` 的实时状态做时间预算统计。 | 状态预算不是形态品级、固定反应阈值或真实守卫比例。 |
| `--ablate` | 使用配对 seed 的 baseline/关闭模块比较，报告产出、存活、人口过程和内部权重漂移。 | 程序开关不是一一对应的药理、基因、脑区或神经元操作。 |
| `--bench-cx` | 在已知朝向、无感觉噪声的数值步行中量化抽象积分器误差。 | 不是 Cataglyphis 的 CX 病灶实验或真实路径积分误差拟合。 |
| `--compare-lit` | 将定义不同的代表性文献量级作为上下文锚点，并显式显示不匹配。 | 区间命中不是物种级验证、统计拟合或模型选择证据。 |

## 模型内结果如何转为外部假设

### A. 信息素与双桥

**模型内证据**：返巢蚂蚁的 Trail 沉积与衰减可在当前双桥几何中改变两臂的门线通行流量。

**可检验假设**：在依赖群体招募的物种、明确双路径和控制食源条件下，降低 trail 信号可用性或持久性可能降低短路径偏置、降低其收敛速度，或增加重复间变异；并不预言选择必然完全消失。

**边界**：模型没有真实信息素分子、受体、挥发动力学或物种特异的觅食规则。

### B. 路径积分与 CX-like 模块

**模型内证据**：清空内部 home-vector 状态会改变当前控制器的归巢和交付读出。

**可检验假设**：在地标稀少、但仍可利用太阳/天空罗盘的导航任务中，选择性扰动方向罗盘、里程计或相关整合通路应增加归巢方向或距离误差。

**边界**：`homevector` 是抽象状态清零；不等同于中央复合体的局灶操纵。沙漠蚁“地标稀少”也不等于无视觉线索。

### C. 多巴胺调制的可塑性

**模型内证据**：reward/punish 标量可门控 MB/SNN 权重更新；`weight_drift` 是内部可塑性状态指标。直接 ANN 是固定的演化控制器，不含生命周期内权重更新。

**可检验假设**：在已定义的 CS–US 配对任务、特定 DAN 亚群与 MB 分室中，抑制相应通路可能选择性影响相应价性的行为记忆。

**边界**：当前觅食 pickup/delivery 或伤害不是嗅觉条件化；权重漂移不是关联记忆的行为读出。PAM/PPL1 只能被称为功能类比，不能当作模型中的真实细胞类型。

### D. OA-like 唤醒、育幼与视觉

- OA-like 变量是模型内同时影响探索、线索响应、攻击和步速的耦合状态；其消融不能等同于某个 OA 受体、递质合成酶或神经元群的操作。
- `eclosion` 消融只阻止模型的 brood→adult 转换；它不等同于移除 nurse。真实净增长还取决于产卵、brood 存活、发育、营养、死亡和任务重分配。
- `vision` 消融只移除前方近距离食物探测；它不等同于遮眼、去地标、天空偏振罗盘干预或完整视觉导航。

### E. 演化、架构与竞争

- 迁移矩阵可显示此参数化下的 G×E 和潜在特化—泛化权衡；性能下降不是所有环境组合中必然存在的生物学 trade-off。
- 控制器排名是**算法归纳偏置**比较。它不支持“CX 比 MLP 跨物种更鲁棒”或真实神经架构优劣的结论。
- 两群胜负翻转仅表明竞争对策略和初始条件敏感；没有三策略非传递支付矩阵、频率依赖与稳定循环，就不能称为 cyclic dominance。
- `--novelty` 是人工搜索算子。它可与 fitness-only 对照比较模型行为描述符的多样性，不能解释为自然种群中的“新奇度选择”。

## 不可验证 / 系统边界

1. **物种级定量一致**：tick 不映射秒、格点不映射米，且文献读出与模型读出常不同构。
2. **真实必要性、充分性与回路保真**：MB（12 瞌小球/64 KC）和 CX（16 单元）是工程尺度抽象，不是连接组或病灶模型。
3. **田间生态适合度**：抽象敌人不代表经校准的捕食、天气、竞争、寄生或季节生态。
4. **分子、遗传、发育机制**：没有基因调控、表观遗传或真实发育；不可外推到 `foraging/PKG` 等位基因效应。
5. **学习动力学拟合**：模型的觅食 bout、内部权重变化与果蝇 odor–shock/odor–sugar trial 不同构。
6. **系统发育、物种形成和宏观演化**：短期 GA 不能代表长期种群历史。
7. **真实信息素化学、个体识别与社会记忆**：四个抽象通道与可互换个体不包含这些机制。
8. **规模外推**：许多物种成熟群体远大于当前常用 colony；规模依赖涌现未经验证。

## 参考文献与使用方式

以下文献提供外部背景或可检验假设的依据；它们不把当前模型转化为对应物种的定量拟合。

1. Goss, S. et al. (1989). *Self-organized shortcuts in the Argentine ant*. **Naturwissenschaften** 76, 579–581. DOI: [10.1007/BF00462870](https://doi.org/10.1007/BF00462870).
2. Müller, M. & Wehner, R. (1988). *Path integration in desert ants, Cataglyphis fortis*. **PNAS** 85, 5287–5290. DOI: [10.1073/pnas.85.14.5287](https://doi.org/10.1073/pnas.85.14.5287).
3. Wehner, R. (2003). *Desert ant navigation: how miniature brains solve complex tasks*. **J. Comp. Physiol. A** 189, 579–588. DOI: [10.1007/s00359-003-0431-1](https://doi.org/10.1007/s00359-003-0431-1).
4. Seelig, J. D. & Jayaraman, V. (2015). *Neural dynamics for landmark orientation and angular path integration*. **Nature** 521, 186–191. DOI: [10.1038/nature14446](https://doi.org/10.1038/nature14446).
5. Aso, Y. et al. (2014). *The neuronal architecture of the mushroom body provides a logic for associative learning*. **eLife** 3:e04577. DOI: [10.7554/eLife.04577](https://doi.org/10.7554/eLife.04577).
6. Liu, C. et al. (2012). *A subset of dopamine neurons signals reward for odour memory in Drosophila*. **Nature** 488, 512–516. DOI: [10.1038/nature11304](https://doi.org/10.1038/nature11304).
7. Aso, Y. et al. (2010). *Specific dopaminergic neurons for the formation of labile aversive memory*. **Current Biology** 20, 1445–1451. DOI: [10.1016/j.cub.2010.06.048](https://doi.org/10.1016/j.cub.2010.06.048).
8. Burke, C. J. et al. (2012). *Layered reward signalling through octopamine and dopamine in Drosophila*. **Nature** 492, 433–437. DOI: [10.1038/nature11614](https://doi.org/10.1038/nature11614).
9. Gordon, D. M. (1996). *The organization of work in social insect colonies*. **Nature** 380, 121–124. DOI: [10.1038/380121a0](https://doi.org/10.1038/380121a0).
10. Bonabeau, E., Theraulaz, G. & Deneubourg, J.-L. (1996). *Quantitative study of the fixed threshold model for the regulation of division of labour in insect societies*. **Proc. R. Soc. B** 263, 1565–1569. DOI: [10.1098/rspb.1996.0229](https://doi.org/10.1098/rspb.1996.0229).
11. Kawecki, T. J. & Ebert, D. (2004). *Conceptual issues in local adaptation*. **Ecology Letters** 7, 1225–1241. DOI: [10.1111/j.1461-0248.2004.00684.x](https://doi.org/10.1111/j.1461-0248.2004.00684.x).
12. Sinervo, B. & Lively, C. M. (1996). *The rock–paper–scissors game and the evolution of alternative male strategies*. **Nature** 380, 240–243. DOI: [10.1038/380240a0](https://doi.org/10.1038/380240a0).
13. Lehman, J. & Stanley, K. O. (2011). *Abandoning Objectives: Evolution Through the Search for Novelty Alone*. **Evolutionary Computation** 19, 189–223. DOI: [10.1162/EVCO_a_00025](https://doi.org/10.1162/EVCO_a_00025).

## 元结论

应把“sim 内已证”写为“在本模型、此参数和此读出下观察到”；应把“真实干预应当”写为“可检验的、带条件的外部假设”。模型内百分比只描述该实现，不构成物种级效应量预测。
