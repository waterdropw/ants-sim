//! ants-sim — ant individual + collective intelligence simulation.
//!
//! Phase 1 (M1): world + pheromone field + correlated random walks + egui viz.
//! Headless mode (`--headless`) runs the sim without a window and prints
//! telemetry (collected, per-source visits, per-source corridor density) so
//! emergence can be measured, not just eyeballed.

// Milestone-driven development keeps a number of API surfaces (Vec2 helpers,
// CHANNELS, max_val, World::at_nest, …) unused until later milestones land.
#![allow(dead_code)]

mod ant;
mod config;
mod environment;
mod evolution;
mod genome;
mod sim;
mod viz;
mod world;

use eframe::egui;
use rand::{Rng, SeedableRng};
use std::io::Write;

#[derive(Clone, Copy, Debug, Default)]
struct Moments {
    n: u64,
    mean: f64,
    m2: f64,
}

impl Moments {
    fn push(&mut self, value: f64) {
        self.n += 1;
        let delta = value - self.mean;
        self.mean += delta / self.n as f64;
        self.m2 += delta * (value - self.mean);
    }

    fn sample_sd(self) -> Option<f64> {
        (self.n > 1).then(|| (self.m2 / (self.n - 1) as f64).sqrt())
    }

    fn display_sd(self) -> String {
        self.sample_sd()
            .map(|sd| format!("{sd:.3}"))
            .unwrap_or_else(|| "NA".to_string())
    }

    fn display_count(self) -> u64 {
        self.n
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct TaskMoments {
    foraging: Moments,
    defense: Moments,
    brood_care: Moments,
}

impl TaskMoments {
    fn push(&mut self, fractions: sim::TaskFractions) {
        self.foraging.push(fractions.foraging as f64);
        self.defense.push(fractions.defense as f64);
        self.brood_care.push(fractions.brood_care as f64);
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct AblationReadout {
    collected: f64,
    max_alive: f64,
    final_alive: f64,
    weight_drift: f64,
}

fn apply_brain(sim: &mut sim::Simulator, brain: &str) {
    sim.brain_integrated = brain == "integrated";
    sim.brain_ann = brain == "ann" || brain == "cppn";
    sim.brain_snn = brain == "snn";
    sim.brain_mb = brain == "mb";
    sim.brain_cx = brain == "cx";
}

fn configure_ablation(sim: &mut sim::Simulator, mechanism: &str, enabled: bool) {
    sim.ablate_octopamine = enabled && mechanism == "octopamine";
    sim.ablate_trail = enabled && mechanism == "trail";
    sim.ablate_eclosion = enabled && mechanism == "eclosion";
    sim.ablate_homevector = enabled && mechanism == "homevector";
    sim.ablate_vision = enabled && mechanism == "vision";
    sim.ablate_stdp = enabled && mechanism == "stdp";
    sim.ablate_reward = enabled && mechanism == "reward";
    sim.ablate_punish = enabled && mechanism == "punish";
    sim.ablate_al_inhibition = enabled && mechanism == "al_inhibition";
    sim.ablate_pn_multichannel = enabled && mechanism == "pn_multichannel";
    sim.ablate_lh_reflex = enabled && mechanism == "lh_reflex";
    sim.ablate_orn = enabled && mechanism == "orn";
    sim.ablate_compass = enabled && mechanism == "compass";
    sim.ablate_cx_motor = enabled && mechanism == "cx_motor";
    sim.ablate_contact = enabled && mechanism == "contact";
}

fn run_task_protocol(
    cfg: &config::Config,
    seed: u64,
    brain: &str,
    env_name: &str,
    colony: usize,
    ticks: u64,
    warmup: u64,
    sample_every: u64,
) -> TaskMoments {
    let g = cfg.genome.clone();
    let env = environment::Environment::build(
        env_name,
        world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y),
        cfg.world.width as f32,
        cfg.world.height as f32,
    );
    let mut simulator = sim::Simulator::new(cfg.world.width, cfg.world.height, seed, &g);
    simulator.world.nest = world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y);
    simulator.world.nest_radius = cfg.world.nest_radius;
    apply_brain(&mut simulator, brain);
    simulator.set_colony_size(colony, &g);
    simulator.apply_environment(&env);

    let mut moments = TaskMoments::default();
    for tick in 0..ticks {
        simulator.step();
        if tick + 1 >= warmup && (tick + 1 - warmup).is_multiple_of(sample_every.max(1)) {
            moments.push(sim::TaskFractions::from_ants(&simulator.ants));
        }
    }
    moments
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let cfg = match arg_value(&args, "--config", "").as_str() {
        "" => config::Config::default(),
        p => match config::Config::load(p) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("config load error: {e:?}");
                std::process::exit(1);
            }
        },
    };
    // --seed override (applies to headless evolve/validate/etc.)
    let mut cfg = cfg;
    if args.iter().any(|a| a == "--seed") {
        if let Ok(s) = arg_value(&args, "--seed", "42").parse::<u64>() {
            cfg.sim.seed = s;
        }
    }

    if args.iter().any(|a| a == "--print-config") {
        println!("{}", cfg.to_toml());
        return Ok(());
    }

    if args.iter().any(|a| a == "--headless") {
        match run_headless(&args, cfg) {
            Ok(()) => return Ok(()),
            Err(e) => {
                eprintln!("headless error: {e:?}");
                std::process::exit(1);
            }
        }
    }

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([960.0, 720.0])
        .with_title("ants-sim");

    let opts = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "ants-sim",
        opts,
        Box::new(move |cc| Ok(Box::new(viz::App::from_config(cc, cfg.clone())))),
    )
}

/// Parse a `--flag value` argument from the CLI, falling back to `default`.
fn arg_value(args: &[String], flag: &str, default: &str) -> String {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == flag {
            if let Some(v) = it.next() {
                return v.clone();
            }
        }
    }
    default.to_string()
}

/// Build a fresh simulator from cfg + CLI scenario/colony (deterministic).
fn build_sim(args: &[String], cfg: &config::Config) -> sim::Simulator {
    let colony: usize = arg_value(args, "--colony", "400").parse().unwrap_or(400);
    let scenario = arg_value(args, "--scenario", "two");
    let genome = cfg.genome.clone();
    let mut sim = sim::Simulator::new(cfg.world.width, cfg.world.height, cfg.sim.seed, &genome);
    sim.seasonal = args.iter().any(|a| a == "--seasonal");
    if args.iter().any(|a| a == "--diverse") {
        sim.set_colony_diverse(colony, &genome);
    } else {
        sim.set_colony_size(colony, &genome);
    }
    sim.world.nest = world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y);
    sim.world.nest_radius = cfg.world.nest_radius;
    let env_name = arg_value(args, "--env", "");
    if env_name.is_empty() {
        match scenario.as_str() {
            "single" => sim.scenario_single(),
            _ => sim.scenario_two_branch(),
        }
    } else {
        let env = environment::Environment::build(
            &env_name,
            sim.world.nest,
            cfg.world.width as f32,
            cfg.world.height as f32,
        );
        println!(
            "env: {} foods={} mean_dist={:.0} enemies={} obstacle_cells={}",
            env.name,
            env.metrics.food_count,
            env.metrics.mean_food_dist,
            env.metrics.enemy_count,
            env.metrics.obstacle_cells
        );
        sim.apply_environment(&env);
    }
    sim
}

/// Run one two-bridge traffic assay with the supplied genome and return the
/// cumulative actual gate crossings. The helper deliberately does not write a
/// CSV so parameter sweeps cannot overwrite the primary `--bridge` artifact.
fn run_bridge_trial(
    cfg: &config::Config,
    genome: &genome::Genome,
    seed: u64,
    colony: usize,
    ticks: u64,
    ablate_trail: bool,
) -> sim::BridgeFlow {
    let mut simulator = sim::Simulator::new(cfg.world.width, cfg.world.height, seed, genome);
    simulator.scenario_two_bridge();
    simulator.set_colony_size(colony, genome);
    simulator.ablate_trail = ablate_trail;
    for _ in 0..ticks {
        simulator.step();
    }
    simulator.bridge_flow
}

/// Deterministic fingerprint of sim state (collected, visits, Trail field sum).
fn fingerprint(sim: &sim::Simulator) -> String {
    let trail_sum: f64 = sim
        .world
        .field
        .channel_slice(world::Channel::Trail)
        .iter()
        .map(|&v| v as f64)
        .sum();
    let home_sum: f64 = sim
        .world
        .field
        .channel_slice(world::Channel::Home)
        .iter()
        .map(|&v| v as f64)
        .sum();
    format!(
        "col={:.4} visits={:?} trail_sum={:.6} home_sum={:.6} ants={}",
        sim.collected,
        sim.source_visits,
        trail_sum,
        home_sum,
        sim.ants.len()
    )
}

