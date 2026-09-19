# ants-sim 隔夜进度日志

最新一条在底部。每行：时间 | 任务 | 改动摘要 | 验证 | build。

baseline (今天): M1–M4 代码完成，egui 0.36 跑通；M3 双源选择已用 headless 遥测验证
(seed=42: 近/远 visits=1445/252, trail ratio≈9.5)。

2026-09-12T23:39:35 | A1 | config.rs + Config{TOML} + --config/--print-config; 生成 config/default.toml; App::from_config; 巢位从config | headless --config 往返跑通(500t smoke) | build OK
2026-09-12T23:43:20 | A2 | build_sim/fingerprint + --verify-determinism 双跑比对 | 1000t: DETERMINISM PASS (col=286 visits=[197,89] trail_sum=76.625749 一致) | build OK
2026-09-12T23:47:20 | A3 | --csv/--csv-every: 写 tick,collected,visits_i,trail_i,def_frac,forage_frac | 2000t two: 10行CSV, near/far visits 378/99 ratio3.83 | build OK
2026-09-12T23:53:02 | B1 | environment.rs Environment+EnvMetrics+5预设+Simulator::apply_environment+--env | 5 env headless 500t 各异: rich259/scarce34/predator0(patchy94/maze19; predator=0待调(TODO) | build OK
2026-09-12T23:59:58 | B2 | CSV 加 env/env_foods/env_dist/env_enemies/env_obs 五列(每行) | maze行:1/98/0/1048; predator行:2/60/3/0 def_frac→1.0印证过锁 | build OK
2026-09-13T00:06:21 | C1 | Genome::mutate(高斯)+crossover(BLX)+random+in_range; 22字段值域表; --test-genome | GENOME-OPS PASS n=3000 bad=0; 确定性未破 | build OK
2026-09-13T00:11:23 | C2 | set_colony_diverse(每蚁 base.mutate); --diverse; --inspect-diversity 同质vs异质统计 | DIVERSITY PASS: homo fs=3.000; div fs[1.49,4.73] explore[0.01,0.25] agg[0.22,0.90] | build OK
2026-09-13T00:15:16 | C3 | evolution.rs Fitness{collected,per_source,mean_def_frac,trail_total,score}; evaluate(); --fitness | rich482/scarce55/predator0→score89(防御项) 跨环境信号分明 | build OK
2026-09-13T00:19:35 | C4 | evolution::run GA(锦标赛+BLX+精英); --evolve --gens/--pop; 存 evolved_*.toml+history.csv | rich_close pop8 gens5: best 209→454(2.2×) mean 97.6→352.4(3.6×) | build OK
2026-09-13T00:24:53 | C5 | Ant.total_delivered; set_colony_from_pool; run_multilevel(个体级选择); --evolve-ml | rich_close pool12 gens5: colony_score 116→300(2.6×); 存 evolved_ml_*.toml | build OK
2026-09-13T00:30:21 | D1 | --evolve-all: 5预设各跑演化, --evolve-all; champions.csv+evolved_*.toml | 跨环境冠军发散: rich fs3.06/exp0.01; scarce exp0.183; predator aggr0.751/def0.98; patchy/maze exp高 | build OK
2026-09-13T00:34:33 | D2 | --zoo: 各环境冠军在原生环境跑 vs 默认; zoo.csv | champion全面碾压default: rich 284→1670(5.9×), patchy 87→424(4.9×), maze 23→87(3.8×), scarce 34→111 | build OK
2026-09-13T00:38:02 | D3 | --transfer 迁移矩阵(5冠军×5环境); transfer.csv | 特化: rich本土1297→scarce77(远食崩); predator防御特化觅食环境全垮(97/20/38/12); patchy/maze压力相近适应相近 | build OK
2026-09-13T00:43:43 | E1 | GUI加env ComboBox+Apply env+Load evolved champion+基因组显示; env_name字段 | 编译通过+启动无panic; 视觉确认待明天 | build OK
2026-09-13T00:48:11 | E2 | scenario_two_bridge + --bridge遥测(上下路径浓度比) | 6000t: BRIDGE PASS top0.0034/bottom0.0000, collected171, 避障+梯度上升路由可用 | build OK
2026-09-13T00:52:03 | E3 | --report: 汇总 champions/zoo/transfer CSV 生成 REPORT.md(冠军表+zoo+迁移矩阵+解读+复现命令) | REPORT.md 3036字节, 表格正确 | build OK
2026-09-13T00:57:23 | E4 | README.md + genome单元测试(5) + release build + GUI冒烟 | cargo test 5/5 PASS; release build OK; GUI无panic; 确定性PASS | build OK

2026-09-13T08:24:45 | ==== Tier 1 启动：做深做实 (robust fitness/energy死亡/predator调平/对称双桥/10k基准/大跑) ====
2026-09-13T08:26:55 | T1.1 | evaluate_multi(多种子均值); --n-seeds; --fitness显示per-seed+range; --evolve/--evolve-all接入 | rich_close 5seed scores[196,215,164,240,218] spread76 mean206.6 → 噪声大, 多种子必要 | build OK
2026-09-13T08:35:29 | T1.2 | Ant.dead + 能量经济学(常量) + retain移除死亡 + Fitness.survival + score含0.2·survival·colony | rich surv0.975/scarce0.965/maze0.945 死亡分化 | build OK
2026-09-13T08:42:14 | T1.3 | predator: 敌人2+外移28格+血量10; 去告警中继(只接敌沉积); 觅食者阈值2× | predator collected169/def0.015/surv0.91(原1/0.976/0.045); rich无回归422 | build OK
2026-09-13T08:50:02 | T1.4 | 对称双桥(墙1对称缺口+墙2逼底路绕远); max_in_rect; 采样墙西侧 | 6000t: PASS both_used=true, top0.0422/bot0.0048 ratio8.71 | build OK
2026-09-13T08:54:39 | T1.5 | --bench + Simulator.interact/spatial(SpatialHash重建); --interact开关 | 10k release: 1086tps(无交互)/1180tps(交互) ≫30目标36×; spatial开销可忽略 | build OK
2026-09-13T09:01:34 | T1.6 | 大跑 release pop10/gens12/ticks600/n_seeds2 + zoo/transfer/report 重生成 | champions全面碾压default(3-9×); predator冠军collected421/def0.019边防边觅; transfer特化(rich2181→scarce253); REPORT.md重生成 | build OK

2026-09-13T09:04:51 | ==== Tier 2 启动：架构升级 (可演化能量/ANN决策层/间接编码/niching) ====
2026-09-13T09:10:21 | T2.1 | 4能量常量移入Genome(字段+ranges+mutate/crossover/random/in_range); ant用g.field; 修at_nest y-bug | scarce drain0.00056 vs rich0.00149(2.6×省能); test5/5 | build OK
2026-09-13T09:37:21 | T2.2 | ANN决策层: Genome.ann_weights(141→155w)+mutate/crossover/random/in_range; brain::ann_decide(9in/10hid/5out); foraging_ann_seed(门控输入+pass-through); --brain ann; 拾取/交付反射 | gen0 collected509 trail148 四源均匀[66,166,181,96]; 演化精英维持觅食; best持平(seed近优,升待T2.3) | build OK
2026-09-13T09:45:17 | T2.3 | Genome.cppn_genes(16)+develop_cppn(余弦基)+develop_phenotype(seed+扰动); --brain cppn; evaluate/run穿brain_cppn; mutate/crossover/random/in_range加cppn | gen0 collected404(seed觅食); 演化best247→666(2.7×,超直接ANN); 16→155压缩 | build OK
2026-09-13T09:55:38 | T2.4 | genome::trait_vec/distance(归一化基因型距离); run加niche适应度共享(threshold0.25); --niche | no-niche best267(fs3.56) vs niche best417(fs2.68,不同区域且更优)→多样性维持+跳出早熟 | build OK
2026-09-13T10:01:56 | T2.5 | --brain cppn 跨环境evolve-all + zoo/transfer/report 重生成 | 冠军发散(rich exp0.02/patchy0.32/predator aggr0.79); zoo rich444→751/pred186→456/maze67→155碾压; transfer特化(rich675本土→异地40-46); scarce/patchy漂移低于种子(可调) | build OK

2026-09-13T14:52:10 | ==== Tier 3 启动：丰富生物+可视+深度演化 (GUI演化面板/文献验证/深度演化/文档) ====
2026-09-13T14:54:44 | T3.1 | GUI: brain ComboBox(fsm/ann/cppn)+应用(brain_ann/cppn develop)+collected sparkline(自绘polyline)+live metrics; brain字段+collected_hist | 编译+启动无panic; --brain ann回归collected123 | build OK
2026-09-13T15:01:46 | T3.2 | --bridge CSV+peak rising检查; --caste 守卫占比; task_jitter偏置(-0.4..1→~30%); REPORT加文献对齐小节(bridge/ caste/确定性) | bridge峰值7.31(短路径放大); caste0.315在[0.2,0.4]; REPORT含验证小节 | build OK
2026-09-13T15:09:07 | T3.3 | 深度演化 --evolve --brain cppn --niche pop30 gens50 rich_close; cp history→deep_evolution.csv | best单调315→1589(5.0×), collected1549; mean探索87→334 | build OK
2026-09-13T15:13:24 | T3.4 | README加Tier2/3能力(ANN/CPPN/niching/energy/caste/bridge命令)+决策层三选一+演化算子小节; 最终冒烟 | test5/5 PASS; 确定性PASS; release GUI无panic; REPORT重生成3451B | build OK

2026-09-13T15:42:54 | ==== Tier 4 启动：科学丰富化 (育幼stigmergy/多通道可视化/协同进化/真实数据对齐) ====
2026-09-13T15:48:54 | T4.1 | World.brood+Ant.pending_brood; Nurse持续照料(不掉Nurse); caste按task_jitter<-0.2进Nurse; sim flush brood衰+累加; --brood | FSM brood20→354(24nurses增长) vs ANN 20→1.6(无nurse衰减) | build OK
2026-09-13T15:53:45 | T4.2 | GUI: tex_all[4]通道缩略图(2×2 Trail/Home/Alarm/Recr)+brood计数label; refresh_tex刷新4缩略 | 编译+启动无panic | build OK
2026-09-13T16:04:32 | T4.3 | Ant.colony_id+pending_pickup; sim collected_a/b+食物递减+set_two_colonies; --coevolve 冠军vs挑战者有限食物 | 10轮两群都>0, winner翻转6次(军备竞赛), champ_best122→147 | build OK
2026-09-13T16:11:24 | T4.4 | REPORT加##4(brood/coevolve发现)+##5(与文献对照定性:ACO/分工/挥发/演化)+##6复现; README加--brood/--coevolve; 修字符串转义 | test5/5; release build绿; REPORT4684B 6节 | build OK

2026-09-13T17:09:59 | ==== Tier 5 启动：工程完整性与科学深度 (性质测试/定量对齐/开放性演化/100k规模) ====
2026-09-13T17:17:17 | T5.1 | sim.rs测试模块(determinism/brood非负/field非负/coevolve预算)+genome distance测试; 移除unused Environment import | cargo test 10/10 PASS | build OK
2026-09-13T17:22:06 | T5.2 | REPORT ##5加5.1定量对照表(双桥88%vs80-90/半衰期46tick/forager70%/育幼)+5.2定性+诚实局限 | REPORT5558B含定量表 | build OK
2026-09-13T17:32:34 | T5.3 | EvolveResult.diversity(per-gen mean pairwise Genome::distance)+run/run_multilevel计算+--evolve打印+open-ended检查 | cppn+niche pop30gens40: div 0.1728→0.3384(不坍缩反增2×) open=true; best315→1507 | build OK
2026-09-13T17:38:41 | T5.4 | --bench --colony 100000; README加100k性能行; 最终冒烟 | 100k@136tps(4.5×余量,alive100000); test10/10; GUI无panic; REPORT5558B6节 | build OK

2026-09-13T18:13:26 | ==== Tier 6 启动：可分享性 + 新行为 (HTML快照/文献数据/领地战争/GUI场景编辑) ====
2026-09-13T18:16:20 | T6.1 | --export-html: 采样帧→嵌入自包含HTML(canvas+FRAMES JSON+JS动画); 占位符replace避免format转义 | results/snapshot.html 132KB, 50帧, 含canvas+FRAMES | build OK
2026-09-13T18:21:36 | T6.2 | literature.json(带来源声明)+--compare-lit(模型vs文献表: bridge0.88/半衰期46tick/forager0.69/caste0.31/brood/stigmergy) | 对比表输出正常, 诚实标注 | build OK
2026-09-13T18:29:17 | T6.3 | Ant.health+Simulator.war; step flush O(n²)群间近战(WAR_RANGE2.5/DMG0.15); --war模式 | 50v50: aliveA4/aliveB9 casA46/casB41 both_cas=true winner=B | build OK
2026-09-13T18:35:12 | T6.4 | App.place_mode(none/food/enemy/wall/erase)+radio+点击路由(food/enemy/wall放置/erase近邻擦除/default deposit Trail) | 编译+启动无panic | build OK

2026-09-13T20:30:14 | ==== Tier 7 启动：生物差距修正 (SNN种子/STDP/交哺/年龄多型/brood个体发生/感觉/生态/收尾) ====
2026-09-13T20:32:58 | T7.1 | snn_decide清重复inputs+LIF_LEAK 0.3→1.0(稳态增益=1.0匹配tanh) | --brain snn collected238 survival0.865 trail59.8 四源均匀 | build OK
2026-09-13T20:40:50 | T7.2 | Ant.learned_w(per-ant可学习权重副本)+last_spike_h/o(spike时序); snn_decide用learned_w+STDP(LTP:pre先LTD:post先) | SNN+STDP collected238→318(+34%); test10/10 | build OK
2026-09-13T20:48:41 | T7.3 | Simulator.colony_energy(共享池)+flush交付→池+巢内蚂蚁从池取能; 移除个体免费补能 | rich581/surv0.82(池→存活); scarce28/surv0.085(少投递→饥饿,比0.965真实); test10/10 | build OK
2026-09-13T20:56:49 | T7.4 | brain: age-based caste(nurse_age/guard_age+task_jitter变异); Nurse arm超龄→Explore; 移除birth-fixed caste | collected438/surv1.0/trail171; 年龄→工种转换通过; test10/10 | build OK
2026-09-13T21:03:54 | T7.5 | sim flush: brood≥50→eclosion(产新蚂蚁@巢,age0→Nurse)+MAX_COLONY=1000; --brood报colony_init/final | colony200→242(42新蚂蚁); brood20→47.8; ecloded=true; test10/10 | build OK
2026-09-13T21:10:29 | T7.6 | sensors: vision_food_idx(前方57°锥形5×远距食物探测)+Recruitment沉积@拾食; brain Explore用视觉steer; 3处pickup沉积Recruitment | collected380/surv1.21(蚁群增长!)/trail52; test10/10 | build OK
2026-09-13T21:17:47 | T7.7 | Simulator.seasonal; step flush: 食物再生(+0.3/tick cap300)+周期天敌入侵(每500tick); --seasonal穿给evaluate/run; evaluate初始化food=100 | seasonal rich: collected380/surv0.785(def_frac0=敌人被防住)/trail116; test10/10 | build OK
2026-09-13T21:22:18 | T7.8 | 最终集成: cargo test(10/10) + GUI冒烟(无panic) + REPORT重生成(5558B) + 确定性PASS + git commit(7975e43) | 全绿 | build OK

2026-09-14T23:10:54 | ==== Tier 8 启动：神经模块化 AL+MB+多巴胺STDP (规格specs/ 计划plans/ 日期前缀) ====
2026-09-14T23:20:39 | T8.1 | Genome: mb_weights(Vec)+mb_dopamine_gain+mb_weight_count()(=1101)+mutate/crossover/random/in_range扩展; Ant: mb_al_v[8]+mb_kc_v(Vec64)+mb_out_v[5]+mb_al_osc+learned_mb_w+last_kc_spike(Vec64)+dopamine+octopamine; update dopamine/octopamine衰减; 常量MB_AL_INPUTS=16/MB_AL_GLOM=8/MB_KC=64/MB_OUT=5/KC_THRESH=0.5/DOPAMINE_THRESH=0.1 | cargo build+test 10/10 PASS | build OK
2026-09-14T23:33:29 | T8.2+T8.3 | brain.rs mb_decide(630行): 16输入→AL(8瞌小球收敛tanh+侧抑制WTA+振荡)→MB(64 KC随机投射+高阈值二值)→5 output(tanh)+多巴胺门控STDP(KC→out); Genome mb_seed(AL identity+KC稀疏随机+out关键连接); Ant.learned_mb_w+mb_al_v/kc_v/out_v/dopamine/octopamine; Ant::update brain_mb dispatch; sim/evolution/main 穿brain_mb | collected21>0(surv0因seed需调); test10/10 | build OK
2026-09-14T23:37:59 | T8.4 | sim flush: iter_mut loop delivery→dopamine+=mb_dopamine_gain (after immutable flush loop); mb_decide中STDP已有多巴胺门控(DOPAMINE_THRESH=0.1) | collected21(test10/10); dopamine释放机制完成(需seed调优才能显著改善) | build OK
2026-09-14T23:44:13 | T8.5 | KC_THRESH 0.5→0.15(降阈值让KC发放); mb_seed KC bias 0.3→0.0; --brain mb --evolve 5gens | evolve best 38→63(+65%) 不降; collected19(seed仍需调) | build OK
2026-09-14T23:49:36 | T8.6 | GUI brain ComboBox加"mb"; app brain_mb切换逻辑(want_mb+brain_ann分别设置); --brain mb已穿入evaluate/run/evolve | test10/10; mb collected22/surv0.38; GUI无panic | build OK
2026-09-14T23:55:21 | T8.7 | README加--brain mb+蘑菇体说明; REPORT重生成5558B; git commit 6c767b0(10files+359-31); 全部验证通过 | test10/10; mb collected19; 确定性PASS; GUI无panic; REPORT含6节 | build OK

2026-09-17T15:23:05 | ==== Tier 9 启动：中央复合体 CX 环吸引子路径积分（神经化归巢向量） ====
2026-09-17T15:23:05 | T9.1 | Genome: CX_N=16+cx_bump_gain/cx_inhibition/cx_compass_gain/cx_shift_gain/cx_hv_leak(cx_*基因索引26-30)纳入mutate/crossover/random/in_range; Ant: use_cx/cx_bump[16]/cx_hv_x/cx_hv_y/cx_prev_heading+home_vector()分发(use_cx时读神经积分否则软件真值) | cargo build+test PASS | build OK
2026-09-17T15:23:05 | T9.2 | brain.rs cx_integrate(环吸引子: WTA余弦核+侧抑制维持单一bump, 按角速度shift平移, 太阳罗盘compass注入, 除法归一化)+cx_heading(population vector解码)+cx_home_error(ground-truth对照); CPU4类似物累加cx_hv_x/y(漏积); CarryReturn改用home_vector() | 单测4/4(bump稳定+朝向跟踪/bump平移正确/积分误差有界/home_vector分发); 确定性PASS | build OK
2026-09-17T15:23:05 | T9.3 | 穿线: main.rs --brain cx解析+run_headless/evolution穿brain_cx; sim.rs brain_cx字段+step_ants/update传参; app.rs GUI ComboBox加"cx"+切换逻辑(修复原仅want_mb触发bug); README/CLAUDE加cx说明 | --evolve-ml 3gens colony_score85.6→139.2(演化cx_*生效); cx collected198 vs FSM819(精度低于真值,符合神经积分预期,colony200→487健康); test14/14 | build OK

2026-09-17T16:40:00 | T9.4 | CX 加长演化: --evolve --brain cx rich_close pop16 gens12(首轮)/gens16×4seed(二轮) | 首轮 collected884; 二轮 16代4seed collected1613(远超FSM819, 多样性0.17→0.30不坍缩); 存 results/evolved_rich_close.toml; 采纳演化 cx_* 默认值 → CX out-of-box collected198→597 | build OK

2026-09-17T17:15:00 | ==== Tier 10 启动：多模态整合（视觉+距离→MB 层） ====
2026-09-17T17:15:00 | T10.1 | sensors: 加 vision_food_bearing/dist + nest_dist 到 per-ant sensors; genome: MB_AL_INPUTS 16→20/MB_AL_GLOM 8→12, mb_seed 视觉/prox 输入接到 glomeruli 8-11 | 编译通过 | build OK
2026-09-17T17:20:00 | T10.2 | brain.rs mb_decide: inputs 数组加 4 新模态(v_sin/v_cos/v_prox/nest_prox, 视觉门控无目标时归零); al_raw/al_out 数组从硬编码[0.0f32;8]改为跟 MB_AL_GLOM(修 OOB panic) | 冒烟通过 | build OK
2026-09-17T17:25:00 | T10.3 | 修回归: 原 mb_seed KC 稀疏投影用 seeded RNG gen_range(0..al_g), al_g 8→12 后整条 KC 布线重排稀释嗅觉核心 → 改确定性投影(k%12,(k+4)%12,(k+8)%12 均匀铺满12 glom) | MB 3000t collected 4→49(反超原基线19×2.6); test14/14; mb+cx 确定性PASS; fsm/ann/cx 冒烟无回归; git commit abad889 | build OK

2026-09-17T17:45:00 | ==== Tier 11 启动：神经调质动态（章鱼胺/多巴胺经验驱动→动机/情绪） ====
2026-09-17T17:45:00 | T11.1 | genome.rs: 常量 OCT_FORAGE_GAIN=0.02/OCT_REST_DECAY=0.03/OCT_MAX=1.0; 清理 Tier10 遗留 unused SeedableRng import | 编译通过 | build OK
2026-09-17T17:48:00 | T11.2 | mod.rs Ant::update: 神经调质改为经验驱动(替代纯衰减)——octopamine 觅食(Explore/FollowTrail非负重)时 +OCT_FORAGE_GAIN 封顶 OCT_MAX, 负重/育幼时 -OCT_REST_DECAY 保底0; dopamine 仍 *0.95 快衰减 | 编译通过 | build OK
2026-09-17T17:50:00 | T11.3 | brain.rs decide(): octopamine 耦合行为——探索强度 explore_rate×(1+arousal)、轨迹响应阈值 trail_threshold×(1-0.5·arousal)、攻击性 aggressive 判据 +0.5·arousal | 编译通过 | build OK
2026-09-17T17:52:00 | T11.4 | brain.rs mb_decide() 移除冗余 octopamine+=0.001(已收敛到共享 update); sim.rs flush(): pickup→dopamine 释放(奖励预测, 0.5×gain 封顶2.0) | 编译通过 | build OK
2026-09-17T17:55:00 | T11.5 | 验证: FSM 3000t rich_close collected 819→1135(+38%!唤醒耦合提升觅食); ann114/snn65/mb7/cx29 无回归; fsm+cx+mb 确定性PASS; test14/14 | 全绿 | build OK

2026-09-17T18:10:00 | T12 | 探索: 涌现式 AL 弛豫振荡器(慢全局抑制 interneuron mb_al_inh 替代硬编码 cos 相位) —— 已回滚 | 慢抑制弛豫产生 boom-bust 循环(抑制期把觅食压死): MB 49→15→2; 真实 AL 20-30Hz 是 GABA 快速反馈的对称振荡, 硬编码 cos 反而是更准确模型; 回滚后 MB 45 恢复。结论: gap D(振荡/时间编码)已由 Tier8 硬编码 AL 振荡满足, 无需涌现化 | build OK

2026-09-17T18:30:00 | ==== Tier 13 启动：发育性结构可塑性（一生内 MB 神经发生） ====
2026-09-17T18:30:00 | T13.1 | genome: mb_neurogenesis 可演化字段(每交付长 KC 数, 0.2~3.0 默认 1.0)+MB_KC_INIT=24 常量; 镜像 mb_dopamine_gain 全套(struct/default/mutate/crossover/random/in_range) | 编译通过 | build OK
2026-09-17T18:33:00 | T13.2 | Ant: mb_kc_active 字段(初始 MB_KC_INIT)+grow_mb() 方法(封顶 MB_KC); mb_decide KC发放/输出/STDP 循环改用 kc_act=min(mb_kc_active,64), 体眠 KC 不参与; sim flush 交付→grow_mb(神经发生) | 编译通过 | build OK
2026-09-17T18:36:00 | T13.3 | 单测 neurogenesis_grows_and_caps(初始=MB_KC_INIT/grow 递增/封顶 MB_KC)+main final 遥测(ants alive + mb_kc_active mean/max) | 15/15 测试; MB 500t mean24.2/max26 增长可见 | build OK
2026-09-17T18:40:00 | T13.4 | 扫描 MB_KC_INIT 24/32/40/48/56: collected 40/33/25/22/17(单调递减), 64(无门控)=45。取 24(门控中最佳+生物合理 37.5% 成年MB) | MB 3000t collected 40(vs 无门控45, -11% 为未成熟MB代价); FSM 仍1135 无回归; mb 确定性PASS; 注: MB 蚁群 3000t 饿死(ants alive=0) 是 T8 老问题(弱脑+T7.3 营养不足)非本层引入 | build OK

2026-09-17T18:50:00 | ==== Tier 14 启动：CPG 步态层（刻板 tripod + octopamine→步频→速度） ====
2026-09-17T18:50:00 | T14.1 | genome: cpg_freq 可演化字段(基础步频 0.5~2.0 默认1.0)+CPG_LEGS=6/CPG_AROUSAL_GAIN=0.3 常量; 镜像全套(struct/default/mutate/crossover/random/in_range) | 编译通过 | build OK
2026-09-17T18:52:00 | T14.2 | Ant: leg_phase[6] 字段(初始 tripod [0,π,0,π,0,π])+step_cpg(omega); 运动层 omega=cpg_freq×(1+0.3·octopamine), speed=max_speed×(omega/cpg_freq), 每 tick step_cpg(omega) | 编译通过 | build OK
2026-09-17T18:54:00 | T14.3 | 踩坑: 对称 Kuramoto 耦合(同奇偶同步/异奇偶反相)有寄生吸引子(全同步也是固定点因 sin(π)=0), 测试不收敛→改刻板 tripod(主相位+硬编码偏移, 生物正确: 昆虫 tripod 相位关系是 CPG 半中心互抑制的刻板输出, 频率才是被调制的动态部分) | 单测 cpg_produces_tripod_gait 通过 | build OK
2026-09-17T18:56:00 | T14.4 | 验证: FSM 1135→1204(+6% 唤醒→速度↑覆盖更广); MB 40→56(补偿未成熟MB代价, 反超基线45); ann/snn/cx 300t 均提升; 16/16 测试; fsm+mb 确定性PASS | 全绿 | build OK

2026-09-17T19:00:00 | ==== 全 brain 跨环境演化大跑（6 brain × 5 env = 30 轮，600t×8gens×10pop×3seed） ====
2026-09-17T19:00:00 | 大跑结果(evolved best collected): fsm rich994/scarce104/pred508/patchy350/maze84; ann 280/40/103/115/45; cppn 910/43/480/254/62; snn 168/31/74/99/20; mb 291/0.7/123/45/7; cx 861/92/195/200/85 | 存 results/cross_brain_evolution_summary.txt | 30轮完成

2026-09-17T19:05:00 | 大跑洞察: ①CX 是神经网络里最稳健的(总1433, maze 85 追平 FSM 84, scarce 92 接近 FSM 104)——神经路径积分确实有效; ②MB 双峰(rich 291 但 scarce 0.7/maze 6.7 崩)——学习脑 600t 短跑无时间 STDP 学习, 印证 MB 弱在短跑而非不可演化; ③CPPN 间接编码意外强(rich 910/pred 480, 总1749 第二); ④FSM 手写基线仍最强(总2040); ⑤SNN LIF 最弱(总392)。环境难度: rich< predator<patchy< scarce≈maze | 结论存档 | build OK

2026-09-17T19:15:00 | MB 长评估验证(3000t×8gens): best collected 309.7 vs 600t 的 291(+6% 几乎无提升) | 根因修正: 非学习时间不足, 而是蚁群饿死——evolved MB 3000t 直接跑 collected=304 但 ants alive=0; 304×0.5=152 能量 vs 200蚁×3000×0.0009=540 消耗, T7.3 营养池下 MB 觅食速率喂不饱蚁群, 拉长评估没用。MB 真正瓶颈是觅食速率(输出映射 KC→turn/trail/home 太弱), 非 STDP 时间 | 结论修正存档 | build OK

2026-09-17T22:00:00 | ==== Tier 15 启动：MB 觅食活力（让学习脑能活到学会）规格 specs/ 计划 plans/ 已批 ====
2026-09-17T22:05:00 | T15.1 | mb_decide 输入槽3/15 改 carrying 门控转向(out_steer=(1-c)·ts·tv+vs / in_steer=c·ns)；删未用 av；复用 tv 为 trail 门控；dep_home 魔数0.1加注释；移除死字段 mb_al_v(尺寸8≠12)；新增 MB 私有 last_mb_out_spike[MB_OUT] 替代复用 last_spike_o(ANN 共用)；mb_seed 振荡 osc_mod 0.5±0.5→0.8±0.2(检测器KC需可靠发放) | cargo build+test 16/16 PASS | build OK
2026-09-17T22:15:00 | T15.2 | 根因深挖: 二值KC无法编码带符号分级转向→sign-splitting bang-bang 有41°死区, 默认mb_seed 600t 仅收集180(虽3×旧默认56)仍饿死(需0.36/tick, 仅0.30)。调参(死区-0.1/降噪/强trail)均更差。改方案: 转向做成分级先天反射(al_raw[0]+al_raw[1] 即AL→turn, 旁路二值KC, 类昆虫侧角innate通路), KC→output 保留二值+STDP给信息素决策。+视觉加入outbound(食物在15内可见时beeline)。+年龄多型育幼(年轻蚁@巢→Nurse+pending_brood, 镜像FSM, 给MB eclosion增长引擎) | 默认MB 600t: 180→256 survival1.0; 仍1000t后饿死(盈亏点无余量) | build OK
2026-09-17T22:25:00 | T15.2验证 | 演化 --brain mb rich_close 8gens/10pop/3seed: gen0 best190→gen5 best588(collected539.7, 3×默认), 多样性0.18→0.25不坍缩。evolved champion 3000t: collected=2191(旧evolved 304→7.2×!), survival=0.150(蚁群存续不再全饿死), 600t collected910 survival1.217(增长)。超过FSM默认1470 | MB 不再垫底 | build OK
2026-09-17T22:28:00 | T15.3 | brain.rs 新 mb_tests 模块3测: outbound trail左/右→转向对应侧(graded reflex符号正确); carrying+nest在一侧→转向nest。共19测试全过 | test 19/19 PASS | build OK
2026-09-17T22:30:00 | T15.4 | 确定性 --brain mb --verify-determinism 1000t PASS。其余brain 300t冒烟无回归(fsm58/ann193/cppn193/snn120/cx56, 无panic)。MB×5env演化sweep(8gens/10pop/3seed): rich539.7/scarce32.3/pred95/patchy80/maze32, 总分779(旧535)→超snn(392)脱离末位; scarce 0.7→32.3、maze 6.7→32 大幅改善 | cross_brain_evolution_summary.txt MB行更新 | build OK
2026-09-17T22:40:00 | T15.6 | 文献定量对照: 新增 --bench-cx(CX路径积分误差 vs Cataglyphis: dist10/20/40/80 → mag_err 0.0/0.1/0.1/0.3%, dir_err 0°; 模型比真实蚁更准(无传感噪声), 定性缩放一致)。扩展 literature.json(+CX PI drift/dir, +MB KC sparsity/associative learning 4条)。扩展 --compare-lit 计算CX PI drift 0.1%(lit 5-15%)/dir 0°(lit几度)/MB KC sparsity 19%(lit 5-15%,略高)/MB多巴胺门控STDP(lit Drosophila true) | compare-lit 10行表输出正常 | build OK

2026-09-17T23:10:00 | ==== Tier 16 启动：多巴胺 RPE 化 + 价态 + aversive 学习（方案已批，bug 先修 → 重要性排序） ====
2026-09-17T23:12:00 | T16.1 | bug: genome 加 DOPAMINE_MAX=2.0 常量；sim.rs 交付路径 dopamine 加 .min(DOPAMINE_MAX) 与拾食一致(原交付无封顶) | build+test 19/19 PASS | build OK
2026-09-17T23:25:00 | T16.2 | 双通道多巴胺: Genome mb_dopamine_gain→reward_gain+punish_gain(PAM/PPL1 类比, 均 default0.5 可演化[0.1,1.0], +#[serde(default)] 兼容旧TOML) 镜像全套算子; Ant dopamine→dopamine_reward/punish(均 0.95 衰减); MB STDP LTP 改 reward 门控、LTD 改"基线0.005遗忘+punish+0.005增强"(去掉纯ungated LTD, 加 aversive 调控); sim 释放改 dopamine_reward | 踩坑: 完全去掉 ungated LTD→LTP 失衡杂散累积→MB 600t 79(崩); 恢复基线0.005→178(Tier15基线, 无回归); test19/19; 确定性PASS | build OK
2026-09-17T23:30:00 | T16.3 | aversive: sim war 块应用 health-=dmg 后若 dmg>0 释放 dopamine_punish(封顶 DOPAMINE_MAX)——受伤→punish→近期活跃 KC 做 LTD, aversive 学习闭环(仅 war, 不重平衡 predator) | --war --brain mb 1000t: 双方均伤亡 both_casualties=true | build OK
2026-09-17T23:35:00 | T16.4 | ANN STDP 纳入多巴胺门控: ann_decide LTP 加 dopamine_reward 门控、LTD 加 dopamine_punish 增强(基线0.005+punish+0.005); 多巴胺调制不再局限 MB; innate foraging_ann_seed 不受影响 | ANN 300t=193/3000t=2107 surv0.813 无回归 | build OK
2026-09-17T23:45:00 | T16.5 | 4 新测试(mb_punish_amplifies_ltd 直接 / reward_dopamine_released_on_foraging / punish_dopamine_released_on_war_damage / mb_reward_dopamine_enables_ltp 奖励门控LTP)→共23测试全过; compare-lit MB 行改"双通道多巴胺(reward PAM/punish PPL1)门控STDP"; literature.json 补 PAM/PPL1 双 DAN 通路; 旧 Tier15 champion 靠 serde default 仍加载(600t 收915) | 全绿; 确定性PASS; MB/ANN 无回归 | build OK

2026-09-18T00:10:00 | ==== Tier 17 启动：饱食调制 + W_kc 可塑性门控（合并 master 后，方案已批） ====
2026-09-18T00:15:00 | T17.1 | 饱食调制: Genome 加 mb_satiety_gain(default0.5,[0,1.5],+serde default) 镜像全套算子; sim 拾食/交付奖励 gain 改 eff=base·(1+satiety·(1−2·energy)) 居中调制(能量0.5=base, 饥饿0→1.5×, 饱1→0.5×), clamp≥0 | MB 600t 178→182(无回归, 略升); test 23/23 | build OK
2026-09-18T00:22:00 | T17.2 | W_kc 可塑性: genome 加 MB_DETECTOR_KCS=10; mb_decide STDP 段后加 AL→KC Hebbian 塑性(reward→LTP/punish→LTD, 速率0.002, 仅 k≥10 基底KC, 检测器0-9保护, W bounds夹紧) | MB 600t 182→194(略升); 确定性PASS; test 25/25(2新: satiety_modulates_reward_dopamine 饥饿>饱食 / mb_wkc_plasticity_reward_ltp 奖励驱动基底KC W_kc 上升) | build OK
2026-09-18T00:25:00 | T17.3 | literature.json mb_associative_learning 补"AL→KC Hebbian + 饱食调制"; README/CLAUDE 多巴胺说明补两项; 25测试全过; MB/ANN 无回归 | 全绿 | build OK

2026-09-18T01:30:00 | ==== Tier 18 启动：分室化DAN + 开放性行为度量 + 文献数据 + 工程/GUI（4部分依次） ====
2026-09-18T01:35:00 | T18.1 | 分室化DAN: genome 加 MB_APPROACH_OUT=3; mb_decide STDP 按 output 价值分室——approach(0,1,2) reward→LTP/punish→LTD, avoidance(3,4) opponent punish→LTP/reward→LTD(PAM/PPL1 分室); 基线遗忘LTD全作用。觅食无回归(avoidance输出在无威胁时不发放), war下防御输出强化 | MB 600t 194→228(avoidance不再被reward错强化); 26测试(新mb_compartmentalized_avoidance_ltp); 确定性PASS | build OK
2026-09-18T01:50:00 | T18.2 | 开放性行为度量: Fitness加behavior向量(6状态占比+per_source归一化+trail tanh); evaluate采样全6态(原仅Defend/Alarm, mean_def_frac由全向量派生不变); EvolveResult加diversity_behavior; run()双多样性O(pop²); --evolve打印双多样性, open-ended要求两者都不坍缩; run_multilevel个体级保持基因型only | --evolve cppn niche: 基因型0.175→0.239/行为0.249→0.255 双不坍缩 open=true; 27测试(新behavior_distance_distinguishes) | build OK
2026-09-18T02:10:00 | T18.3 | 文献数据对接: literature/*.toml 显式数据点(bridge_deneubourg/caste_gordon, 带source+caveat, TOML复用toml crate免新依赖); --compare-lit加FIT段(加载TOML算abs_err/in_range); bridge 0.88∈[0.80,0.90]yes err0.06, forager 0.69 vs 0.30[0.20,0.40] no(定义差异,显式拟合暴露); REPORT§5.2 + literature.json note 更新 | compare-lit + report 输出正常 | build OK
2026-09-18T02:30:00 | T18.4 | war O(n²)→spatial hash: 用现有SpatialHash(rebuild+query_near,原死代码), 每蚁查WAR_RANGE内邻居, j>i去重, O(n·k); 5000蚁war流畅(原12.5M pairs); 同伤亡(50蚁18/21), 确定性PASS。GUI MB活动面板: brain_mb时加KC稀疏sparkline+reward/punish多巴胺条(复用sparkline/thumbnail模式) | 27测试, MB600t=228无回归; GUI build-clean(无显示环境未跑运行时) | build OK

2026-09-18T03:00:00 | ==== Tier 19 启动：新奇度搜索 + SNN活力 + 文献曲线 + GUI深化（4部分依次） ====
2026-09-18T03:10:00 | T19.1 | 新奇度搜索(NSLC-lite): behavior_distance 改 pub; run()加 novelty 参数; --novelty flag; novelty[i]=种群内 k(pop/4∈[1,5]) 最近邻行为距离均值; select_scores += NOVELTY_W·novelty·mean|score|(兼容 --niche); EvolveResult.novelty; --evolve 打印 | --evolve cppn --novelty: novelty init0.180→final0.134 序列打印; 28测试(新novelty_higher_for_diverse_behaviors); 默认off无回归 | build OK
2026-09-18T03:30:00 | T19.2 | SNN活力(镜像MB Tier15): snn_decide 顶部加育幼 early-return(age<nurse_age@巢→Nurse+brood, eclosion增长引擎); input[0] 改 (1-c)·(tv·ts+vs) 含视觉奔食(不扩ANN_IN,不动ANN) | SNN 600t 120→148 survival1.217(增长!); 3000t collected705 survival0.307(不再全饿死!); 30测试(2新snn nurse/steer); 确定性PASS; SNN脱离垫底 | build OK
2026-09-18T03:50:00 | T19.3 | 文献曲线: LitPoint加 x:Option<f32>; FIT迭代所有点; literature/cx_cataglyphis.toml(多距离PI漂移10/20/40/80) + mb_drosophila.toml(学习比); --compare-lit CX多距离拟合(0.03-0.28% vs 8-12%,模型更准) + MB学习比(late/early 0.55 vs 1.5,诚实负结果colony衰退主导); 踩坑: bridge/caste TOML字符串x与Option<f32>冲突→移除x; MB ratio前200t育幼期0→改用200-400 vs 600-800窗口; REPORT§5更新 | compare-lit 4行FIT正常; 30测试 | build OK
2026-09-18T04:10:00 | T19.4 | GUI深化: app.rs CX环吸引子bump可视(brain_cx,16点环+heading箭头); 演化历史CSV回放(Load champion时读evolved_*_history.csv, 多线sparkline best/mean/worst); 蚁神经叠加overlay toggle(none/dopamine/carrying, dopamine染色蚁) | 30测试, MB600t=228无回归; GUI build-clean(无显示未跑运行时) | build OK

2026-09-19T01:00:00 | ==== Tier 20 启动：--validate 行为涌现电池（大规模鲁棒性+定性涌现验证） ====
2026-09-19T01:20:00 | T20.1 | --validate 模式: 跨6脑×5环境×3000t(默认未演化基因组), 每100t采样 alive/collected/guard_frac/trail_total/brood; 5判据 survival>0.1/trail_formed(峰>2且末>1)/caste_stable(σ<0.10)/colony_grew(峰>1.05·初始)/foraging(末500t有交付); 打印表+写results/validate.md+诚实评估段; --brain/--env/--ticks可限范围。踩坑: trail_formed 判据"末>0.1·峰"过严(FSM trail峰460后consolidate到34=7%峰,非坍塌)→改"峰>2且末>1"; sim.world.nest 需从cfg设(同--fitness) | fsm/rich 5/5; 30测试无回归 | build OK
2026-09-19T01:40:00 | T20.2 | 全量验证: 75/150(50%)通过。rich_close 6脑均涌现良好(fsm/snn/cx 5/5, ann/cppn 4/5, mb 3/5); hard env(scarce_far/maze)默认脑全饿死(survival 0)——需演化适配非模型故障(EVOLVED sweep 已示硬环境可收集); predator/patchy fsm/snn/cx 较好。诚实边界: 内部行为合理性+定性涌现验证,非与物种定量一致 | validate.md 含表+判据+评估; ann==cppn(未演化零基因) | build OK

2026-09-19T02:30:00 | ==== Tier 21 启动：演化适配硬环境（回应 Tier 20"需演化适配"） ====
2026-09-19T02:40:00 | T21.1 | --evolve 加 --out-stem(自定义冠军输出名, 默认 evolved_<env>); --validate 加 --evolved flag(每 brain×env 载 evolved_<brain>_<env>.toml, 失败回退默认); cppn 验证时 develop_phenotype | build+test 30/30 | build OK
2026-09-19T02:55:00 | T21.2 | 演化 6脑×{scarce_far,maze}(8gen/10pop/3seed/600t) --out-stem evolved_<brain>_<env>: fsm scarce92/maze53.7, cppn 83.3/40.7, cx 76.3/49.3, ann 40.3/30.7, mb 31.7/27.3, snn 21.3/11.7 | 12 champion 存档 | build OK
2026-09-19T03:05:00 | T21.3 | --validate --evolved --ticks 3000 vs 默认: scarce_far fsm 0→1.17 surv(5/5 增长!), cppn 0→0.41(4/5)——演化适配 fsm/cppn; ann/snn/mb 仍饿死(600t 收集不够 3000t); maze 全仍饿死。根因: 600t 评估选 600t 收集非 3000t 存活。长视野验证: fsm/maze 用 3000t 评估演化(6gen)→冠军 collected 399.5@3000t→validate 5/5 surv1.21(增长!)——闭合 maze gap。结论: 演化适配真实, 评估视野需匹配目标存活视野 | validate_evolved.md 含对比表+长视野结果 | build OK
2026-09-19T04:00:00 | T21+ 10000t视野实验: fsm/maze 10000t-eval演化(6gen/2seed)→冠军 collected 1489.5@10000t; 3-way @10000t: 默认 1/5(饿死), 3k-eval冠军 5/5(surv1.02 泛化!), 10k-eval冠军 5/5(surv1.14 略优)。发现: 演化选过饿死视野(~3k)即泛化到10k, 10k-eval边际略优; 600t-eval没过饿死点→3k/10k都死。诚实演化结论: 需选过生存阈值 | validate_evolved.md 加 10k 段 | build OK
2026-09-19T04:30:00 | T21+ scarce_far 10k对比: fsm/scarce_far 3k-eval演化(485.5@3k)+10k-eval演化(1388@10k)。4-way @10000t: 默认 1/5(饿死), 600t-eval 5/5(surv0.78), 3k-eval 4/5(surv0.08 近崩!), 10k-eval 5/5(surv1.13)。非单调+环境依赖: scarce_far 3k-eval在10k近崩(过拟合), 与maze(3k泛化)相反。诚实结论: 视野匹配最可靠, 中间视野泛化环境依赖可能过拟合 | validate_evolved.md 加 scarce_far 段 | build OK
2026-09-19T05:00:00 | T21+ 复现性收尾: 加 --seed flag。跑2独立3k-eval演化验证 scarce_far 近崩: A(n5,seed42,673.6@3k)→10k surv0.47(5/5), B(n2,seed123,774.5@3k)→10k surv0.00(1/5全崩)。3冠军@10k survival 0.08/0.47/0.00——高方差非确定性过拟合。对比10k-eval(1.13稳健)。诚实结论: 视野匹配最可靠, 3k-eval在10k高方差不稳定; 原近崩0.08是高方差区样本。软化"过拟合"措辞为"非目标视野选择高方差" | validate_evolved.md 加复现段; 30测试 | build OK

2026-09-19T05:30:00 | ==== Tier 22 启动：消融实验→可证伪预测（系统作充分性/消融引擎） ====
2026-09-19T05:40:00 | T22.1 | Simulator 加 ablate_octopamine/trail/eclosion 标志; step 接线: octopamine 每tick清零(OA沉默类比), Trail deposit 跳过(信息素干扰类比), brood eclosion 跳过(nurse移除类比); Channel import 修 | build+test 30/30 | build OK
2026-09-19T05:50:00 | T22.2 | --ablate <mech> 模式: baseline vs ablated 各跑一环境, 比 collected/max_alive, 打印 %变化 + 方向感知预测陈述(映射真实干预: OA受体敲低/信息素干扰/nurse移除)。踩坑: 预测文本原假设"ablation总drop", trail实际增产(+44%)→改方向感知 | build OK
2026-09-19T06:00:00 | T22.3 | 3消融结果: octopamine@scarce -52.7%(OA驱动觅食, 预测OA敲低降觅食); trail@patchy +44.4%(反直觉! trail过度收敛瓶颈, 消融分散→高吞吐, 预测多源环境信息素干扰增产); trail@scarce +40%同向; eclosion@rich max_alive -17.4%(预测nurse移除停增长)。诚实: 充分性≠必要性, 方向是预测内容非定量 | ablations.md 含表+3预测+边界+复现 | build OK

2026-09-19T06:30:00 | ==== Tier 23 启动：保存可验证范围文档 + 补全所有可验证消融 + 湿实验protocol ====
2026-09-19T06:35:00 | T23.1 | docs/research_scope.md: 可验证(7类:A机制充分性/B基因×环境/C架构鲁棒/D分工/E协同进化/F视野泛化/G新奇度) vs 不可验证(10项:物种定量/必要性/回路保真/田间/分子/学习曲线/系统发育/信息素化学/个体识别/规模) + 元结论 | 文档存档 | build OK
2026-09-19T06:50:00 | T23.2 | 补全5消融: Ant加 ablate_vision/ablate_stdp per-ant标志; Simulator加 ablate_homevector/reward/punish/vision/stdp; step接线(homevector清零home+cx_hv; vision/stdp per-ant传播; reward跳过release; punish跳过release); sensors跳过vision; mb_decide/snn_decide STDP段加 ablate_stdp guard; --ablate扩展8机制(behavioral+learning读出, punish用war+forage) | build OK
2026-09-19T07:00:00 | T23.3 | 8消融结果: octopamine@scarce -52.7%; trail@patchy +44.4%(反直觉); eclosion@rich max_alive -17.4%; homevector@scarce -44.6%; vision@rich_fsm +23.2%/@scarce_mb -9.2%(环境依赖); stdp@mb drift -100%; reward@mb drift -58.1%; punish@war 无定论(噪声大). 6/8稳健预测, 2反直觉(trail/vision), 1无定论(punish诚实标注). 写 results/ablations.md(8表+逐项湿实验protocol: 物种/干预/读出/对照/预期+机制) | 30测试 | build OK
2026-09-19T07:30:00 | T23+ 偏差检验: 反直觉trail/vision是否sim偏差? 加--food-amount. 检验trail@patchy: 有限食物(200/源)+44.4%未翻转(非无限食物偏差); 大群落1000 +98%更强(非小群落artifact); 跨脑ann -31%翻转!(trail帮ann). 判定: trail-hurts是FSM-trail实现artifact(过收敛/失配), ANN trail有益符合真蚁. vision@rich跨脑: fsm+23/cx+40(用FSM vision-steer覆盖trail)/ann 0(无视觉)/mb+3.9——也是FSM-框架artifact. 诚实撤回trail/vision的普遍湿实验预测, 定位FSM调参为改进靶点. 跨脑测试=判别sim-artifact vs robust预测的决定性工具 | ablations.md加偏差检验节 | 30测试 | build OK
2026-09-19T08:00:00 | T23+ 修偏差: FSM FollowTrail硬爬升=过收敛artifact根因. 修: Explore改分级trail软偏置(steer_toward(trail_bearing,turn_rate×0.5)+探索噪声持续,不切FollowTrail硬爬升), 镜像ANN分级模式. 结果: trail消融+44%→-46.9%(翻转有益!); vision消融+23%→-30.6%(翻转有益!); FSM产量大涨(patchy 198→539, rich 1204→4566, 旧硬爬升在拖累FSM). octopamine转环境依赖(rich-22.6%/patchy-34.3%帮, scarce+19.6%伤——defensible, OA探索多源帮/单源-trail-高效伤). 7/8直观或可解释, punish仍无定论. 30测试/确定性/validate全过 | ablations.md更新修复后表 | build OK

2026-09-19T15:20:00 | ==== Tier 24 启动：全脑×环境×视野演化大扫（3k vs 10k, bias-fixed FSM） ====
2026-09-19T15:20:00 | T24 | results/full_evolve_3k_10k.sh: 6 brain×5 env×2 视野(3000/10000 eval tick)=60轮, 8pop/6gen/2seed, 4并行, 写 full_evolve_3k_10k.csv | 60轮完成(10k全/3k除snn4环境截断); csv+per-run log+84 evolved_*.toml 存档 | build OK
2026-09-19T15:20:00 | T24.2 | 汇总分析: 10k视野FSM全环境第一(总分64437=2.4×cx27324), 架构排名fsm>cx>cppn>ann>mb>snn跨视野稳定; CX最强神经脑(predator7383/rich16212, 1.5-3×缩放佐证PI可演化优势); 视野缩放异质(fsm/cx增长, 弱脑停滞/回退——觅食速率天花板+营养池饿死); 硬环境仅fsm/cx过生存阈值. 诚实局限: score跨视野不可直接比/弱脑高方差/snn3k缺4环境 | REPORT.md §7 加全扫矩阵+洞察+局限; 诚实边界存档 | build OK
2026-09-19T15:35:00 | T24.3 | 补齐 snn 3k 缺的4环境(首跑日志截断): 重跑 scarce_far 62.0/predator 234.2/patchy 261.5/maze 48.7(collected0, 靠survival项得48.7). snn 3k总分1223.8仍末位, 排名跨视野仍稳定 fsm>cx>cppn>ann>mb>snn. CSV由60 log重新生成(clean brain×env×horizon序) | REPORT §7.2填snn行/§7.4标注补齐; 60轮全完整 | build OK
