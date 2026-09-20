# ants-sim 反事实敏感性实验

## 定位

`--ablate <mechanism>` 是一个**模型内、配对随机种子的反事实敏感性协议**。每个 replicate 对同一 seed 分别运行 baseline 与关闭一个实现开关的条件，并报告均值、样本标准差和配对百分比变化。

它能回答“在当前抽象实现、参数、环境、读出下，关闭此代码路径会怎样改变结果”，不能回答“真实动物是否依赖此机制”“此机制是否充分/必要”“某个程序开关等同于某项湿实验干预”。旧的单 seed 百分比和“充分性”表述已废弃，不能与新协议的数字直接比较。

## 运行协议

```bash
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"

# 推荐至少 5 个配对 replicate；--out 保存每 seed 的原始数据。
cargo run --release -- --headless --ablate octopamine --env scarce_far --ticks 3000 --n-seeds 5 --out results/ablate_octopamine.csv
cargo run --release -- --headless --ablate trail --env patchy --ticks 3000 --n-seeds 5 --out results/ablate_trail.csv
cargo run --release -- --headless --ablate eclosion --env rich_close --ticks 3000 --n-seeds 5 --out results/ablate_eclosion.csv
cargo run --release -- --headless --ablate homevector --env scarce_far --ticks 3000 --n-seeds 5 --out results/ablate_homevector.csv
cargo run --release -- --headless --ablate vision --env scarce_far --brain mb --ticks 3000 --n-seeds 5 --out results/ablate_vision.csv
cargo run --release -- --headless --ablate stdp --env rich_close --ticks 3000 --n-seeds 5 --out results/ablate_stdp.csv
cargo run --release -- --headless --ablate reward --env rich_close --ticks 3000 --n-seeds 5 --out results/ablate_reward.csv
cargo run --release -- --headless --ablate punish --env rich_close --ticks 2000 --colony 100 --n-seeds 5 --out results/ablate_punish.csv
```

`--n-seeds 1` 保留用于快速调试；其标准差显示为 `NA`，不应用于稳健性结论。每个输出 CSV 都是 tidy 格式：

```text
seed,condition,collected,max_alive,final_alive,weight_drift
```

`collected`、`max_alive`、`final_alive` 为模型行为/人口读出；`weight_drift` 是相对出生时 MB 权重的 RMS 内部状态变化，并不是行为记忆测验。

## 开关的准确含义

| `mechanism` | 程序中关闭的内容 | 不等同于 |
|---|---|---|
| `octopamine` | 每 tick 将 OA-like 标量清零 | 特定受体、Tdc2、单一神经元群的操作 |
| `trail` | 跳过 `Channel::Trail` 的沉积 | 信息素分子降解、受体阻断或完整招募系统的移除 |
| `eclosion` | 阻止模型 brood→adult 转换 | 移除 nurse 或现实中全部育幼过程 |
| `homevector` | 将软件/CX home-vector 清零 | CX 局灶病灶、步长计或罗盘的单独操纵 |
| `vision` | 删除前方近距离食物探测 | 致盲、地标移除、偏振光罗盘干预 |
| `stdp` | 跳过模型的 SNN/MB 权重更新 | 任一真实突触可塑性通路的完全阻断 |
| `reward` | 跳过 delivery/pickup 触发的 reward 标量 | PAM 亚群沉默 |
| `punish` | 跳过战争损伤触发的 punish 标量 | PPL1 亚群沉默 |

`punish` 使用双 colony 战争以产生模型伤害事件；它和普通觅食协议不同，应单独解释，也可能因死亡和硬连线战斗规则而高方差。

## 报告规则

1. 先检查每 seed 的原始输出、配对方向和样本数；不要只引用一个汇总百分比。
2. 报告为“模型内敏感性”，例如：`关闭 Trail 后，patchy 条件下 collected 的配对变化为 mean ± SD`。
3. 当 baseline 为零时，该 replicate 不参与该指标的百分比计算；若某项没有有效配对，终端汇总显示 `NA (n=0)`，而非制造无穷值。
4. 若跨 seed 的方向不一致或标准差大，应报告 **inconclusive / 环境依赖**，而非输出确定性的机制叙事。
5. 学习相关开关除 `weight_drift` 外，应在未来加入与训练/测试阶段分离的行为记忆任务，才可讨论“记忆表现”。

## 外部实验如何使用这些结果

模型结果只能帮助提出条件化的假设。例如，若在明确的远食环境中关闭 OA-like 标量稳定降低模型产出，则一个后续研究可预注册：在一个选定物种、食源距离、药理/遗传操作、运动控制和觅食读出下，检验候选 OA 通路是否同向改变觅食。即使结果同向，也不能反证或证明模型结构；反向或无效结果则要求修订该外推假设。

针对真实双桥、路径积分、MB/DAN、任务分配和 OA 的文献依据、定义差异及不可外推项见 `docs/research_scope.md` 与 `literature/*.toml`。