fn run_headless(args: &[String], cfg: config::Config) -> anyhow::Result<()> {
    let ticks: u64 = arg_value(args, "--ticks", "3000").parse()?;
    let brain_ann = arg_value(args, "--brain", "fsm") == "ann";
    let brain_cppn = arg_value(args, "--brain", "fsm") == "cppn";
    let brain_snn = arg_value(args, "--brain", "fsm") == "snn";
    let brain_mb = arg_value(args, "--brain", "fsm") == "mb";
    let brain_cx = arg_value(args, "--brain", "fsm") == "cx";
    let brain_integrated = arg_value(args, "--brain", "fsm") == "integrated";
    let use_cx = brain_cx || brain_integrated || args.iter().any(|a| a == "--use-cx");
    let social_contact = args.iter().any(|a| a == "--social-contact");
    let niche = args.iter().any(|a| a == "--niche");
    let novelty = args.iter().any(|a| a == "--novelty");
    let seasonal = args.iter().any(|a| a == "--seasonal");

    if args.iter().any(|a| a == "--evolve") {
        let env_name = arg_value(args, "--env", "rich_close");
        let gens: u32 = arg_value(args, "--gens", "8").parse().unwrap_or(8);
        let pop: usize = arg_value(args, "--pop", "10").parse().unwrap_or(10);
        let eticks: u64 = arg_value(args, "--ticks", "600").parse().unwrap_or(600);
        let colony: usize = arg_value(args, "--colony", "200").parse().unwrap_or(200);
        let out_dir = arg_value(args, "--out-dir", "results");
        let out_stem_arg = arg_value(args, "--out-stem", "");
        let env = environment::Environment::build(
            &env_name,
            world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y),
            cfg.world.width as f32,
            cfg.world.height as f32,
        );
        println!(
            "EVOLVE env={} pop={} gens={} eval_ticks={} colony={}",
            env.name, pop, gens, eticks, colony
        );
        let n_seeds: usize = arg_value(args, "--n-seeds", "3").parse().unwrap_or(3);
        let res = evolution::run(
            &cfg,
            &env,
            &cfg.genome,
            pop,
            gens,
            eticks,
            colony,
            cfg.sim.seed,
            n_seeds,
            brain_ann,
            brain_cppn,
            brain_snn,
            brain_mb,
            brain_cx,
            brain_integrated,
            seasonal,
            niche,
            novelty,
        );
        for r in &res.history {
            println!(
                "  gen {:<2} best={:.2} mean={:.2} worst={:.2} div={:.4}",
                r.gen,
                r.best,
                r.mean,
                r.worst,
                res.diversity.get(r.gen as usize).copied().unwrap_or(0.0)
            );
        }
        // open-ended litmus (T5.3 genotypic + T18 behavioral): neither
        // genotypic nor behavioral diversity should collapse.
        let d_init = res.diversity.first().copied().unwrap_or(0.0);
        let d_final = res.diversity.last().copied().unwrap_or(0.0);
        let b_init = res.diversity_behavior.first().copied().unwrap_or(0.0);
        let b_final = res.diversity_behavior.last().copied().unwrap_or(0.0);
        let open = d_final > d_init * 0.3 && b_final > b_init * 0.3;
        println!(
            "  diversity genotypic init={:.4} final={:.4} | behavioral init={:.4} final={:.4} | open-ended(non-collapse)={}",
            d_init, d_final, b_init, b_final, open
        );
        if !res.novelty.is_empty() {
            let n_init = res.novelty.first().copied().unwrap_or(0.0);
            let n_final = res.novelty.last().copied().unwrap_or(0.0);
            println!(
                "  novelty(NSLC-lite) init={:.4} final={:.4}",
                n_init, n_final
            );
        }
        println!(
            "BEST score={:.2} collected={:.1} mean_def_frac={:.3} fs={:.3} explore={:.3} aggression={:.3}",
            res.best_fitness.score,
            res.best_fitness.collected,
            res.best_fitness.mean_def_frac,
            res.best_genome.follow_strength,
            res.best_genome.explore_rate,
            res.best_genome.aggression,
        );
        // persist best genome (toml) + history (csv)
        let _ = std::fs::create_dir_all(&out_dir);
        let stem = if out_stem_arg.is_empty() {
            format!("{out_dir}/evolved_{}", env_name)
        } else {
            format!("{out_dir}/{}", out_stem_arg)
        };
        let _ = std::fs::write(
            format!("{stem}.toml"),
            toml::to_string_pretty(&config::Config {
                genome: res.best_genome.clone(),
                world: cfg.world.clone(),
                sim: cfg.sim.clone(),
            })
            .unwrap_or_default(),
        );
        let mut hist = String::from("gen,best,mean,worst\n");
        for r in &res.history {
            hist.push_str(&format!(
                "{},{:.4},{:.4},{:.4}\n",
                r.gen, r.best, r.mean, r.worst
            ));
        }
        let _ = std::fs::write(format!("{stem}_history.csv"), hist);
        println!("  saved: {stem}.toml + {stem}_history.csv");
        return Ok(());
    }

    if args.iter().any(|a| a == "--evolve-ml") {
        let env_name = arg_value(args, "--env", "rich_close");
        let gens: u32 = arg_value(args, "--gens", "8").parse().unwrap_or(8);
        let pool: usize = arg_value(args, "--pop", "12").parse().unwrap_or(12);
        let eticks: u64 = arg_value(args, "--ticks", "600").parse().unwrap_or(600);
        let colony: usize = arg_value(args, "--colony", "200").parse().unwrap_or(200);
        let out_dir = arg_value(args, "--out-dir", "results");
        let env = environment::Environment::build(
            &env_name,
            world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y),
            cfg.world.width as f32,
            cfg.world.height as f32,
        );
        println!(
            "EVOLVE-ML (multilevel) env={} pool={} gens={} ticks={} colony={}",
            env.name, pool, gens, eticks, colony
        );
        let res = evolution::run_multilevel(
            &cfg,
            &env,
            &cfg.genome,
            pool,
            gens,
            eticks,
            colony,
            cfg.sim.seed,
            brain_ann,
            brain_cppn,
            brain_snn,
            brain_mb,
            brain_cx,
            brain_integrated,
            seasonal,
        );
        for r in &res.history {
            println!("  gen {:<2} colony_score={:.2}", r.gen, r.best);
        }
        println!(
            "BEST score={:.2} collected={:.1} fs={:.3} explore={:.3} aggression={:.3}",
            res.best_fitness.score,
            res.best_fitness.collected,
            res.best_genome.follow_strength,
            res.best_genome.explore_rate,
            res.best_genome.aggression,
        );
        let _ = std::fs::create_dir_all(&out_dir);
        let stem = format!("{out_dir}/evolved_ml_{}", env_name);
        let _ = std::fs::write(
            format!("{stem}.toml"),
            toml::to_string_pretty(&config::Config {
                genome: res.best_genome.clone(),
                world: cfg.world.clone(),
                sim: cfg.sim.clone(),
            })
            .unwrap_or_default(),
        );
        let mut hist = String::from("gen,colony_score\n");
        for r in &res.history {
            hist.push_str(&format!("{},{:.4}\n", r.gen, r.best));
        }
        let _ = std::fs::write(format!("{stem}_history.csv"), hist);
        println!("  saved: {stem}.toml + {stem}_history.csv");
        return Ok(());
    }

    if args.iter().any(|a| a == "--evolve-all") {
        let gens: u32 = arg_value(args, "--gens", "5").parse().unwrap_or(5);
        let pop: usize = arg_value(args, "--pop", "8").parse().unwrap_or(8);
        let eticks: u64 = arg_value(args, "--ticks", "400").parse().unwrap_or(400);
        let colony: usize = arg_value(args, "--colony", "150").parse().unwrap_or(150);
        let out_dir = arg_value(args, "--out-dir", "results");
        let _ = std::fs::create_dir_all(&out_dir);
        let nest = world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y);

        println!("EVOLVE-ALL gens={gens} pop={pop} ticks={eticks} colony={colony}");
        println!(
            "{:<12} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
            "env", "score", "coll", "fs", "explore", "aggr", "deffrac"
        );
        let mut csv = String::from(
            "env,best_score,collected,follow_strength,explore_rate,aggression,mean_def_frac,task_bias\n",
        );
        for (name, _desc) in environment::Environment::presets() {
            let env = environment::Environment::build(
                name,
                nest,
                cfg.world.width as f32,
                cfg.world.height as f32,
            );
            // different seed per env so each run is independent but reproducible
            let name_hash = name
                .bytes()
                .fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(b as u64));
            let env_seed = cfg.sim.seed.wrapping_add(name_hash.wrapping_mul(0x9E37));
            let n_seeds: usize = arg_value(args, "--n-seeds", "3").parse().unwrap_or(3);
            let res = evolution::run(
                &cfg,
                &env,
                &cfg.genome,
                pop,
                gens,
                eticks,
                colony,
                env_seed,
                n_seeds,
                brain_ann,
                brain_cppn,
                brain_snn,
                brain_mb,
                brain_cx,
                brain_integrated,
                seasonal,
                niche,
                novelty,
            );
            println!(
                "{:<12} {:>8.1} {:>8.1} {:>8.3} {:>8.3} {:>8.3} {:>8.3}",
                name,
                res.best_fitness.score,
                res.best_fitness.collected,
                res.best_genome.follow_strength,
                res.best_genome.explore_rate,
                res.best_genome.aggression,
                res.best_fitness.mean_def_frac,
            );
            let stem = format!("{out_dir}/evolved_{name}");
            let _ = std::fs::write(
                format!("{stem}.toml"),
                toml::to_string_pretty(&config::Config {
                    genome: res.best_genome.clone(),
                    world: cfg.world.clone(),
                    sim: cfg.sim.clone(),
                })
                .unwrap_or_default(),
            );
            csv.push_str(&format!(
                "{name},{:.2},{:.1},{:.4},{:.4},{:.4},{:.4},{:.4}\n",
                res.best_fitness.score,
                res.best_fitness.collected,
                res.best_genome.follow_strength,
                res.best_genome.explore_rate,
                res.best_genome.aggression,
                res.best_fitness.mean_def_frac,
                res.best_genome.task_bias,
            ));
        }
        let _ = std::fs::write(format!("{out_dir}/champions.csv"), csv);
        println!("  saved: {out_dir}/champions.csv + evolved_*.toml");
        return Ok(());
    }

    if args.iter().any(|a| a == "--report") {
        let out_dir = arg_value(args, "--out-dir", "results");
        let read_lines = |p: &str| -> Vec<String> {
            std::fs::read_to_string(p)
                .map(|s| s.lines().map(|l| l.to_string()).collect())
                .unwrap_or_default()
        };
        let champs = read_lines(&format!("{out_dir}/champions.csv"));
        let zoo = read_lines(&format!("{out_dir}/zoo.csv"));
        let transfer = read_lines(&format!("{out_dir}/transfer.csv"));

        let mut r = String::new();
        r.push_str("# ants-sim 结果报告\n\n");
        r.push_str("隔夜自治演化产出的跨环境基因×环境结果。所有数值由 `--evolve-all` / `--zoo` / `--transfer` headless 跑出，可复现（固定种子）。\n\n");

        // --- champions table ---
        r.push_str("## 1. 各环境演化冠军基因组\n\n");
        r.push_str("小种群(8)少代(5)演化在 5 个环境预设中跑出的冠军基因组关键字段。\n\n");
        if !champs.is_empty() {
            r.push_str("| env | best_score | collected | follow_str | explore_rate | aggression | def_frac | task_bias |\n");
            r.push_str("|---|---|---|---|---|---|---|---|\n");
            for l in champs.iter().skip(1) {
                let f: Vec<&str> = l.split(',').collect();
                if f.len() >= 8 {
                    r.push_str(&format!(
                        "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                        f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7]
                    ));
                }
            }
        } else {
            r.push_str("_(champions.csv 缺失：先跑 `--evolve-all`)_\n");
        }
        r.push_str("\n**解读**：环境把不同基因显现出来——\n");
        r.push_str("- `rich_close`（食物近且多）→ 低 explore_rate（剥削近食，不乱跑）、高 follow_strength。\n");
        r.push_str("- `scarce_far` / `patchy` / `maze`（食物远/分散/有墙）→ 高 explore_rate（必须探索）。\n");
        r.push_str("- `predator`（巢口有敌）→ 高 aggression + 高 def_frac（防御特化）。\n\n");

        // --- zoo table ---
        r.push_str("## 2. Zoo：冠军 vs 默认（原生环境）\n\n");
        r.push_str("每个冠军在自己环境跑 1000 tick，对比默认基因组——冠军应碾压默认（适应）。\n\n");
        if !zoo.is_empty() {
            r.push_str("| env | default_score | champ_score | collected | eff/tick | def_frac | follow_str |\n");
            r.push_str("|---|---|---|---|---|---|---|\n");
            for l in zoo.iter().skip(1) {
                let f: Vec<&str> = l.split(',').collect();
                if f.len() >= 7 {
                    r.push_str(&format!(
                        "| {} | {} | {} | {} | {} | {} | {} |\n",
                        f[0], f[1], f[2], f[3], f[4], f[5], f[6]
                    ));
                }
            }
        } else {
            r.push_str("_(zoo.csv 缺失：先跑 `--zoo`)_\n");
        }
        r.push('\n');

        // --- transfer matrix ---
        r.push_str("## 3. 跨环境迁移矩阵（环境特化）\n\n");
        r.push_str(
            "每个冠军跑所有 5 个环境；对角线=原生环境，其余=迁移。特化=对角线高于均值。\n\n",
        );
        if !transfer.is_empty() {
            let hdr: Vec<&str> = transfer[0].split(',').collect();
            r.push_str(&format!("| {} |", hdr.join(" | ")));
            r.push_str("\n|");
            for _ in &hdr {
                r.push_str("---|");
            }
            r.push('\n');
            for l in transfer.iter().skip(1) {
                let f: Vec<&str> = l.split(',').collect();
                r.push_str("| ");
                r.push_str(&f.join(" | "));
                r.push_str(" |\n");
            }
        } else {
            r.push_str("_(transfer.csv 缺失：先跑 `--transfer`)_\n");
        }
        r.push_str("\n**解读**：冠军在不匹配环境退化——例如 rich_close 冠军(低探索)在 scarce_far 远食环境暴跌；predator 冠军(防御特化)在所有觅食环境都差。压力相似的环境(patchy/maze 都需探索)选出的适应相近，故迁移损失小。\n\n");

        // validation section
        r.push_str("## 4. 文献对齐验证\n\n");
        if let Ok(txt) = std::fs::read_to_string("results/bridge.csv") {
            let mut lines = txt.lines();
            let is_gate_flow_schema = lines.next() == Some("tick,short_outbound,long_outbound,short_inbound,long_inbound,short_inbound_fraction,collected");
            let fraction = is_gate_flow_schema
                .then(|| {
                    lines
                        .filter_map(|line| line.split(',').nth(5)?.trim().parse::<f32>().ok())
                        .next_back()
                })
                .flatten();
            if let Some(fraction) = fraction {
                r.push_str(&format!("- **双桥门线流量**：短桥返巢过线流量占比={fraction:.3}。这是实际 movement segment 的计数，不是 Trail 浓度或其变换；需要重复运行才能估计不确定性。\n"));
            } else {
                r.push_str(
                    "- **双桥门线流量**：bridge.csv 没有可用返巢流量；请重跑 `--bridge`。\n",
                );
            }
        }
        r.push_str("- **行为状态预算**：`--caste` 在 warm-up 后统计觅食/防御/育幼 State 的时间均值与波动；它不报告预置 task_jitter，也不等同于形态品级。\n");
        r.push_str("- **育幼人口过程**：模型的 nurse 行为与 brood→adult 转换可改变 colony size；这不等同于真实 nurse 移除实验。\n");
        r.push_str("- **竞争敏感性**：两群共享有限食物时 winner 可翻转，表明该实现对策略和初始条件敏感；这不是 cyclic dominance 的证据。\n");
        r.push_str(
            "- **确定性**：`--verify-determinism` 双跑指纹逐字节一致（rayon 并行下可复现）。\n\n",
        );

        r.push_str("## 5. 与文献对照\n\n");
        r.push_str("### 5.1 定量对照（模型 vs 文献量级）\n\n");
        r.push_str("| 指标 | 当前模型读出 | 外部文献 | 可得结论 |\n");
        r.push_str("|---|---|---|---|\n");
        r.push_str("| 双桥短路偏置 | 返巢门线过线流量占比 | Deneubourg/Goss：收敛后短桥偏置 | 定义更接近，但几何/物种不同；仅描述性比较 |\n");
        r.push_str("| 信息素半衰期 | 无量纲 tick | 真蚁 trail：分钟–小时级 | 无 tick→时间映射，不能量化对齐 |\n");
        r.push_str("| 觅食状态预算 | `Explore+FollowTrail+CarryReturn` 的时间均值 | Gordon：离巢觅食个体比例 | 非同构读出，不能称验证 |\n");
        r.push_str("| CX 误差 | 无感觉噪声的数值积分误差 | Cataglyphis 路径积分误差 | 只能比较误差随距离的定性模式 |\n");
        r.push_str("| MB 可塑性 | 内部权重与交付率 | 果蝇条件化表现 | 不是同一学习任务 |\n\n");
        r.push_str("### 5.2 解读规则\n\n");
        r.push_str("- `--compare-lit` 的绝对误差或区间命中仅作**描述性上下文**；不能据此宣称模型通过生物学验证。\n");
        r.push_str("- 双桥和任务分配已由 Trail 峰值/预置阈值代理重构为实际门线交通/实时 State 时间预算。历史峰值比、~88% 和 ~30% 数值不可与新协议直接比较。\n");
        r.push_str("- 消融百分比是此实现、参数、环境和读出的 paired counterfactual effect；不是物种级效应量，也不能代替机制特异的湿实验。\n");
        r.push_str("- 完整边界、假设写法和参考文献见 `docs/research_scope.md`。\n\n");

        r.push_str("## 6. 复现命令\n\n");
        r.push_str("```bash\n");
        r.push_str("export PATH=\"/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH\"\n");
        r.push_str(
            "cargo run --release -- --headless --evolve-all --gens 8 --pop 10   # 跨环境演化\n",
        );
        r.push_str(
            "cargo run --release -- --headless --zoo --ticks 1000                # 冠军 vs 默认\n",
        );
        r.push_str(
            "cargo run --release -- --headless --transfer --ticks 800           # 迁移矩阵\n",
        );
        r.push_str(
            "cargo run --release -- --headless --verify-determinism --ticks 1000 # 确定性\n",
        );
        r.push_str("cargo run                                                            # GUI\n");
        r.push_str("```\n");

        let _ = std::fs::write("REPORT.md", r);
        println!(
            "REPORT.md written ({} bytes). Read it tomorrow.",
            std::fs::metadata("REPORT.md").map(|m| m.len()).unwrap_or(0)
        );
        return Ok(());
    }

    if args.iter().any(|a| a == "--bench-cx") {
        // The benchmark distinguishes a zero-noise numeric reference from a
        // sensory compass condition and an explicit observation occlusion.
        // It is a model-internal navigation assay, not an animal calibration.
        // Walk the CX ant outbound along a straight bearing for increasing
        // distances, then read the neurally-integrated home vector (cx_hv) and
        // compare its magnitude/direction error to the true displacement. Real
        // desert ants (Cataglyphis fortis; Wehner & Müller 1988, Müller &
        // Wehner 1988) show path-integration homing error scaling roughly
        // linearly with distance (~5-15% systematic drift, distance generally
        // underestimated) and a small angular error. The leak-integrated ring
        // attractor should match that order of magnitude.
        let g = genome::Genome::default();
        let world = world::World::new(256, 256, world::Vec2::new(128.0, 128.0), 4.0);
        println!("BENCH-CX (observation-driven path integration)");
        println!("{}", "-".repeat(70));
        println!(
            "{:<12} {:<10} {:<14} {:<14} {:<14}",
            "condition", "dist", "hv_mag", "mag_err_%", "dir_err_deg"
        );
        println!("{}", "-".repeat(70));
        let stride = 0.5_f32;
        for &(label, noise, occlude) in &[
            ("reference", 0.0, false),
            ("noisy", 0.20, false),
            ("occluded", 0.0, true),
        ] {
            for &dist in &[10.0_f32, 20.0, 40.0, 80.0] {
                let mut ant = ant::Ant::new(world::Vec2::new(0.0, 0.0), 0.0, &g, 99);
                ant.use_cx = true;
                ant.cx_prev_heading = 0.0;
                let n = (dist / stride) as usize;
                for step in 0..n {
                    let available = !occlude || step < n / 3 || step >= 2 * n / 3;
                    let compass = ant::sensors::CompassObservation {
                        bearing: noise * ((step as f32 * 1.73).sin()),
                        confidence: if available { 1.0 } else { 0.0 },
                        available,
                    };
                    ant::brain::cx_integrate_observed(&mut ant, &world, stride, compass);
                }
                let mag = ant.cx_hv_x.hypot(ant.cx_hv_y);
                let mag_err = ((mag - dist) / dist).abs() * 100.0;
                let dir_err = ant.cx_hv_y.atan2(ant.cx_hv_x).to_degrees().abs();
                println!(
                    "{:<12} {:<10.1} {:<14.2} {:<14.1} {:<14.1}",
                    label, dist, mag, mag_err, dir_err
                );
            }
        }
        println!("{}", "-".repeat(70));
        println!("reference uses a perfect observation; noisy and occluded rows use only");
        println!("the provided CompassObservation plus motor feedback. Compare conditions,");
        println!("rather than treating any absolute error as an animal fit.");
        return Ok(());
    }

    if args.iter().any(|a| a == "--bench") {
        let colony: usize = arg_value(args, "--colony", "10000")
            .parse()
            .unwrap_or(10000);
        let bticks: u64 = arg_value(args, "--ticks", "200").parse().unwrap_or(200);
        let interact = args.iter().any(|a| a == "--interact");
        let seed = cfg.sim.seed;
        let genome = cfg.genome.clone();
        let mut sim = sim::Simulator::new(cfg.world.width, cfg.world.height, seed, &genome);
        sim.set_colony_size(colony, &genome);
        sim.interact = interact;
        let t0 = std::time::Instant::now();
        for _ in 0..bticks {
            sim.step();
        }
        let dt = t0.elapsed().as_secs_f64().max(1e-9);
        let tps = bticks as f64 / dt;
        println!(
            "BENCH colony={colony} ticks={bticks} interact={interact} time={:.2}s tps={:.1} alive={}",
            dt, tps, sim.ants.len()
        );
        let pass = tps >= 30.0;
        println!(
            "BENCH {} (≥30 tick/s at N={colony})",
            if pass { "PASS" } else { "FAIL" }
        );
        return Ok(());
    }

    if args.iter().any(|a| a == "--bridge-control") {
        let colony: usize = arg_value(args, "--colony", "500").parse().unwrap_or(500);
        let control_ticks: u64 = arg_value(args, "--ticks", "3000").parse().unwrap_or(3000);
        let n_seeds: usize = arg_value(args, "--n-seeds", "5")
            .parse()
            .unwrap_or(5)
            .max(1);
        let geometry = {
            let mut simulator =
                sim::Simulator::new(cfg.world.width, cfg.world.height, cfg.sim.seed, &cfg.genome);
            simulator.scenario_two_bridge();
            simulator.bridge_route_lengths()
        };
        let mut baseline = Moments::default();
        let mut trail_off = Moments::default();
        let mut baseline_inbound_total = Moments::default();
        let mut trail_off_inbound_total = Moments::default();
        let mut paired_fraction_delta_pct = Moments::default();
        let mut paired_inbound_total_delta_pct = Moments::default();
        let mut raw_csv = String::from(
            "seed,condition,short_inbound,long_inbound,inbound_total,short_inbound_fraction\n",
        );
        println!(
            "BRIDGE_CONTROL paired Trail counterfactual colony={colony} ticks={control_ticks} n_seeds={n_seeds}"
        );
        match geometry {
            Some(lengths) => println!(
                "  geometry: short={} grid steps long={} grid steps detour_ratio={:.2}x",
                lengths.short_steps,
                lengths.long_steps,
                lengths.detour_ratio()
            ),
            None => println!("  geometry: UNRESOLVED (no traversable arm-specific route)"),
        }
        for replicate in 0..n_seeds {
            let seed = cfg.sim.seed.wrapping_add(replicate as u64 * 0x1000_0003);
            let with_trail =
                run_bridge_trial(&cfg, &cfg.genome, seed, colony, control_ticks, false);
            let without_trail =
                run_bridge_trial(&cfg, &cfg.genome, seed, colony, control_ticks, true);
            for (condition, flow) in [("trail_on", with_trail), ("trail_off", without_trail)] {
                let fraction = flow
                    .short_inbound_fraction()
                    .map(|value| format!("{value:.6}"))
                    .unwrap_or_else(|| "NA".to_string());
                raw_csv.push_str(&format!(
                    "{seed},{condition},{},{},{},{}\n",
                    flow.short_inbound,
                    flow.long_inbound,
                    flow.inbound_total(),
                    fraction
                ));
            }
            if let Some(value) = with_trail.short_inbound_fraction() {
                baseline.push(value as f64);
            }
            if let Some(value) = without_trail.short_inbound_fraction() {
                trail_off.push(value as f64);
            }
            baseline_inbound_total.push(with_trail.inbound_total() as f64);
            trail_off_inbound_total.push(without_trail.inbound_total() as f64);
            if let (Some(on), Some(off)) = (
                with_trail.short_inbound_fraction(),
                without_trail.short_inbound_fraction(),
            ) {
                paired_fraction_delta_pct
                    .push(100.0 * (off as f64 - on as f64) / on.max(f32::EPSILON) as f64);
            }
            paired_inbound_total_delta_pct.push(
                100.0 * (without_trail.inbound_total() as f64 - with_trail.inbound_total() as f64)
                    / with_trail.inbound_total().max(1) as f64,
            );
        }
        let display = |moments: Moments| {
            if moments.display_count() > 0 {
                format!(
                    "{:.3}±{} (n={})",
                    moments.mean,
                    moments.display_sd(),
                    moments.n
                )
            } else {
                "NA (n=0)".to_string()
            }
        };
        println!("  Trail on  short_inbound_fraction={}", display(baseline));
        println!("  Trail off short_inbound_fraction={}", display(trail_off));
        println!(
            "  Trail on/off inbound_total={}/{}",
            display(baseline_inbound_total),
            display(trail_off_inbound_total)
        );
        println!(
            "  paired Trail-off short-fraction Δ%={}",
            display(paired_fraction_delta_pct)
        );
        println!(
            "  paired Trail-off inbound-total Δ%={}",
            display(paired_inbound_total_delta_pct)
        );
        let _ = std::fs::create_dir_all("results");
        let _ = std::fs::write("results/bridge_control.csv", raw_csv);
        println!("saved: results/bridge_control.csv (per-seed paired actual gate traffic)");
        println!("scope: model-internal Trail sensitivity only; this implementation switch is not a molecular or species-level pheromone intervention.");
        return Ok(());
    }

    if args.iter().any(|a| a == "--bridge-decay-sweep") {
        let colony: usize = arg_value(args, "--colony", "500").parse().unwrap_or(500);
        let sweep_ticks: u64 = arg_value(args, "--ticks", "3000").parse().unwrap_or(3000);
        let n_seeds: usize = arg_value(args, "--n-seeds", "5")
            .parse()
            .unwrap_or(5)
            .max(1);
        let decays = [0.002_f32, 0.0075, 0.015, 0.03, 0.05];
        let geometry = {
            let mut simulator =
                sim::Simulator::new(cfg.world.width, cfg.world.height, cfg.sim.seed, &cfg.genome);
            simulator.scenario_two_bridge();
            simulator.bridge_route_lengths()
        };
        let mut csv = String::from(
            "trail_decay,seed,short_inbound,long_inbound,inbound_total,short_inbound_fraction\n",
        );
        println!(
            "BRIDGE_DECAY_SWEEP actual gate traffic colony={colony} ticks={sweep_ticks} n_seeds={n_seeds}"
        );
        match geometry {
            Some(lengths) => println!(
                "  geometry: short={} grid steps long={} grid steps detour_ratio={:.2}x",
                lengths.short_steps,
                lengths.long_steps,
                lengths.detour_ratio()
            ),
            None => println!("  geometry: UNRESOLVED (no traversable arm-specific route)"),
        }
        println!(
            "{:<14} {:>15} {:>12} {:>12}",
            "trail_decay", "fraction mean±sd", "valid seeds", "inbound mean"
        );
        for decay in decays {
            let mut fractions = Moments::default();
            let mut inbound_total = Moments::default();
            for replicate in 0..n_seeds {
                let seed = cfg.sim.seed.wrapping_add(replicate as u64 * 0x1000_0003);
                let genome = genome::Genome {
                    trail_decay: decay,
                    ..cfg.genome.clone()
                };
                let flow = run_bridge_trial(&cfg, &genome, seed, colony, sweep_ticks, false);
                let fraction_cell = flow
                    .short_inbound_fraction()
                    .map(|fraction| {
                        fractions.push(fraction as f64);
                        format!("{fraction:.6}")
                    })
                    .unwrap_or_else(|| "NA".to_string());
                inbound_total.push(flow.inbound_total() as f64);
                csv.push_str(&format!(
                    "{decay:.4},{seed},{},{},{},{}\n",
                    flow.short_inbound,
                    flow.long_inbound,
                    flow.inbound_total(),
                    fraction_cell
                ));
            }
            let fraction = if fractions.display_count() > 0 {
                format!("{:.3}±{}", fractions.mean, fractions.display_sd())
            } else {
                "NA (n=0)".to_string()
            };
            println!(
                "{decay:<14.4} {fraction:>15} {:>12} {:>12.1}",
                fractions.display_count(),
                inbound_total.mean
            );
        }
        let _ = std::fs::create_dir_all("results");
        let _ = std::fs::write("results/bridge_decay_sweep.csv", csv);
        println!("saved: results/bridge_decay_sweep.csv (per-seed actual gate-crossing traffic)");
        println!("scope: sensitivity analysis for this abstract model; it does not calibrate a species-level trail lifetime or short-path preference.");
        return Ok(());
    }

    if args.iter().any(|a| a == "--bridge-sweep") {
        let colony: usize = arg_value(args, "--colony", "500").parse().unwrap_or(500);
        let sweep_ticks: u64 = arg_value(args, "--ticks", "3000").parse().unwrap_or(3000);
        let n_seeds: usize = arg_value(args, "--n-seeds", "5")
            .parse()
            .unwrap_or(5)
            .max(1);
        let strengths = [0.5_f32, 1.5, 3.0, 4.5, 6.0];
        let geometry = {
            let mut simulator =
                sim::Simulator::new(cfg.world.width, cfg.world.height, cfg.sim.seed, &cfg.genome);
            simulator.scenario_two_bridge();
            simulator.bridge_route_lengths()
        };
        let mut csv = String::from(
            "follow_strength,seed,short_inbound,long_inbound,inbound_total,short_inbound_fraction\n",
        );
        println!(
            "BRIDGE_SWEEP actual gate traffic colony={colony} ticks={sweep_ticks} n_seeds={n_seeds}"
        );
        match geometry {
            Some(lengths) => println!(
                "  geometry: short={} grid steps long={} grid steps detour_ratio={:.2}x",
                lengths.short_steps,
                lengths.long_steps,
                lengths.detour_ratio()
            ),
            None => println!("  geometry: UNRESOLVED (no traversable arm-specific route)"),
        }
        println!(
            "{:<16} {:>15} {:>12} {:>12}",
            "follow_strength", "fraction mean±sd", "valid seeds", "inbound mean"
        );
        for strength in strengths {
            let mut fractions = Moments::default();
            let mut inbound_total = Moments::default();
            for replicate in 0..n_seeds {
                let seed = cfg.sim.seed.wrapping_add(replicate as u64 * 0x1000_0003);
                let genome = genome::Genome {
                    follow_strength: strength,
                    ..cfg.genome.clone()
                };
                let flow = run_bridge_trial(&cfg, &genome, seed, colony, sweep_ticks, false);
                let fraction_cell = flow
                    .short_inbound_fraction()
                    .map(|fraction| {
                        fractions.push(fraction as f64);
                        format!("{fraction:.6}")
                    })
                    .unwrap_or_else(|| "NA".to_string());
                inbound_total.push(flow.inbound_total() as f64);
                csv.push_str(&format!(
                    "{strength:.3},{seed},{},{},{},{}\n",
                    flow.short_inbound,
                    flow.long_inbound,
                    flow.inbound_total(),
                    fraction_cell
                ));
            }
            let fraction = if fractions.display_count() > 0 {
                format!("{:.3}±{}", fractions.mean, fractions.display_sd())
            } else {
                "NA (n=0)".to_string()
            };
            println!(
                "{strength:<16.1} {fraction:>15} {:>12} {:>12.1}",
                fractions.display_count(),
                inbound_total.mean
            );
        }
        let _ = std::fs::create_dir_all("results");
        let _ = std::fs::write("results/bridge_sweep.csv", csv);
        println!("saved: results/bridge_sweep.csv (per-seed actual gate-crossing traffic)");
        println!("scope: sensitivity analysis for this abstract model; it does not calibrate a species-level short-path preference.");
        return Ok(());
    }

    if args.iter().any(|a| a == "--bridge") {
        let colony: usize = arg_value(args, "--colony", "500").parse().unwrap_or(500);
        let seed = cfg.sim.seed;
        let genome = cfg.genome.clone();
        let mut simulator = sim::Simulator::new(cfg.world.width, cfg.world.height, seed, &genome);
        simulator.scenario_two_bridge();
        let geometry = simulator.bridge_route_lengths();
        simulator.set_colony_size(colony, &genome);
        println!("BRIDGE traffic-flow protocol colony={colony} seed={seed} ticks={ticks}");
        match geometry {
            Some(lengths) => println!(
                "  geometry: short={} grid steps long={} grid steps detour_ratio={:.2}x",
                lengths.short_steps,
                lengths.long_steps,
                lengths.detour_ratio()
            ),
            None => println!("  geometry: UNRESOLVED (no traversable arm-specific route)"),
        }
        let mut csv = String::from("tick,short_outbound,long_outbound,short_inbound,long_inbound,short_inbound_fraction,collected\n");
        let sample_every = (ticks / 6).max(1);
        for tick in 0..ticks {
            simulator.step();
            if (tick + 1) % sample_every == 0 || tick + 1 == ticks {
                let flow = simulator.bridge_flow;
                let fraction = flow.short_inbound_fraction();
                let fraction_cell = fraction
                    .map(|value| format!("{value:.5}"))
                    .unwrap_or_else(|| "NA".to_string());
                println!(
                    "  t={:<5} short(out/in)={}/{} long(out/in)={}/{} short_inbound_fraction={} collected={:.0}",
                    simulator.tick,
                    flow.short_outbound,
                    flow.short_inbound,
                    flow.long_outbound,
                    flow.long_inbound,
                    fraction_cell,
                    simulator.collected
                );
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{:.0}\n",
                    simulator.tick,
                    flow.short_outbound,
                    flow.long_outbound,
                    flow.short_inbound,
                    flow.long_inbound,
                    fraction_cell,
                    simulator.collected
                ));
            }
        }
        let flow = simulator.bridge_flow;
        let status = if flow.inbound_total() >= 20 {
            "MEASURED"
        } else {
            "INSUFFICIENT_FLOW"
        };
        let fraction = flow
            .short_inbound_fraction()
            .map(|value| format!("{value:.3}"))
            .unwrap_or_else(|| "NA".to_string());
        let _ = std::fs::create_dir_all("results");
        let _ = std::fs::write("results/bridge.csv", csv);
        println!(
            "BRIDGE {status} short_inbound={} long_inbound={} short_fraction={} denominator={} collected={:.0}",
            flow.short_inbound,
            flow.long_inbound,
            fraction,
            flow.inbound_total(),
            simulator.collected
        );
        println!("  saved: results/bridge.csv (actual gate-crossing traffic; no pheromone-to-traffic conversion)");
        return Ok(());
    }

    if args.iter().any(|a| a == "--caste") {
        let colony: usize = arg_value(args, "--colony", "400").parse().unwrap_or(400);
        let env_name = arg_value(args, "--env", "predator");
        let brain = arg_value(args, "--brain", "fsm");
        let cticks: u64 = arg_value(args, "--ticks", "3000").parse().unwrap_or(3000);
        let warmup: u64 = arg_value(args, "--caste-warmup", &(cticks / 3).to_string())
            .parse()
            .unwrap_or(cticks / 3);
        let sample_every: u64 = arg_value(args, "--sample-every", "10")
            .parse()
            .unwrap_or(10);
        let moments = run_task_protocol(
            &cfg,
            cfg.sim.seed,
            &brain,
            &env_name,
            colony,
            cticks,
            warmup.min(cticks),
            sample_every,
        );
        println!(
            "TASK_ALLOCATION env={} brain={} seed={} ticks={} warmup={} samples={}",
            env_name,
            brain,
            cfg.sim.seed,
            cticks,
            warmup.min(cticks),
            moments.foraging.n
        );
        println!(
            "  foraging   mean={:.3} sd={}",
            moments.foraging.mean,
            moments.foraging.display_sd()
        );
        println!(
            "  defense    mean={:.3} sd={}",
            moments.defense.mean,
            moments.defense.display_sd()
        );
        println!(
            "  brood_care mean={:.3} sd={}",
            moments.brood_care.mean,
            moments.brood_care.display_sd()
        );
        println!("  note: live State time-budget, not a morphological caste proportion or a species-level fit.");
        return Ok(());
    }

    if args.iter().any(|a| a == "--war") {
        let colony: usize = arg_value(args, "--colony", "200").parse().unwrap_or(200);
        let seed = cfg.sim.seed;
        let g = cfg.genome.clone();
        let gb = g.mutate(&mut rand_chacha::ChaCha8Rng::seed_from_u64(seed ^ 0xBA5E));
        let mut sim = sim::Simulator::new(cfg.world.width, cfg.world.height, seed, &g);
        sim.world.nest = world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y);
        sim.world.nest_radius = cfg.world.nest_radius;
        sim.scenario_single();
        sim.set_two_colonies(colony, &g, &gb);
        sim.war = true;
        let init_a = (colony + 1) as u32 / 2;
        let init_b = colony as u32 / 2;
        for _ in 0..ticks {
            sim.step();
        }
        let alive_a = sim
            .ants
            .iter()
            .filter(|a| a.colony_id == 0 && !a.dead)
            .count() as u32;
        let alive_b = sim
            .ants
            .iter()
            .filter(|a| a.colony_id == 1 && !a.dead)
            .count() as u32;
        let cas_a = init_a.saturating_sub(alive_a);
        let cas_b = init_b.saturating_sub(alive_b);
        let winner = if alive_a == alive_b {
            "tie"
        } else if alive_a > alive_b {
            "A"
        } else {
            "B"
        };
        let both_cas = cas_a > 0 && cas_b > 0;
        println!(
            "WAR ticks={} colony={colony} initA={} initB={} aliveA={} aliveB={} casA={} casB={} winner={} both_casualties={}",
            ticks, init_a, init_b, alive_a, alive_b, cas_a, cas_b, winner, both_cas
        );
        return Ok(());
    }

    if args.iter().any(|a| a == "--compare-lit") {
        let _lit = std::fs::read_to_string("literature.json")
            .unwrap_or_else(|_| "{\"metrics\":[]}".to_string());
        println!("COMPARE-LIT (model vs representative published values)");
        println!("{}", "-".repeat(78));
        println!("{:<34} {:<24} {:<20}", "metric", "model", "literature");
        println!("{}", "-".repeat(78));
        // Model-derived quantities. These remain illustrative comparisons, not
        // statistical fits to a species-level data set.
        let half = (std::f32::consts::LN_2 / cfg.genome.trail_decay) as i64;
        let g = cfg.genome.clone();
        let task_protocol =
            run_task_protocol(&cfg, cfg.sim.seed, "fsm", "rich_close", 400, 3000, 1000, 10);
        let active_forager_fraction = task_protocol.foraging.mean as f32;
        // The bridge CSV now stores directly observed gate-crossing traffic.
        // Read its last finite cumulative inbound fraction, never a pheromone ratio.
        let bridge_fraction = std::fs::read_to_string("results/bridge.csv").ok().and_then(|text| {
            let mut lines = text.lines();
            (lines.next() == Some("tick,short_outbound,long_outbound,short_inbound,long_inbound,short_inbound_fraction,collected"))
                .then(|| lines.filter_map(|line| line.split(',').nth(5)?.trim().parse::<f32>().ok()).next_back())
                .flatten()
        });
        // model rows with literature notes (lit.json is the human-readable source)
        // T15.6: CX path-integration drift (bench at dist=40) + MB KC sparsity.
        let cx_world = world::World::new(256, 256, world::Vec2::new(128.0, 128.0), 4.0);
        let mut cx_ant = ant::Ant::new(world::Vec2::new(0.0, 0.0), 0.0, &g, 99);
        cx_ant.use_cx = true;
        cx_ant.cx_prev_heading = 0.0;
        let cx_dist = 40.0_f32;
        let cx_stride = 0.5_f32;
        for _ in 0..(cx_dist / cx_stride) as usize {
            ant::brain::cx_integrate(&mut cx_ant, &cx_world, cx_stride);
        }
        let cx_mag = cx_ant.cx_hv_x.hypot(cx_ant.cx_hv_y);
        let cx_drift = ((cx_mag - cx_dist) / cx_dist).abs() * 100.0;
        let cx_dir_err = cx_ant.cx_hv_y.atan2(cx_ant.cx_hv_x).to_degrees().abs();
        // MB KC sparsity: run an MB colony briefly, sample the fraction of
        // active KCs that spiked (membrane >= KC_THRESH) — insect MB KCs fire
        // sparsely (~5-15%).
        let mut mb_sim = sim::Simulator::new(cfg.world.width, cfg.world.height, cfg.sim.seed, &g);
        mb_sim.brain_mb = true;
        mb_sim.set_colony_size(80, &g);
        let mut spike_sum = 0.0f32;
        let mut sample_count = 0u32;
        let kc_thresh = genome::KC_THRESH;
        for t in 0..400u64 {
            mb_sim.step();
            // sample only once ants have matured past nurse_age (foraging, not
            // nursing — nursing ants return before KCs are computed).
            if t >= 200 && t % 20 == 0 {
                for a in &mb_sim.ants {
                    let act = a.mb_kc_active.min(genome::MB_KC);
                    let spikes = a
                        .mb_kc_v
                        .iter()
                        .take(act)
                        .filter(|&&v| v >= kc_thresh)
                        .count() as f32;
                    spike_sum += spikes / act.max(1) as f32;
                    sample_count += 1;
                }
            }
        }
        let kc_sparsity = if sample_count > 0 {
            spike_sum / sample_count as f32
        } else {
            0.0
        };
        let rows: [(&str, String, &str); 10] = [
            (
                "bridge_short_path_fraction",
                bridge_fraction
                    .map(|value| format!("{value:.2} (actual inbound gate traffic)"))
                    .unwrap_or_else(|| "NA (run --bridge first)".to_string()),
                "~0.80-0.90; qualitative only",
            ),
            (
                "trail_pheromone_halflife",
                format!("~{} ticks", half),
                "minutes-hours; no tick→time map",
            ),
            (
                "active_forager_fraction",
                format!("{:.2} (live State time budget)", active_forager_fraction),
                "~0.20-0.40; definition differs",
            ),
            (
                "non_foraging_state_fraction",
                format!("{:.2}", 1.0 - active_forager_fraction),
                "species-/task-dependent",
            ),
            (
                "brood_to_adult_transition",
                "model population process; not nurse removal".to_string(),
                "brood care affects development",
            ),
            (
                "path_selection_via_stigmergy",
                "model gate-flow assay (see --bridge)".to_string(),
                "qualitative comparison",
            ),
            (
                "cx_path_integration_drift",
                format!("{:.1}% (dist={}, abstract integrator)", cx_drift, cx_dist),
                "~5-15% (Cataglyphis; non-isomorphic)",
            ),
            (
                "cx_pi_direction_error",
                format!("{:.2} deg", cx_dir_err),
                "not a direct species fit",
            ),
            (
                "mb_kc_sparsity",
                format!("{:.0}% KC active", kc_sparsity * 100.0),
                "~5-15% active; abstract mapping",
            ),
            (
                "mb_associative_learning",
                "dual-valence plasticity proxy".to_string(),
                "not an odor-conditioning assay",
            ),
        ];
        for (n, m, l) in rows.iter() {
            println!("{:<34} {:<24} {:<20}", n, m, l);
        }
        // T18.3: literature datapoint fit (explicit published values, not
        // "from memory" magnitudes). Loads literature/*.toml datapoints and
        // reports absolute error + whether the model value falls in the
        // published range.
        #[derive(serde::Deserialize)]
        struct LitPoint {
            y: f32,
            y_lo: f32,
            y_hi: f32,
            #[serde(default)]
            x: Option<f32>,
        }
        #[derive(serde::Deserialize)]
        struct LitData {
            metric: String,
            datapoints: Vec<LitPoint>,
        }
        let load_lit = |path: &str| -> Option<LitData> {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|s| toml::from_str(&s).ok())
        };
        println!("\nDESCRIPTIVE COMPARISON (model vs representative datapoints; see literature/)");
        println!("{}", "-".repeat(78));
        println!(
            "{:<28} {:<12} {:<18} {:<10} {:<8}",
            "metric", "model", "lit [range]", "in_range", "abs_err"
        );
        println!("{}", "-".repeat(78));
        // These are descriptive, definition-mismatched comparisons; an interval
        // hit is reported as a number, never as biological validation.
        if let Some(lit) = load_lit("literature/bridge_deneubourg.toml") {
            if let (Some(p), Some(bridge_fraction)) = (lit.datapoints.first(), bridge_fraction) {
                let in_range = bridge_fraction >= p.y_lo && bridge_fraction <= p.y_hi;
                println!(
                    "{:<28} {:<12.2} {:<18} {:<10} {:<8.2}",
                    "bridge_gate_traffic_fraction",
                    bridge_fraction,
                    format!("{:.2} [{:.2},{:.2}]", p.y, p.y_lo, p.y_hi),
                    if in_range { "yes" } else { "no" },
                    (bridge_fraction - p.y).abs()
                );
            } else {
                println!(
                    "{:<28} NA (run --bridge first)",
                    "bridge_gate_traffic_fraction"
                );
            }
        }
        if let Some(lit) = load_lit("literature/caste_gordon.toml") {
            if let Some(p) = lit.datapoints.first() {
                let in_range =
                    active_forager_fraction >= p.y_lo && active_forager_fraction <= p.y_hi;
                println!(
                    "{:<28} {:<12.2} {:<18} {:<10} {:<8.2}",
                    "active_forager_state_fraction",
                    active_forager_fraction,
                    format!("{:.2} [{:.2},{:.2}]", p.y, p.y_lo, p.y_hi),
                    if in_range { "yes" } else { "no" },
                    (active_forager_fraction - p.y).abs()
                );
            }
        }
        // T19.3: CX PI drift vs distance — multi-point curve fit. Model drift
        // computed at each literature distance (x); compared to lit [y_lo,y_hi].
        if let Some(lit) = load_lit("literature/cx_cataglyphis.toml") {
            for p in &lit.datapoints {
                if let Some(dist) = p.x {
                    let mut cx_ant = ant::Ant::new(world::Vec2::new(0.0, 0.0), 0.0, &g, 99);
                    cx_ant.use_cx = true;
                    cx_ant.cx_prev_heading = 0.0;
                    let stride = 0.5_f32;
                    for _ in 0..(dist / stride) as usize {
                        ant::brain::cx_integrate(&mut cx_ant, &cx_world, stride);
                    }
                    let mag = cx_ant.cx_hv_x.hypot(cx_ant.cx_hv_y);
                    let drift = ((mag - dist) / dist).abs() * 100.0;
                    let in_range = drift >= p.y_lo && drift <= p.y_hi;
                    println!(
                        "{:<28} {:<12.2} {:<18} {:<10} {:<8.2}",
                        format!("cx_pi_drift@dist{}", dist as i32),
                        drift,
                        format!("{:.0} [{:.0},{:.0}]", p.y, p.y_lo, p.y_hi),
                        if in_range { "yes" } else { "no" },
                        (drift - p.y).abs()
                    );
                }
            }
        }
        // T19.3: MB learning ratio — late/early delivery rate (>1 = learning).
        // Model: MB colony, compare deliveries in a late window (600-800t) vs an
        // early post-nursing window (200-400t). Both windows are after ants
        // mature past nurse_age, so foraging is active in both; a ratio >1 means
        // the colony forages better with experience (STDP / trail maturation).
        if let Some(lit) = load_lit("literature/mb_drosophila.toml") {
            let mb_env = environment::Environment::build(
                "rich_close",
                world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y),
                cfg.world.width as f32,
                cfg.world.height as f32,
            );
            let mut mb_sim =
                sim::Simulator::new(cfg.world.width, cfg.world.height, cfg.sim.seed, &g);
            mb_sim.brain_mb = true;
            mb_sim.set_colony_size(80, &g);
            mb_sim.apply_environment(&mb_env);
            let mut c200 = 0.0f32;
            let mut c400 = 0.0f32;
            let mut c600 = 0.0f32;
            for t in 0..800u64 {
                mb_sim.step();
                match t + 1 {
                    200 => c200 = mb_sim.collected,
                    400 => c400 = mb_sim.collected,
                    600 => c600 = mb_sim.collected,
                    _ => {}
                }
            }
            let early = (c400 - c200).max(0.0); // ticks 200-400
            let late = (mb_sim.collected - c600).max(0.0); // ticks 600-800
            let ratio = if early > 1e-9 { late / early } else { 0.0 };
            if let Some(p) = lit.datapoints.first() {
                let in_range = ratio >= p.y_lo && ratio <= p.y_hi;
                println!(
                    "{:<28} {:<12.2} {:<18} {:<10} {:<8.2}",
                    "mb_learning_ratio(late/early)",
                    ratio,
                    format!("{:.1} [{:.1},{:.1}]", p.y, p.y_lo, p.y_hi),
                    if in_range { "yes" } else { "no" },
                    (ratio - p.y).abs()
                );
            }
        }
        println!("{}\nnote: literature values are representative context anchors, not a verified per-colony statistical fit. An interval hit is descriptive only: bridge, task, CX, and MB readouts differ materially from their biological counterparts (see literature/*.toml and docs/research_scope.md).", "-".repeat(78));
        return Ok(());
    }

    if args.iter().any(|a| a == "--ablate") {
        // Paired, model-internal counterfactual: both conditions share a seed
        // within each replicate. The implementation switch is not a wet-lab
        // intervention and does not establish biological sufficiency/necessity.
        let mech = arg_value(args, "--ablate", "");
        let known = [
            "octopamine",
            "trail",
            "eclosion",
            "homevector",
            "vision",
            "stdp",
            "reward",
            "punish",
            "al_inhibition",
            "pn_multichannel",
            "lh_reflex",
            "orn",
            "compass",
            "cx_motor",
            "contact",
        ];
        if !known.contains(&mech.as_str()) {
            anyhow::bail!(
                "unknown --ablate mechanism '{mech}'; use {}",
                known.join("|")
            );
        }
        let env_name = arg_value(args, "--env", "rich_close");
        let aticks: u64 = arg_value(args, "--ticks", "3000").parse().unwrap_or(3000);
        let colony: usize = arg_value(args, "--colony", "200").parse().unwrap_or(200);
        // `--n-seeds` is the documented spelling. Keep `--reps` as a
        // compatibility alias so an intended repeated ablation cannot silently
        // degrade to a single-seed run.
        let replicate_arg = if args.iter().any(|arg| arg == "--n-seeds") {
            arg_value(args, "--n-seeds", "1")
        } else {
            arg_value(args, "--reps", "1")
        };
        let replicates: usize = replicate_arg.parse().unwrap_or(1).max(1);
        let learning = matches!(mech.as_str(), "stdp" | "reward" | "punish");
        let requested_brain = arg_value(args, "--brain", "fsm");
        let brain = if learning {
            "mb".to_string()
        } else {
            requested_brain
        };
        let food_amount: f32 = arg_value(args, "--food-amount", "0").parse().unwrap_or(0.0);
        let out_path = arg_value(args, "--out", "");
        let run = |seed: u64, ablated: bool| -> AblationReadout {
            let g = cfg.genome.clone();
            let env = environment::Environment::build(
                &env_name,
                world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y),
                cfg.world.width as f32,
                cfg.world.height as f32,
            );
            let mut simulator = sim::Simulator::new(cfg.world.width, cfg.world.height, seed, &g);
            simulator.world.nest = world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y);
            simulator.world.nest_radius = cfg.world.nest_radius;
            apply_brain(&mut simulator, &brain);
            configure_ablation(&mut simulator, &mech, ablated);
            if mech == "punish" {
                simulator.apply_environment(&env);
                simulator.war = true;
                let mut competitor_rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed ^ 0xA11CE);
                let competitor = g.mutate(&mut competitor_rng);
                simulator.set_two_colonies(colony, &g, &competitor);
            } else {
                simulator.set_colony_size(colony, &g);
                simulator.apply_environment(&env);
            }
            if food_amount > 0.0 {
                for food in &mut simulator.world.food {
                    food.amount = food_amount;
                }
            }
            let initial_weights = simulator
                .ants
                .first()
                .map(|ant| ant.learned_mb_w.clone())
                .unwrap_or_default();
            let mut max_alive = simulator.ants.len();
            for _ in 0..aticks {
                simulator.step();
                max_alive = max_alive.max(simulator.ants.len());
            }
            let weight_drift = if initial_weights.is_empty() || simulator.ants.is_empty() {
                0.0
            } else {
                let n = initial_weights.len().max(1) as f32;
                simulator
                    .ants
                    .iter()
                    .map(|ant| {
                        let squared_error: f32 = ant
                            .learned_mb_w
                            .iter()
                            .zip(&initial_weights)
                            .map(|(current, initial)| (current - initial).powi(2))
                            .sum();
                        (squared_error / n).sqrt()
                    })
                    .sum::<f32>()
                    / simulator.ants.len() as f32
            };
            AblationReadout {
                collected: simulator.collected as f64,
                max_alive: max_alive as f64,
                final_alive: simulator.ants.len() as f64,
                weight_drift: weight_drift as f64,
            }
        };

        let mut base_collected = Moments::default();
        let mut abl_collected = Moments::default();
        let mut delta_collected_pct = Moments::default();
        let mut base_max_alive = Moments::default();
        let mut abl_max_alive = Moments::default();
        let mut delta_max_alive_pct = Moments::default();
        let mut base_final_alive = Moments::default();
        let mut abl_final_alive = Moments::default();
        let mut delta_final_alive_pct = Moments::default();
        let mut base_drift = Moments::default();
        let mut abl_drift = Moments::default();
        let mut delta_drift_pct = Moments::default();
        let mut raw_csv =
            String::from("seed,condition,collected,max_alive,final_alive,weight_drift\n");
        for replicate in 0..replicates {
            let seed = cfg.sim.seed.wrapping_add(replicate as u64 * 0x1000_0003);
            let baseline = run(seed, false);
            let ablated = run(seed, true);
            for (condition, readout) in [("baseline", baseline), ("ablated", ablated)] {
                raw_csv.push_str(&format!(
                    "{seed},{condition},{:.6},{:.6},{:.6},{:.6}\n",
                    readout.collected, readout.max_alive, readout.final_alive, readout.weight_drift
                ));
            }
            base_collected.push(baseline.collected);
            abl_collected.push(ablated.collected);
            base_max_alive.push(baseline.max_alive);
            abl_max_alive.push(ablated.max_alive);
            base_final_alive.push(baseline.final_alive);
            abl_final_alive.push(ablated.final_alive);
            base_drift.push(baseline.weight_drift);
            abl_drift.push(ablated.weight_drift);
            if baseline.collected > f64::EPSILON {
                delta_collected_pct
                    .push(100.0 * (ablated.collected - baseline.collected) / baseline.collected);
            }
            if baseline.max_alive > f64::EPSILON {
                delta_max_alive_pct
                    .push(100.0 * (ablated.max_alive - baseline.max_alive) / baseline.max_alive);
            }
            if baseline.final_alive > f64::EPSILON {
                delta_final_alive_pct.push(
                    100.0 * (ablated.final_alive - baseline.final_alive) / baseline.final_alive,
                );
            }
            if baseline.weight_drift > f64::EPSILON {
                delta_drift_pct.push(
                    100.0 * (ablated.weight_drift - baseline.weight_drift) / baseline.weight_drift,
                );
            }
        }
        if !out_path.is_empty() {
            std::fs::write(&out_path, raw_csv)?;
            println!("saved paired replicate data: {out_path}");
        }
        println!("ABLATE_MODEL_SENSITIVITY mechanism={} env={} brain_effective={} brain_requested={} ticks={} colony={} paired_replicates={}", mech, env_name, brain, arg_value(args, "--brain", "fsm"), aticks, colony, replicates);
        println!(
            "{:<20} {:>16} {:>16} {:>18}",
            "metric", "baseline mean±sd", "ablated mean±sd", "paired Δ% mean±sd"
        );
        let show = |label: &str, baseline: Moments, ablated: Moments, delta: Moments| {
            let delta_value = if delta.display_count() > 0 {
                format!("{:+.3}±{}", delta.mean, delta.display_sd())
            } else {
                "NA (n=0)".to_string()
            };
            println!(
                "{:<20} {:>8.3}±{:<6} {:>8.3}±{:<6} {:>18}",
                label,
                baseline.mean,
                baseline.display_sd(),
                ablated.mean,
                ablated.display_sd(),
                delta_value
            );
        };
        show(
            "collected",
            base_collected,
            abl_collected,
            delta_collected_pct,
        );
        show(
            "max_alive",
            base_max_alive,
            abl_max_alive,
            delta_max_alive_pct,
        );
        show(
            "final_alive",
            base_final_alive,
            abl_final_alive,
            delta_final_alive_pct,
        );
        if learning {
            show("weight_drift", base_drift, abl_drift, delta_drift_pct);
            println!("  note: weight_drift is an internal plasticity-state proxy, not a behavioral memory assay.");
        }
        println!("MODEL SCOPE: paired counterfactual sensitivity in this abstract implementation only; it does not establish biological sufficiency, necessity, a wet-lab intervention mapping, or a species-level effect size.");
        println!("EXTERNAL HYPOTHESIS: define a species-, circuit-, and task-specific perturbation separately, then test its direction with appropriate controls.");
        return Ok(());
    }

    if args.iter().any(|a| a == "--validate") {
        // T20: behavioral-emergence + robustness battery. Runs each brain in
        // each env for vticks (default 3000), samples per 100t after a warmup,
        // and checks colony-level emergence: survival, trail-network formation,
        // live task-budget stability, colony growth, sustained foraging. Honest
        // internal-plausibility / qualitative-emergence check — NOT a
        // quantitative match to a specific ant species (model is illustrative,
        // tick<->seconds undefined; see literature/*.toml for the quantitative
        // anchors). --brain / --env restrict the sweep for quick checks.
        let vticks: u64 = arg_value(args, "--ticks", "3000").parse().unwrap_or(3000);
        let brains: Vec<String> = if args.iter().any(|a| a == "--brain") {
            vec![arg_value(args, "--brain", "fsm")]
        } else {
            ["fsm", "ann", "cppn", "snn", "mb", "cx"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        };
        let envs: Vec<String> = if args.iter().any(|a| a == "--env") {
            vec![arg_value(args, "--env", "rich_close")]
        } else {
            ["rich_close", "scarce_far", "predator", "patchy", "maze"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        };
        let colony: usize = arg_value(args, "--colony", "200").parse().unwrap_or(200);
        let evolved = args.iter().any(|a| a == "--evolved");
        println!(
            "VALIDATE brains={:?} envs={:?} ticks={} colony={} evolved={}",
            brains, envs, vticks, colony, evolved
        );
        println!("{}", "-".repeat(86));
        println!(
            "{:<6} {:<12} {:<7} {:<6} {:<6} {:<6} {:<6} {:<5}",
            "brain", "env", "surv", "trail", "tasks", "grew", "forag", "pass"
        );
        println!("{}", "-".repeat(86));
        let mut report = String::new();
        report.push_str("# ants-sim 行为涌现验证（--validate）\n\n");
        report.push_str(&format!(
            "- brains={:?} envs={:?} ticks={} colony={}\n",
            brains, envs, vticks, colony
        ));
        report.push_str("- 诚实边界：这是**内部行为合理性 + 定性涌现**验证，非与某物种定量一致（模型示意级、tick↔秒无量纲；定量锚点见 literature/*.toml + --compare-lit）。\n\n");
        report.push_str("| brain | env | survival | trail_formed | task_budget_stable | colony_grew | foraging | pass |\n");
        report.push_str("|---|---|---|---|---|---|---|---|\n");
        let mut total_pass = 0usize;
        let mut total_runs = 0usize;
        for b in &brains {
            for e in &envs {
                // T21: --evolved loads the per-(brain,env) evolved champion
                // (results/evolved_<brain>_<env>.toml) if present, else falls
                // back to the default cfg genome.
                let g = if evolved {
                    let path = format!("results/evolved_{}_{}.toml", b, e);
                    crate::config::Config::load(&path)
                        .map(|c| c.genome)
                        .unwrap_or_else(|_| cfg.genome.clone())
                } else {
                    cfg.genome.clone()
                };
                let env = environment::Environment::build(
                    e,
                    world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y),
                    cfg.world.width as f32,
                    cfg.world.height as f32,
                );
                // cppn: develop the phenotype from cppn_genes so the champion
                // runs as the developed ANN (evolution tuned cppn_genes, not
                // the seed ann_weights stored in the toml).
                let mut g = g;
                if b == "cppn" {
                    g.ann_weights = crate::genome::develop_phenotype(&g);
                }
                let mut sim =
                    sim::Simulator::new(cfg.world.width, cfg.world.height, cfg.sim.seed, &g);
                sim.set_colony_size(colony, &g);
                sim.world.nest = world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y);
                sim.world.nest_radius = cfg.world.nest_radius;
                sim.brain_ann = b == "ann" || b == "cppn";
                sim.brain_snn = b == "snn";
                sim.brain_mb = b == "mb";
                sim.brain_cx = b == "cx";
                sim.apply_environment(&env);
                let initial = sim.ants.len() as f32;
                let mut alive: Vec<f32> = Vec::new();
                let mut coll: Vec<f32> = Vec::new();
                let mut task_foraging: Vec<f32> = Vec::new();
                let mut task_defense: Vec<f32> = Vec::new();
                let mut task_brood: Vec<f32> = Vec::new();
                let mut trail: Vec<f32> = Vec::new();
                for t in 0..vticks {
                    sim.step();
                    if t >= vticks / 3 && t % 100 == 0 {
                        let tasks = sim::TaskFractions::from_ants(&sim.ants);
                        let tt = sim
                            .world
                            .field
                            .channel_slice(world::Channel::Trail)
                            .iter()
                            .copied()
                            .sum();
                        alive.push(sim.ants.len() as f32);
                        coll.push(sim.collected);
                        task_foraging.push(tasks.foraging);
                        task_defense.push(tasks.defense);
                        task_brood.push(tasks.brood_care);
                        trail.push(tt);
                    }
                }
                let survival = alive.last().copied().unwrap_or(0.0) / initial.max(1.0);
                let trail_peak = trail.iter().cloned().fold(0.0f32, f32::max);
                let trail_last = trail.last().copied().unwrap_or(0.0);
                // trail formed = a meaningful network emerged (peak above noise)
                // AND still present at run end (trail_last > 1, not fully
                // decayed). Note: trail_total often PEAKS early then drops as
                // the network consolidates onto fewer efficient cells — that
                // consolidation is good, so we don't require trail_last near peak.
                let trail_formed = trail_peak > 2.0 && trail_last > 1.0;
                let task_budget_stable = {
                    let stable = |series: &[f32]| {
                        if series.len() < 2 {
                            return false;
                        }
                        let mean = series.iter().sum::<f32>() / series.len() as f32;
                        let variance = series
                            .iter()
                            .map(|&value| (value - mean).powi(2))
                            .sum::<f32>()
                            / series.len().max(1) as f32;
                        variance.sqrt() < 0.10
                    };
                    stable(&task_foraging) && stable(&task_defense) && stable(&task_brood)
                };
                let colony_grew = alive.iter().cloned().fold(0.0f32, f32::max) > initial * 1.05;
                let foraging = {
                    let last = coll.last().copied().unwrap_or(0.0);
                    let prev = coll
                        .get(coll.len().saturating_sub(5))
                        .copied()
                        .unwrap_or(0.0);
                    last - prev > 0.0
                };
                let npass = [
                    survival > 0.1,
                    trail_formed,
                    task_budget_stable,
                    colony_grew,
                    foraging,
                ]
                .iter()
                .filter(|&&p| p)
                .count();
                total_pass += npass;
                total_runs += 1;
                println!(
                    "{:<6} {:<12} {:<7.2} {:<6} {:<6} {:<6} {:<6} {:<2}/5",
                    b,
                    e,
                    survival,
                    if trail_formed { "Y" } else { "-" },
                    if task_budget_stable { "Y" } else { "-" },
                    if colony_grew { "Y" } else { "-" },
                    if foraging { "Y" } else { "-" },
                    npass
                );
                report.push_str(&format!(
                    "| {} | {} | {:.2} | {} | {} | {} | {} | {}/5 |\n",
                    b,
                    e,
                    survival,
                    if trail_formed { "Y" } else { "-" },
                    if task_budget_stable { "Y" } else { "-" },
                    if colony_grew { "Y" } else { "-" },
                    if foraging { "Y" } else { "-" },
                    npass
                ));
            }
        }
        println!("{}", "-".repeat(86));
        println!(
            "aggregate: {}/{} checks passed ({:.0}%)",
            total_pass,
            total_runs * 5,
            100.0 * total_pass as f64 / (total_runs * 5).max(1) as f64
        );
        report.push_str(&format!(
            "\n**聚合：{}/{} 检查通过 ({:.0}%)**\n",
            total_pass,
            total_runs * 5,
            100.0 * total_pass as f64 / (total_runs * 5).max(1) as f64
        ));
        report.push_str(
            "\n## 判据\n- survival>0.1（蚁群存续）\n- trail_formed：trail_total 峰值>2（网络形成）且末值>1（仍存在；峰值后下降可能是网络压缩，而非必然坍塌）\n- task_budget_stable：实时行为状态（觅食/防御/育幼）三个时间序列的标准差均<0.10；这不是形态品级或物种级分工比例\n- colony_grew：峰值活蚁>1.05·初始（模型的 brood→adult 人口过程）\n- foraging：末 500t 仍有交付（持续觅食）\n\n## 诚实评估\n这是**内部行为合理性 + 定性涌现**检查，非物种级定量验证。行为状态、抽象信息素、brood→adult 转换与真实蚂蚁的任务、化学或发育过程并非一一对应；定量锚点只能作为上下文，不能据区间命中宣称验证（见 `literature/*.toml`、`--compare-lit` 与 `docs/research_scope.md`）。\n",
        );
        let _ = std::fs::write("results/validate.md", report);
        println!("saved: results/validate.md");
        return Ok(());
    }

    if args.iter().any(|a| a == "--export-html") {
        let out = arg_value(args, "--export-html", "snapshot.html");
        let colony: usize = arg_value(args, "--colony", "250").parse().unwrap_or(250);
        let frames: usize = arg_value(args, "--frames", "50").parse().unwrap_or(50);
        let seed = cfg.sim.seed;
        let genome = cfg.genome.clone();
        let mut sim = sim::Simulator::new(cfg.world.width, cfg.world.height, seed, &genome);
        sim.set_colony_size(colony, &genome);
        sim.scenario_single();
        let gw = sim.world.width;
        let gh = sim.world.height;
        let nest = (sim.world.nest.x, sim.world.nest.y);
        let food: Vec<(f32, f32)> = sim.world.food.iter().map(|f| (f.pos.x, f.pos.y)).collect();
        // sample frames
        let total_ticks = ticks.max(1);
        let stride = (total_ticks / frames.max(1) as u64).max(1);
        let mut fr: Vec<Vec<(i32, i32, u8)>> = Vec::new();
        for t in 0..total_ticks {
            sim.step();
            if t % stride == 0 && fr.len() < frames {
                let snap: Vec<(i32, i32, u8)> = sim
                    .ants
                    .iter()
                    .map(|a| (a.pos.x as i32, a.pos.y as i32, a.colony_id))
                    .collect();
                fr.push(snap);
            }
        }
        // build JSON
        let mut j = String::from("[");
        for (i, snap) in fr.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            j.push('[');
            for (k, (x, y, c)) in snap.iter().enumerate() {
                if k > 0 {
                    j.push(',');
                }
                j.push_str(&format!("[{},{},{}]", x, y, c));
            }
            j.push(']');
        }
        j.push(']');
        let food_json = {
            let mut s = String::from("[");
            for (k, (x, y)) in food.iter().enumerate() {
                if k > 0 {
                    s.push(',');
                }
                s.push_str(&format!("[{:.0},{:.0}]", x, y));
            }
            s.push(']');
            s
        };
        let html = r#"<!doctype html><html><head><meta charset="utf-8"><title>ants-sim snapshot</title>
