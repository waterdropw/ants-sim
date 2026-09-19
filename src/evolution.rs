//! Phase-2 evolution machinery: fitness evaluation of one candidate genome
//! in one environment. The evolution loop (C4) calls `evaluate` per candidate;
//! `score` is the composite selection scalar.

use crate::ant::State;
use crate::config::Config;
use crate::environment::Environment;
use crate::genome::Genome;
use crate::sim::Simulator;
use crate::world::{Channel, Vec2};

#[derive(Clone, Debug)]
pub struct Fitness {
    pub collected: f32,
    pub per_source: Vec<u32>,
    pub mean_def_frac: f32,
    pub trail_total: f64,
    pub survival: f32,
    /// T18: behavioral fingerprint (6 state-fractions + per-source visit
    /// distribution + tanh-normalized trail total). Used for behavioral-novelty
    /// diversity, parallel to the genotypic diversity.
    pub behavior: Vec<f32>,
    /// Composite selection scalar. `collected` is primary; the defense term
    /// keeps predator environments (where foraging is suppressed by alarm)
    /// from being a flat zero-signal landscape; the survival term rewards
    /// not starving.
    pub score: f32,
}

impl Fitness {
    /// Construct + compute the composite score.
    fn new(
        collected: f32,
        per_source: Vec<u32>,
        mean_def_frac: f32,
        trail_total: f64,
        survival: f32,
        behavior: Vec<f32>,
        colony: usize,
    ) -> Self {
        let score = collected + 0.3 * mean_def_frac * colony as f32 + 0.2 * survival * colony as f32;
        Self { collected, per_source, mean_def_frac, trail_total, survival, behavior, score }
    }
}

/// T18: Euclidean distance between two behavioral fingerprints (padded to
/// equal length by 0 — within one run all candidates share the same env so
/// lengths match). Components are already ~[0,1] so no extra normalization.
/// T19: pub for novelty-search reuse.
pub fn behavior_distance(a: &[f32], b: &[f32]) -> f64 {
    let n = a.len().max(b.len());
    let mut s = 0.0f64;
    for i in 0..n {
        let av = a.get(i).copied().unwrap_or(0.0) as f64;
        let bv = b.get(i).copied().unwrap_or(0.0) as f64;
        let d = av - bv;
        s += d * d;
    }
    s.sqrt() / (n.max(1) as f64).sqrt()
}

