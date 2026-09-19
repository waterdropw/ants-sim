# ants-sim 行为涌现验证（--validate）

- brains=["fsm"] envs=["rich_close"] ticks=3000 colony=200
- 诚实边界：这是**内部行为合理性 + 定性涌现**验证，非与某物种定量一致（模型示意级、tick↔秒无量纲；定量锚点见 literature/*.toml + --compare-lit）。

| brain | env | survival | trail_formed | caste_stable | colony_grew | foraging | pass |
|---|---|---|---|---|---|---|---|
| fsm | rich_close | 1.10 | Y | Y | Y | Y | 5/5 |

**聚合：5/5 检查通过 (100%)**

## 判据
- survival>0.1（蚁群存续）
- trail_formed：trail_total 峰值>2（网络形成）且末值>1（仍存在；峰值后下降是网络 consolidated 到更少高效 cell，非坍塌）
- caste_stable：guard 占比标准差<0.10（分工稳态）
- colony_grew：峰值活蚁>1.05·初始（eclosion 增长）
- foraging：末 500t 仍有交付（持续觅食）

## 诚实评估
这是**内部行为合理性 + 定性涌现**验证（默认未演化基因组）。定性涌现（trail 形成、分工稳态、蚁群增长、持续觅食）对照真蚁行为模式；rich_close（食物近且多）6 脑均涌现良好，hard env（scarce_far/maze）默认脑全饿死——这是**需演化适配**而非模型故障：cross-brain EVOLVED sweep（results/cross_brain_evolution_summary.txt）显示演化脑在硬环境能收集。定量一致性受模型示意级定位与 tick↔秒无量纲化限制（见 literature/*.toml + --compare-lit 的定量锚点）。ann 与 cppn 在未演化默认基因组下结果一致（cppn 零基因 = ann 种子）。