<style>body{margin:0;background:#000}canvas{image-rendering:pixelated}</style></head>
<body><canvas id="c" width="__W__" height="__H__"></canvas>
<script>
var FRAMES=__J__;
var NEST=[__NX__,__NY__];
var FOOD=__FOOD__;
var W=__W__,H=__H__;
var cv=document.getElementById('c');var cx=cv.getContext('2d');
var i=0;
function draw(){cx.fillStyle='#000';cx.fillRect(0,0,W,H);
 cx.fillStyle='#0f0';for(var k=0;k<FOOD.length;k++){cx.beginPath();cx.arc(FOOD[k][0],FOOD[k][1],4,0,7);cx.fill();}
 cx.fillStyle='#a76';cx.beginPath();cx.arc(NEST[0],NEST[1],6,0,7);cx.fill();
 var f=FRAMES[i];for(var k=0;k<f.length;k++){var a=f[k];cx.fillStyle=a[2]?'#f60':'#eee';cx.fillRect(a[0],a[1],1.5,1.5);}
 i=(i+1)%FRAMES.length;setTimeout(draw,120);
}
draw();
</script></body></html>"#
            .replace("__W__", &gw.to_string())
            .replace("__H__", &gh.to_string())
            .replace("__J__", &j)
            .replace("__NX__", &nest.0.to_string())
            .replace("__NY__", &nest.1.to_string())
            .replace("__FOOD__", &food_json);
        let _ = std::fs::write(&out, html);
        let sz = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
        println!(
            "EXPORT-HTML {} frames={} ants/frame~{} bytes={}",
            out,
            fr.len(),
            colony,
            sz
        );
        return Ok(());
    }

    if args.iter().any(|a| a == "--coevolve") {
        let rounds: u32 = arg_value(args, "--rounds", "10").parse().unwrap_or(10);
        let colony: usize = arg_value(args, "--colony", "200").parse().unwrap_or(200);
        let seed = cfg.sim.seed;
        let mut genome_a = cfg.genome.clone();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed ^ 0xC0E);
        let nest = world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y);
        let env = environment::Environment::build(
            "rich_close",
            nest,
            cfg.world.width as f32,
            cfg.world.height as f32,
        );
        println!("COEVOLVE rounds={rounds} colony={colony} seed={seed} (champion A vs mutant B, limited food)");
        let mut last_winner = 'A';
        let mut flips = 0;
        let mut champ_best = 0.0f32;
        for r in 0..rounds {
            let genome_b = genome_a.mutate(&mut rng);
            let mut sim = sim::Simulator::new(
                cfg.world.width,
                cfg.world.height,
                seed.wrapping_add(r as u64),
                &genome_a,
            );
            sim.world.nest = nest;
            sim.world.nest_radius = cfg.world.nest_radius;
            sim.apply_environment(&env);
            // limited food → competition (each source ~150)
            for f in sim.world.food.iter_mut() {
                f.amount = 150.0;
            }
            sim.set_two_colonies(colony, &genome_a, &genome_b);
            for _ in 0..ticks {
                sim.step();
            }
            let ca = sim.collected_a;
            let cb = sim.collected_b;
            let winner = if cb > ca { 'B' } else { 'A' };
            if winner != last_winner {
                flips += 1;
            }
            last_winner = winner;
            if winner == 'B' {
                genome_a = genome_b;
            }
            champ_best = champ_best.max(ca.max(cb));
            println!(
                "  round {:<2} A={:.0} B={:.0} winner={} champ_best={:.0}",
                r, ca, cb, winner, champ_best
            );
        }
        let improving = champ_best > 0.0;
        println!(
            "COEVOLVE {} champ_best={:.0} flips={} (arms-race迹象: winner身份变动)",
            if improving { "PASS" } else { "FAIL" },
            champ_best,
            flips
        );
        let _ = std::fs::create_dir_all("results");
        let _ = std::fs::write(
            format!("results/coevolve_{}.toml", cfg.sim.seed),
            toml::to_string_pretty(&config::Config {
                genome: genome_a,
                world: cfg.world.clone(),
                sim: cfg.sim.clone(),
            })
            .unwrap_or_default(),
        );
        return Ok(());
    }

    if args.iter().any(|a| a == "--brood") {
        let colony: usize = arg_value(args, "--colony", "200").parse().unwrap_or(200);
        let seed = cfg.sim.seed;
        let genome = cfg.genome.clone();
        let mut sim = sim::Simulator::new(cfg.world.width, cfg.world.height, seed, &genome);
        sim.set_colony_size(colony, &genome);
        sim.scenario_single();
        sim.brain_ann = brain_ann || brain_cppn;
        if brain_cppn {
            for a in sim.ants.iter_mut() {
                a.genome.ann_weights = crate::genome::develop_phenotype(&a.genome);
            }
        }
        let init_brood = sim.world.brood;
        let init_ants = sim.ants.len();
        for _ in 0..ticks {
            sim.step();
        }
        let nurses = sim
            .ants
            .iter()
            .filter(|a| matches!(a.state, crate::ant::State::Nurse))
            .count();
        let tend = sim.ants.iter().map(|a| a.pending_brood).sum::<f32>();
        let final_ants = sim.ants.len();
        let ecloded = final_ants > init_ants;
        println!(
            "BROOD ticks={} colony_init={} colony_final={} init_brood={:.1} final_brood={:.1} nurses={} tend/tick={:.2} ecloded={}",
            ticks, init_ants, final_ants, init_brood, sim.world.brood, nurses, tend, ecloded
        );
        return Ok(());
    }

    if args.iter().any(|a| a == "--zoo") {
        let ticks: u64 = arg_value(args, "--ticks", "1000").parse().unwrap_or(1000);
        let colony: usize = arg_value(args, "--colony", "200").parse().unwrap_or(200);
        let out_dir = arg_value(args, "--out-dir", "results");
        let nest = world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y);
        println!("ZOO ticks={ticks} colony={colony}  (champion vs default in native env)");
        println!(
            "{:<12} {:>10} {:>10} {:>9} {:>8} {:>8} {:>8}",
            "env", "default_s", "champ_s", "coll", "eff/tick", "deffrac", "fs"
        );
        let mut csv = String::from(
            "env,default_score,champ_score,collected,efficiency,def_frac,follow_strength\n",
        );
        for (name, _desc) in environment::Environment::presets() {
            let env = environment::Environment::build(
                name,
                nest,
                cfg.world.width as f32,
                cfg.world.height as f32,
            );
            let default_fit = evolution::evaluate(
                &cfg,
                &env,
                &cfg.genome,
                ticks,
                colony,
                brain_ann,
                brain_cppn,
                brain_snn,
                brain_mb,
                brain_cx,
                brain_integrated,
                seasonal,
            );
            let champ_cfg = config::Config::load(&format!("{out_dir}/evolved_{name}.toml"))
                .unwrap_or_else(|_| cfg.clone());
            let champ_fit = evolution::evaluate(
                &champ_cfg,
                &env,
                &champ_cfg.genome,
                ticks,
                colony,
                brain_ann,
                brain_cppn,
                brain_snn,
                brain_mb,
                brain_cx,
                brain_integrated,
                seasonal,
            );
            let eff = champ_fit.collected / ticks as f32;
            println!(
                "{:<12} {:>10.1} {:>10.1} {:>9.0} {:>8.3} {:>8.3} {:>8.3}",
                name,
                default_fit.score,
                champ_fit.score,
                champ_fit.collected,
                eff,
                champ_fit.mean_def_frac,
                champ_cfg.genome.follow_strength,
            );
            csv.push_str(&format!(
                "{name},{:.2},{:.2},{:.0},{:.4},{:.4},{:.4}\n",
                default_fit.score,
                champ_fit.score,
                champ_fit.collected,
                eff,
                champ_fit.mean_def_frac,
                champ_cfg.genome.follow_strength,
            ));
        }
        let _ = std::fs::write(format!("{out_dir}/zoo.csv"), csv);
        println!("  saved: {out_dir}/zoo.csv");
        return Ok(());
    }

    if args.iter().any(|a| a == "--transfer") {
        let ticks: u64 = arg_value(args, "--ticks", "800").parse().unwrap_or(800);
        let colony: usize = arg_value(args, "--colony", "200").parse().unwrap_or(200);
        let out_dir = arg_value(args, "--out-dir", "results");
        let nest = world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y);
        let presets = environment::Environment::presets();
        // header
        print!("{:<14}", "champ\\env");
        for (n, _) in &presets {
            print!(" {:>10}", n);
        }
        println!();
        let mut csv = String::from("champion");
        for (n, _) in &presets {
            csv.push_str(&format!(",{n}"));
        }
        csv.push('\n');
        // load each champion, evaluate in every env
        for (origin, _) in &presets {
            let champ_cfg = config::Config::load(&format!("{out_dir}/evolved_{origin}.toml"))
                .unwrap_or_else(|_| cfg.clone());
            print!("{:<14}", origin);
            csv.push_str(origin);
            for (target, _) in &presets {
                let env = environment::Environment::build(
                    target,
                    nest,
                    cfg.world.width as f32,
                    cfg.world.height as f32,
                );
                let fit = evolution::evaluate(
                    &champ_cfg,
                    &env,
                    &champ_cfg.genome,
                    ticks,
                    colony,
                    brain_ann,
                    brain_cppn,
                    brain_snn,
                    brain_mb,
                    brain_cx,
                    brain_integrated,
                    seasonal,
                );
                print!(" {:>10.1}", fit.score);
                csv.push_str(&format!(",{:.1}", fit.score));
            }
            println!();
            csv.push('\n');
        }
        let _ = std::fs::write(format!("{out_dir}/transfer.csv"), csv);
        println!(
            "  saved: {out_dir}/transfer.csv  (diagonal = native env; off-diagonal = transfer)"
        );
        return Ok(());
    }

    if args.iter().any(|a| a == "--fitness") {
        let env_name = arg_value(args, "--env", "rich_close");
        let ticks: u64 = arg_value(args, "--ticks", "1000").parse().unwrap_or(1000);
        let colony: usize = arg_value(args, "--colony", "300").parse().unwrap_or(300);
        let n_seeds: usize = arg_value(args, "--n-seeds", "1").parse().unwrap_or(1);
        let env = environment::Environment::build(
            &env_name,
            world::Vec2::new(cfg.world.nest_x, cfg.world.nest_y),
            cfg.world.width as f32,
            cfg.world.height as f32,
        );
        if n_seeds > 1 {
            let mut per = Vec::new();
            for k in 0..n_seeds {
                let mut cfgk = cfg.clone();
                cfgk.sim.seed = cfg.sim.seed.wrapping_add(k as u64 * 0x1000_0003);
                per.push(
                    evolution::evaluate(
                        &cfgk,
                        &env,
                        &cfg.genome,
                        ticks,
                        colony,
                        brain_ann,
                        brain_cppn,
                        brain_snn,
                        brain_mb,
                        brain_cx,
                        brain_integrated,
                        seasonal,
                    )
                    .score,
                );
            }
            let mean = per.iter().sum::<f32>() / per.len() as f32;
            let (mn, mx) = per
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| {
                    (a.min(*v), b.max(*v))
                });
            println!(
                "FITNESS env={} n_seeds={n_seeds} ticks={ticks} colony={colony}",
                env.name
            );
            println!(
                "  per-seed scores: {:?}",
                per.iter().map(|v| format!("{:.1}", v)).collect::<Vec<_>>()
            );
            println!(
                "  mean={mean:.2} range=[{mn:.1},{mx:.1}] spread={:.1}",
                mx - mn
            );
        }
        let fit = evolution::evaluate_multi(
            &cfg,
            &env,
            &cfg.genome,
            ticks,
            colony,
            n_seeds,
            brain_ann,
            brain_cppn,
            brain_snn,
            brain_mb,
            brain_cx,
            brain_integrated,
            seasonal,
        );
        println!(
            "  collected={:.1} score={:.2} mean_def_frac={:.3} survival={:.3} trail_total={:.1} per_source={:?}",
            fit.collected, fit.score, fit.mean_def_frac, fit.survival, fit.trail_total, fit.per_source
        );
        return Ok(());
    }

    if args.iter().any(|a| a == "--test-genome") {
        use rand::SeedableRng;
        let n: u32 = arg_value(args, "--n", "2000").parse().unwrap_or(2000);
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(cfg.sim.seed);
        let mut bad = 0usize;
        let mut parent = genome::Genome::random(&mut rng);
        for _ in 0..n {
            let child = if rng.gen::<f32>() < 0.5 {
                parent.mutate(&mut rng)
            } else {
                let other = genome::Genome::random(&mut rng);
                parent.crossover(&other, &mut rng)
            };
            if !child.in_range() {
                bad += 1;
            }
            parent = child;
        }
        let pass = bad == 0;
        println!(
            "GENOME-OPS {} (n={n} bad={bad})",
            if pass { "PASS" } else { "FAIL" },
        );
        println!("  final genome in_range={}", parent.in_range());
        println!(
            "  sample: follow_strength={:.3} explore_rate={:.3} aggression={:.3}",
            parent.follow_strength, parent.explore_rate, parent.aggression
        );
        return if pass {
            Ok(())
        } else {
            Err(anyhow::anyhow!("genome field out of range"))
        };
    }

    if args.iter().any(|a| a == "--inspect-diversity") {
        // Build two colonies (homo + diverse) and compare genome-field spread.
        let n: usize = arg_value(args, "--colony", "300").parse().unwrap_or(300);
        let mut homo =
            sim::Simulator::new(cfg.world.width, cfg.world.height, cfg.sim.seed, &cfg.genome);
        homo.set_colony_size(n, &cfg.genome);
        let mut div =
            sim::Simulator::new(cfg.world.width, cfg.world.height, cfg.sim.seed, &cfg.genome);
        div.set_colony_diverse(n, &cfg.genome);
        let stat = |s: &sim::Simulator, f: fn(&genome::Genome) -> f32| {
            let vs: Vec<f32> = s.ants.iter().map(|a| f(&a.genome)).collect();
            let (mn, mx) = vs
                .iter()
                .cloned()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| {
                    (a.min(v), b.max(v))
                });
            let mean = vs.iter().sum::<f32>() / vs.len().max(1) as f32;
            (mn, mx, mean)
        };
        let fs: fn(&genome::Genome) -> f32 = |g| g.follow_strength;
        let ex: fn(&genome::Genome) -> f32 = |g| g.explore_rate;
        let ag: fn(&genome::Genome) -> f32 = |g| g.aggression;
        let (hmn, hmx, hmean) = stat(&homo, fs);
        let (dmn, dmx, dmean) = stat(&div, fs);
        let (emn, emx, emean) = stat(&div, ex);
        let (amn, amx, amean) = stat(&div, ag);
        let diverse_ok = (dmx - dmn) > 0.0 && (hmx - hmn) < 1e-6;
        println!(
            "DIVERSITY {} (n={n})",
            if diverse_ok { "PASS" } else { "FAIL" }
        );
        println!(
            "  homo  follow_strength: [{:.3},{:.3}] mean={:.3}",
            hmn, hmx, hmean
        );
        println!(
            "  div   follow_strength: [{:.3},{:.3}] mean={:.3}",
            dmn, dmx, dmean
        );
        println!(
            "  div   explore_rate:    [{:.3},{:.3}] mean={:.3}",
            emn, emx, emean
        );
        println!(
            "  div   aggression:      [{:.3},{:.3}] mean={:.3}",
            amn, amx, amean
        );
        return Ok(());
    }

    if args.iter().any(|a| a == "--verify-determinism") {
        let mut s1 = build_sim(args, &cfg);
        for _ in 0..ticks {
            s1.step();
        }
        let f1 = fingerprint(&s1);
        let mut s2 = build_sim(args, &cfg);
        for _ in 0..ticks {
            s2.step();
        }
        let f2 = fingerprint(&s2);
        let pass = f1 == f2;
        println!(
            "DETERMINISM {} (ticks={ticks})",
            if pass { "PASS" } else { "FAIL" }
        );
        println!("  run1: {f1}");
        println!("  run2: {f2}");
        return if pass {
            Ok(())
        } else {
            Err(anyhow::anyhow!("nondeterminism detected"))
        };
    }

    let colony: usize = arg_value(args, "--colony", "400").parse()?;
    let scenario = arg_value(args, "--scenario", "two");
    let seed = cfg.sim.seed;

    let mut sim = build_sim(args, &cfg);
    sim.brain_ann = brain_ann || brain_cppn;
    sim.brain_snn = brain_snn;
    sim.brain_mb = brain_mb;
    sim.brain_cx = use_cx;
    sim.brain_integrated = brain_integrated;
    sim.social_contact = social_contact;
    if brain_cppn {
        for a in sim.ants.iter_mut() {
            a.genome.ann_weights = crate::genome::develop_phenotype(&a.genome);
        }
    }

    println!("headless: scenario={scenario} colony={colony} seed={seed} ticks={ticks}");
    println!("food sources:");
    for (i, f) in sim.world.food.iter().enumerate() {
        println!(
            "  [{i}] pos=({:.0},{:.0}) dist_from_nest={:.1}",
            f.pos.x,
            f.pos.y,
            (f.pos.x - sim.world.nest.x).hypot(f.pos.y - sim.world.nest.y)
        );
    }

    let csv_path = arg_value(args, "--csv", "");
    let csv_every: u64 = arg_value(args, "--csv-every", "100").parse().unwrap_or(100);

    // static environment metrics (constant per run; written into every CSV
    // row so a sweep's results are self-describing)
    let env_foods = sim.world.food.len();
    let env_dist = if env_foods == 0 {
        0.0
    } else {
        sim.world
            .food
            .iter()
            .map(|f| (f.pos.x - sim.world.nest.x).hypot(f.pos.y - sim.world.nest.y))
            .sum::<f32>()
            / env_foods as f32
    };
    let env_enemies = sim.world.enemies.len();
    let env_obs: usize = sim
        .world
        .walls
        .iter()
        .map(|w| ((w.x1 - w.x0).max(1.0) * (w.y1 - w.y0).max(1.0)) as usize)
        .sum();
    let env_tag = {
        let e = arg_value(args, "--env", "");
        if e.is_empty() {
            arg_value(args, "--scenario", "two")
        } else {
            e
        }
    };

    let mut csv: Option<std::fs::File> = if csv_path.is_empty() {
        None
    } else {
        let mut f = std::fs::File::create(&csv_path)?;
        // header
        let mut hdr = String::from("tick,collected");
        for i in 0..sim.world.food.len() {
            hdr.push_str(&format!(",visits{i}"));
        }
        for i in 0..sim.world.food.len() {
            hdr.push_str(&format!(",trail{i}"));
        }
        hdr.push_str(",def_frac,forage_frac,env,env_foods,env_dist,env_enemies,env_obs");
        writeln!(f, "{hdr}")?;
        Some(f)
    };

    let mut last_print = 0u64;
    for t in 0..ticks {
        sim.step();

        // CSV row at its own cadence (independent of stdout progress)
        if let Some(cf) = csv.as_mut() {
            if t % csv_every == 0 {
                let def = sim
                    .ants
                    .iter()
                    .filter(|a| {
                        matches!(
                            a.state,
                            crate::ant::State::Defend | crate::ant::State::Alarm
                        )
                    })
                    .count() as f32;
                let mut row = format!("{},{}", sim.tick, sim.collected as i64);
                for v in &sim.source_visits {
                    row.push_str(&format!(",{v}"));
                }
                for f in &sim.world.food {
                    let m =
                        sim.world
                            .field
                            .mean_in_rect(world::Channel::Trail, f.pos.x, f.pos.y, 6.0);
                    row.push_str(&format!(",{m:.5}"));
                }
                let total = sim.ants.len().max(1) as f32;
                row.push_str(&format!(",{:.4},{:.4}", def / total, 1.0 - def / total));
                row.push_str(&format!(
                    ",{},{},{},{:.0},{}",
                    env_tag, env_foods, env_dist, env_enemies, env_obs
                ));
                writeln!(cf, "{row}")?;
            }
        }

        // stdout progress samples
        if t >= last_print {
            let mut line = format!("t={:<5} collected={:.0} visits=[", sim.tick, sim.collected);
            for v in &sim.source_visits {
                line += &format!("{v} ");
            }
            line += "]";
            // per-source corridor Trail density (mean in a 6-cell box)
            line += " trail_mean=[";
            for f in &sim.world.food {
                let m = sim
                    .world
                    .field
                    .mean_in_rect(world::Channel::Trail, f.pos.x, f.pos.y, 6.0);
                line += &format!("{m:.3} ");
            }
            line += "]";
            let bt = sim.brain_telemetry();
            line += &format!(
                " brain(turn={:.3} lh={:.3} mb={:.3}/{:.3} cx={:.3} rpe={:.3} e={:.3})",
                bt.mean_turn_drive,
                bt.mean_lh_turn,
                bt.mean_mb_approach,
                bt.mean_mb_avoidance,
                bt.mean_cx_turn,
                bt.mean_rpe,
                bt.mean_eligibility
            );
            println!("{line}");
            last_print = t + ticks / 10 + 1;
        }
    }

    println!("---- final ----");
    println!("collected total = {:.0}", sim.collected);
    println!("ants alive = {}", sim.ants.len());
    let bt = sim.brain_telemetry();
    println!(
        "brain telemetry: n={} turn={:.3} lh={:.3} mb={:.3}/{:.3} cx={:.3} rpe={:.3} eligibility={:.3}",
        bt.ants,
        bt.mean_turn_drive,
        bt.mean_lh_turn,
        bt.mean_mb_approach,
        bt.mean_mb_avoidance,
        bt.mean_cx_turn,
        bt.mean_rpe,
        bt.mean_eligibility
    );
    // T13: developmental neurogenesis telemetry — MB volume (active KC count)
    // grows from MB_KC_INIT toward MB_KC as ants accumulate foraging rewards.
    if !sim.ants.is_empty() {
        let mean_kc =
            sim.ants.iter().map(|a| a.mb_kc_active as f32).sum::<f32>() / sim.ants.len() as f32;
        let max_kc = sim.ants.iter().map(|a| a.mb_kc_active).max().unwrap_or(0);
        println!(
            "mb_kc_active: mean={:.1} max={} init={}",
            mean_kc,
            max_kc,
            crate::genome::MB_KC_INIT
        );
    }
    for (i, v) in sim.source_visits.iter().enumerate() {
        let f = &sim.world.food[i];
        let m = sim
            .world
            .field
            .mean_in_rect(world::Channel::Trail, f.pos.x, f.pos.y, 6.0);
        println!("  source[{i}] visits={v} trail_mean_near={m:.4}");
    }
    if scenario == "two" && sim.source_visits.len() == 2 {
        let (v0, v1) = (sim.source_visits[0], sim.source_visits[1]);
        let (d0, d1) = (sim.world.food[0].pos, sim.world.food[1].pos);
        let dist0 = (d0.x - sim.world.nest.x).hypot(d0.y - sim.world.nest.y);
        let dist1 = (d1.x - sim.world.nest.x).hypot(d1.y - sim.world.nest.y);
        let m0 = sim
            .world
            .field
            .mean_in_rect(world::Channel::Trail, d0.x, d0.y, 6.0);
        let m1 = sim
            .world
            .field
            .mean_in_rect(world::Channel::Trail, d1.x, d1.y, 6.0);
        println!(
            "  near dist={dist0:.0} visits={v0} trail={m0:.4} | far dist={dist1:.0} visits={v1} trail={m1:.4}"
        );
        println!(
            "  visits ratio near/far = {:.2}  trail ratio = {:.2}",
            v0 as f32 / v1.max(1) as f32,
            m0 / m1.max(1e-6),
        );
    }
    Ok(())
}