/// Evaluate a candidate genome in `env`: fresh deterministic sim (same seed
/// for every candidate, so fitness differences come from the genome, not RNG),
/// run `ticks`, return Fitness.
pub fn evaluate(
    cfg: &Config,
    env: &Environment,
    genome: &Genome,
    ticks: u64,
    colony: usize,
    brain_ann: bool,
    brain_cppn: bool,
    brain_snn: bool,
    brain_mb: bool,
    brain_cx: bool,
    seasonal: bool,
) -> Fitness {
    // Indirect encoding (T2.3): develop phenotype from compact genotype.
    let mut g = genome.clone();
    if brain_cppn {
        g.ann_weights = crate::genome::develop_phenotype(genome);
    }
    let mut sim = Simulator::new(cfg.world.width, cfg.world.height, cfg.sim.seed, &g);
    sim.set_colony_size(colony, &g);
    sim.world.nest = Vec2::new(cfg.world.nest_x, cfg.world.nest_y);
    sim.world.nest_radius = cfg.world.nest_radius;
    sim.brain_ann = brain_ann || brain_cppn || brain_snn || brain_mb;
    sim.brain_snn = brain_snn;
    sim.brain_mb = brain_mb;
    sim.brain_cx = brain_cx;
    sim.apply_environment(env);
    // T7.7: in seasonal mode, start food at renewable levels
    if seasonal {
        for f in sim.world.food.iter_mut() {
            f.amount = 100.0;
        }
    }
    let initial = sim.ants.len().max(1) as f32;

    // T18: sample the full 6-state distribution every 10 ticks (was Defend/Alarm
    // only). The mean_def_frac (Alarm+Defend) is derived from this, preserving
    // the existing score signal; the full vector feeds the behavioral fingerprint.
    let mut state_sum = [0.0f32; 6]; // Explore, FollowTrail, CarryReturn, Alarm, Defend, Nurse
    let mut samples = 0u32;
    for t in 0..ticks {
        sim.step();
        if t % 10 == 0 {
            let n = sim.ants.len().max(1) as f32;
            let mut counts = [0.0f32; 6];
            for a in &sim.ants {
                let i = match a.state {
                    State::Explore => 0,
                    State::FollowTrail => 1,
                    State::CarryReturn => 2,
                    State::Alarm => 3,
                    State::Defend => 4,
                    State::Nurse => 5,
                };
                counts[i] += 1.0;
            }
            for i in 0..6 {
                state_sum[i] += counts[i] / n;
            }
            samples += 1;
        }
    }
    let state_fracs: [f32; 6] =
        state_sum.map(|s| if samples > 0 { s / samples as f32 } else { 0.0 });
    let mean_def_frac = state_fracs[3] + state_fracs[4]; // Alarm + Defend
    let trail_total: f64 = sim
        .world
        .field
        .channel_slice(Channel::Trail)
        .iter()
        .map(|&v| v as f64)
        .sum();
    let survival = sim.ants.len() as f32 / initial;

    // T18: behavioral fingerprint = 6 state fractions + per-source visit
    // distribution (normalized) + tanh-normalized trail total.
    let total_visits = sim.source_visits.iter().sum::<u32>().max(1) as f32;
    let mut behavior: Vec<f32> = Vec::with_capacity(6 + sim.source_visits.len() + 1);
    behavior.extend(state_fracs);
    behavior.extend(sim.source_visits.iter().map(|&v| v as f32 / total_visits));
    behavior.push((trail_total / (colony as f64 * 10.0)).tanh() as f32);

    Fitness::new(
        sim.collected,
        sim.source_visits.clone(),
        mean_def_frac,
        trail_total,
        survival,
        behavior,
        colony,
    )
}

/// Robust fitness: average `evaluate` over `n_seeds` independent sim seeds
/// (same env/genome, different RNG). Reduces the selection noise that a
/// single stochastic colony run introduces — so evolution picks genomes that
/// are genuinely better, not lucky.
pub fn evaluate_multi(
    cfg: &Config,
    env: &Environment,
    genome: &Genome,
    ticks: u64,
    colony: usize,
    n_seeds: usize,
    brain_ann: bool,
    brain_cppn: bool,
    brain_snn: bool,
    brain_mb: bool,
    brain_cx: bool,
    seasonal: bool,
) -> Fitness {
    if n_seeds <= 1 {
        return evaluate(cfg, env, genome, ticks, colony, brain_ann, brain_cppn, brain_snn, brain_mb, brain_cx, seasonal);
    }
    let mut acc_col = 0.0f32;
    let mut acc_def = 0.0f32;
    let mut acc_trail = 0.0f64;
    let mut acc_survival = 0.0f32;
    let mut acc_score = 0.0f32;
    let mut last_src = vec![];
    let mut acc_behavior: Vec<f32> = vec![];
    for k in 0..n_seeds {
        let mut cfgk = cfg.clone();
        cfgk.sim.seed = cfg.sim.seed.wrapping_add(k as u64 * 0x1000_0003);
        let f = evaluate(&cfgk, env, genome, ticks, colony, brain_ann, brain_cppn, brain_snn, brain_mb, brain_cx, seasonal);
        acc_col += f.collected;
        acc_def += f.mean_def_frac;
        acc_trail += f.trail_total;
        acc_survival += f.survival;
        acc_score += f.score;
        last_src = f.per_source;
        // T18: average the behavioral fingerprint across seeds
        if acc_behavior.is_empty() {
            acc_behavior = f.behavior.clone();
        } else {
            for (i, v) in f.behavior.iter().enumerate() {
                if let Some(s) = acc_behavior.get_mut(i) {
                    *s += *v;
                }
            }
        }
    }
    let n = n_seeds as f32;
    for v in acc_behavior.iter_mut() {
        *v /= n;
    }
    // Fitness::new recomputes score = collected + 0.3*def*colony from the
    // averaged fields, which equals the mean score (linear), so it stays
    // consistent with the per-seed averaging.
    let mut avg = Fitness::new(acc_col / n, last_src, acc_def / n, acc_trail / n as f64, acc_survival / n, acc_behavior, colony);
    // keep the true averaged score (in case the formula ever goes non-linear)
    avg.score = acc_score / n;
    avg
}

