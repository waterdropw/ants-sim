# ants-sim 行为涌现验证（--validate）

- brains=["fsm"] envs=["rich_close"] ticks=3000 colony=400
- 诚实边界：这是**内部行为合理性 + 定性涌现**验证，非与某物种定量一致（模型示意级、tick↔秒无量纲；定量锚点见 literature/*.toml + --compare-lit）。

| brain | env | survival | trail_formed | task_budget_stable | colony_grew | foraging | pass |
|---|---|---|---|---|---|---|---|
| fsm | rich_close | 1.15 | Y | Y | Y | Y | 5/5 |

**聚合：5/5 检查通过 (100%)**

## 判据
- survival>0.1（蚁群存续）
- trail_formed：trail_total 峰值>2（网络形成）且末值>1（仍存在；峰值后下降可能是网络压缩，而非必然坍塌）
- task_budget_stable：实时行为状态（觅食/防御/育幼）三个时间序列的标准差均<0.10；这不是形态品级或物种级分工比例
- colony_grew：峰值活蚁>1.05·初始（模型的 brood→adult 人口过程）
- foraging：末 500t 仍有交付（持续觅食）

## 诚实评估
这是**内部行为合理性 + 定性涌现**检查，非物种级定量验证。行为状态、抽象信息素、brood→adult 转换与真实蚂蚁的任务、化学或发育过程并非一一对应；定量锚点只能作为上下文，不能据区间命中宣称验证（见 `literature/*.toml`、`--compare-lit` 与 `docs/research_scope.md`）。