/// Per-generation record for the evolution history.
#[derive(Clone, Debug)]
pub struct GenRecord {
    pub gen: u32,
    pub best: f32,
    pub mean: f32,
    pub worst: f32,
}

pub struct EvolveResult {
    pub best_genome: Genome,
    pub best_fitness: Fitness,
    pub history: Vec<GenRecord>,
    /// per-generation mean pairwise genotypic distance (T5.3 open-ended litmus)
    pub diversity: Vec<f64>,
    /// T18: per-generation mean pairwise BEHAVIORAL distance (behavioral-
    /// novelty litmus). Open-ended evolution should keep both genotypic and
    /// behavioral diversity from collapsing.
    pub diversity_behavior: Vec<f64>,
    /// T19: per-generation mean novelty (k-nearest-neighbor behavioral
    /// distance) under --novelty; empty if novelty search is off.
    pub novelty: Vec<f64>,
}

/// Tournament selection: pick `k` random indices, return the one with the
/// highest score among them.
fn tournament(scores: &[f32], rng: &mut impl rand::Rng, k: usize) -> usize {
    let n = scores.len();
    let mut best = rng.gen_range(0..n);
    let mut best_s = scores[best];
    for _ in 1..k {
        let i = rng.gen_range(0..n);
        if scores[i] > best_s {
            best = i;
            best_s = scores[i];
        }
    }
    best
}

/// Run a simple GA: evaluate the population each generation, keep an elite,
/// refill with tournament-selected BLX-crossover children that are then
/// mutated. Deterministic given `seed`.
pub fn run(
    cfg: &Config,
    env: &Environment,
    base: &Genome,
    pop: usize,
    gens: u32,
    ticks: u64,
    colony: usize,
    seed: u64,
    n_seeds: usize,
    brain_ann: bool,
    brain_cppn: bool,
    brain_snn: bool,
    brain_mb: bool,
    brain_cx: bool,
    seasonal: bool,
    niche: bool,
    novelty: bool,
) -> EvolveResult {
    use rand::SeedableRng;
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);

    // initial population: base + its mutants. For the ANN brain the default
    // genome already carries a foraging seed, so mutants refine a working
    // controller rather than cold-starting from scratch.
    let mut population: Vec<Genome> = Vec::with_capacity(pop);
    population.push(base.clone());
    for _ in 1..pop {
        population.push(base.mutate(&mut rng));
    }

    let mut history: Vec<GenRecord> = Vec::new();
    let mut diversity: Vec<f64> = Vec::new();
    let mut diversity_behavior: Vec<f64> = Vec::new();
    let mut novelty_series: Vec<f64> = Vec::new();
    let mut best_genome = base.clone();
    let mut best_fitness = Fitness {
        collected: 0.0,
        per_source: vec![],
        mean_def_frac: 0.0,
        trail_total: 0.0,
        survival: 0.0,
        behavior: vec![],
        score: f32::NEG_INFINITY,
    };

    for gen in 0..gens {
        // evaluate (deterministic per candidate: same env/seed)
        let scored: Vec<(Genome, Fitness)> = population
            .iter()
            .map(|g| {
                let f = evaluate_multi(cfg, env, g, ticks, colony, n_seeds, brain_ann, brain_cppn, brain_snn, brain_mb, brain_cx, seasonal);
                (g.clone(), f)
            })
            .collect();
        let scores: Vec<f32> = scored.iter().map(|(_, f)| f.score).collect();
        let best = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let worst = scores.iter().cloned().fold(f32::INFINITY, f32::min);
        let mean = scores.iter().sum::<f32>() / scores.len() as f32;
        history.push(GenRecord { gen, best, mean, worst });

        // open-ended litmus (T5.3 genotypic + T18 behavioral): mean pairwise
        // distance over genomes AND over behavioral fingerprints.
        let mut dsum = 0.0f64;
        let mut dsum_b = 0.0f64;
        let mut dn = 0u64;
        for i in 0..scored.len() {
            for j in (i + 1)..scored.len() {
                dsum += crate::genome::Genome::distance(&scored[i].0, &scored[j].0);
                dsum_b += behavior_distance(&scored[i].1.behavior, &scored[j].1.behavior);
                dn += 1;
            }
        }
        diversity.push(if dn > 0 { dsum / dn as f64 } else { 0.0 });
        diversity_behavior.push(if dn > 0 { dsum_b / dn as f64 } else { 0.0 });

        // T19: novelty search (NSLC-lite) — per-candidate novelty = mean
        // behavioral distance to its k nearest neighbours in the population.
        // Fed into selection below; reported per generation.
        let novelty_vals: Vec<f64> = if novelty && scored.len() > 1 {
            let k = (scored.len() / 4).clamp(1, 5);
            (0..scored.len())
                .map(|i| {
                    let mut ds: Vec<f64> = (0..scored.len())
                        .filter(|&j| j != i)
                        .map(|j| behavior_distance(&scored[i].1.behavior, &scored[j].1.behavior))
                        .collect();
                    ds.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    ds.iter().take(k).sum::<f64>() / k as f64
                })
                .collect()
        } else {
            vec![]
        };
        if !novelty_vals.is_empty() {
            let mean_n = novelty_vals.iter().sum::<f64>() / novelty_vals.len() as f64;
            novelty_series.push(mean_n);
        }

        // track global best
        let (gi, _) = scored
            .iter()
            .map(|(_, f)| f.score)
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();
        if scored[gi].1.score > best_fitness.score {
            best_genome = scored[gi].0.clone();
            best_fitness = scored[gi].1.clone();
        }

        // next generation: elitism (keep top 2 by RAW score) + tournament-crossover-mutate
        let mut order: Vec<usize> = (0..pop).collect();
        order.sort_by(|&a, &b| scores[b].partial_cmp(&scores[a]).unwrap());
        let mut next: Vec<Genome> = Vec::with_capacity(pop);
        for &i in order.iter().take(2) {
            next.push(scored[i].0.clone());
        }
        // selection scores: with niching, use fitness-sharing-adjusted scores so
        // crowded genomes are penalised and diversity is maintained.
        let select_scores: Vec<f32> = if niche {
            let threshold = 0.25f64;
            (0..pop)
                .map(|i| {
                    let mut share = 1.0f64;
                    for j in 0..pop {
                        if i != j {
                            let d = crate::genome::Genome::distance(&scored[i].0, &scored[j].0);
                            if d < threshold {
                                share += 1.0 - d / threshold;
                            }
                        }
                    }
                    (scores[i] as f64 / share) as f32
                })
                .collect()
        } else {
            scores.clone()
        };
        // T19: novelty-biased selection — add a novelty bonus so behaviorally
        // novel candidates are favoured alongside fitness (NSLC-lite). Bonus
        // scaled to the population's mean |score| so novelty is comparable to
        // fitness in magnitude. Compatible with --niche (applied after sharing).
        let select_scores: Vec<f32> = if novelty && !novelty_vals.is_empty() {
            let mean_abs = select_scores.iter().map(|s| s.abs()).sum::<f32>()
                / select_scores.len().max(1) as f32;
            const NOVELTY_W: f32 = 1.0;
            select_scores
                .iter()
                .enumerate()
                .map(|(i, &s)| s + NOVELTY_W * novelty_vals[i] as f32 * mean_abs)
                .collect()
        } else {
            select_scores
        };
        while next.len() < pop {
            let pa = tournament(&select_scores, &mut rng, 3);
            let pb = tournament(&select_scores, &mut rng, 3);
            let child = scored[pa].0.crossover(&scored[pb].0, &mut rng).mutate(&mut rng);
            next.push(child);
        }
        population = next;
    }

    EvolveResult { best_genome, best_fitness, history, diversity, diversity_behavior, novelty: novelty_series }
}

/// Multi-level selection (C5): one diverse colony is built from a genome
/// `pool`; ants with different genotypes forage together and compete at the
/// *individual* level — the carriers that deliver the most propagate their
/// (mutated) genomes into the next pool. Colony-level score is still tracked
/// for the history. This is intra-colony (individual) selection on top of the
/// colony-level selection of `run`.
pub fn run_multilevel(
    cfg: &Config,
    env: &Environment,
    base: &Genome,
    pool_size: usize,
    gens: u32,
    ticks: u64,
    colony: usize,
    seed: u64,
    brain_ann: bool,
    brain_cppn: bool,
    brain_snn: bool,
    brain_mb: bool,
    brain_cx: bool,
    seasonal: bool,
) -> EvolveResult {
    use rand::{Rng, SeedableRng};
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed ^ 0xC5C5);
    let mut pool: Vec<Genome> = (0..pool_size).map(|_| base.mutate(&mut rng)).collect();

    let mut history: Vec<GenRecord> = Vec::new();
    let mut diversity: Vec<f64> = Vec::new();
    let mut diversity_behavior: Vec<f64> = Vec::new();
    let mut novelty_series: Vec<f64> = Vec::new();
    let mut best_genome = base.clone();
    let mut best_fitness = Fitness {
        collected: 0.0,
        per_source: vec![],
        mean_def_frac: 0.0,
        trail_total: 0.0,
        survival: 0.0,
        behavior: vec![],
        score: f32::NEG_INFINITY,
    };

    for gen in 0..gens {
        // build one diverse colony from the pool
        let mut sim = Simulator::new(cfg.world.width, cfg.world.height, cfg.sim.seed, base);
        sim.world.nest = Vec2::new(cfg.world.nest_x, cfg.world.nest_y);
        sim.world.nest_radius = cfg.world.nest_radius;
        sim.brain_ann = brain_ann || brain_cppn || brain_snn || brain_mb;
        sim.brain_snn = brain_snn;
    sim.brain_mb = brain_mb;
        sim.brain_cx = brain_cx;
        if brain_cppn {
            for a in sim.ants.iter_mut() {
                a.genome.ann_weights = crate::genome::develop_phenotype(&a.genome);
            }
        }
        sim.apply_environment(env);
        sim.set_colony_from_pool(&pool, colony, &mut rng);
        let initial = sim.ants.len().max(1) as f32;

        let mut def_sum = 0.0f32;
        let mut samples = 0u32;
        for t in 0..ticks {
            sim.step();
            if t % 10 == 0 {
                let def = sim
                    .ants
                    .iter()
                    .filter(|a| matches!(a.state, State::Defend | State::Alarm))
                    .count() as f32;
                def_sum += def / sim.ants.len().max(1) as f32;
                samples += 1;
            }
        }
        let mean_def_frac = if samples > 0 { def_sum / samples as f32 } else { 0.0 };
        let trail_total: f64 = sim
            .world
            .field
            .channel_slice(Channel::Trail)
            .iter()
            .map(|&v| v as f64)
            .sum();
        let survival = sim.ants.len() as f32 / initial;
        let colony_fit = Fitness::new(
            sim.collected,
            sim.source_visits.clone(),
            mean_def_frac,
            trail_total,
            survival,
            vec![],
            colony,
        );

        // individual-level selection: rank ants by lifetime deliveries
        let mut scored: Vec<(Genome, u32)> = sim
            .ants
            .iter()
            .map(|a| (a.genome.clone(), a.total_delivered))
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));

        history.push(GenRecord {
            gen,
            best: colony_fit.score,
            mean: colony_fit.score, // single colony per gen
            worst: colony_fit.score,
        });
        {
            // run_multilevel selects at the individual level (scored is
            // (Genome, total_delivered)); behavioral diversity is a per-colony
            // signal so it isn't meaningful here — genotypic only, behavior = 0.
            let mut dsum = 0.0f64;
            let mut dn = 0u64;
            for i in 0..scored.len() {
                for j in (i + 1)..scored.len() {
                    dsum += crate::genome::Genome::distance(&scored[i].0, &scored[j].0);
                    dn += 1;
                }
            }
            diversity.push(if dn > 0 { dsum / dn as f64 } else { 0.0 });
            diversity_behavior.push(0.0);
        }
        if colony_fit.score > best_fitness.score {
            best_genome = scored.first().map(|(g, _)| g.clone()).unwrap_or(base.clone());
            best_fitness = colony_fit.clone();
        }

        // next pool: top carriers (mutated) + a few fresh mutants for diversity
        let mut next: Vec<Genome> = Vec::with_capacity(pool_size);
        let topk = pool_size.min(scored.len());
        for i in 0..topk {
            next.push(scored[i].0.mutate(&mut rng));
        }
        while next.len() < pool_size {
            next.push(if rng.gen::<f32>() < 0.5 {
                base.mutate(&mut rng)
            } else {
                scored[0].0.mutate(&mut rng)
            });
        }
        pool = next;
    }

    EvolveResult { best_genome, best_fitness, history, diversity, diversity_behavior, novelty: novelty_series }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// T18: behavioral fingerprints with different per-source distributions
    /// are farther apart than identical ones.
    #[test]
    fn behavior_distance_distinguishes_patterns() {
        // two colonies that exploited different source distributions
        let a = vec![0.5, 0.5, 0.0, 0.0, 0.3, 0.0, 0.5, 0.5, 0.0]; // states + per-source(2) + trail
        let b = vec![0.5, 0.5, 0.0, 0.0, 0.8, 0.2, 0.0, 0.0, 0.3]; // different foraging
        let same = vec![0.5, 0.5, 0.0, 0.0, 0.3, 0.0, 0.5, 0.5, 0.0];
        let d_diff = behavior_distance(&a, &b);
        let d_same = behavior_distance(&a, &same);
        assert!(d_diff > d_same, "different behaviors should be farther: {} vs {}", d_diff, d_same);
        assert!(d_same < 1e-9, "identical behaviors should be ~0: {}", d_same);
    }
}

#[cfg(test)]
mod novelty_tests {
    use super::*;

    /// T19: novelty (k-nn behavioral distance) is higher for a behaviorally
    /// diverse population than for a clonal one — the signal novelty search
    /// rewards. Mirrors the run() novelty computation.
    fn mean_novelty(behaviors: &[Vec<f32>]) -> f64 {
        let n = behaviors.len();
        if n <= 1 {
            return 0.0;
        }
        let k = (n / 4).clamp(1, 5);
        let mut total = 0.0f64;
        for i in 0..n {
            let mut ds: Vec<f64> = (0..n)
                .filter(|&j| j != i)
                .map(|j| behavior_distance(&behaviors[i], &behaviors[j]))
                .collect();
            ds.sort_by(|a, b| a.partial_cmp(b).unwrap());
            total += ds.iter().take(k).sum::<f64>() / k as f64;
        }
        total / n as f64
    }

    #[test]
    fn novelty_higher_for_diverse_behaviors() {
        // diverse: 4 distinct foraging patterns
        let diverse = vec![
            vec![0.8, 0.1, 0.0, 0.0, 0.0, 0.1, 1.0, 0.0, 0.0, 0.0, 0.3],
            vec![0.1, 0.1, 0.8, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.3],
            vec![0.3, 0.3, 0.3, 0.0, 0.0, 0.1, 0.5, 0.5, 0.0, 0.0, 0.3],
            vec![0.0, 0.0, 0.0, 0.5, 0.5, 0.0, 0.2, 0.2, 0.3, 0.3, 0.3],
        ];
        // clonal: 4 identical
        let clonal = vec![diverse[0].clone(), diverse[0].clone(), diverse[0].clone(), diverse[0].clone()];
        let nd = mean_novelty(&diverse);
        let nc = mean_novelty(&clonal);
        assert!(nd > nc, "diverse population should have higher novelty: {} vs {}", nd, nc);
        assert!(nc < 1e-9, "clonal population novelty should be ~0: {}", nc);
    }
}
