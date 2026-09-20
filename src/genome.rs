//! Genome — parameters shaped so they can become evolvable in phase 2.
//!
//! Phase 1: every ant shares one genome loaded from TOML (or defaults).
//! Phase 2: add `mutate()` / `crossover()` + a fitness landscape; the
//! `Simulator` already consumes `&Genome` so the wiring is a drop-in.

use serde::{Deserialize, Serialize};

/// Parameters that describe an ant's body, senses, behaviour, pheromone
/// chemistry and neuromodulator baselines. All dimensionless or in
/// world-grid units unless noted.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Genome {
    // --- body ---
    pub max_speed: f32,
    pub turn_rate: f32, // max radians per tick

    // --- senses ---
    pub food_radius: f32,
    pub nest_radius: f32,
    pub antenna_angle: f32, // half-angle of the forward sensing fan (rad)
    pub antenna_dist: f32,  // how far ahead the antennae sample
    pub trail_threshold: f32,
    pub alarm_threshold: f32,

    // --- behaviour ---
    pub explore_rate: f32,    // random-walk jitter (Explore state)
    pub follow_strength: f32, // how strongly trail gradient biases heading
    pub aggression: f32,
    pub carry_threshold: f32,
    pub task_switch_threshold: f32,

    // --- pheromone chemistry ---
    pub trail_release_rate: f32,
    pub trail_decay: f32,
    pub alarm_release_rate: f32,
    pub alarm_decay: f32,
    pub home_release_rate: f32,

    // --- neuromodulator baselines (scalar modulation of FSM thresholds) ---
    pub exploration_baseline: f32,
    pub aggression_baseline: f32,
    pub task_bias: f32, // >0 lean toward foraging, <0 lean toward guarding

    // --- learning (phase-1 simplified) ---
    pub trail_reinforcement: f32, // local reinforcement rate when on a good trail

    // --- energy economics (T2.1; was constants) ---
    pub energy_drain: f32,  // base metabolism per tick
    pub carry_cost: f32,    // extra per tick while carrying food
    pub defend_cost: f32,   // extra per tick while defending/alarmed
    pub recharge_rate: f32, // energy regained per tick at the nest (not carrying)

    // --- ANN decision layer (T2.2); used only in --brain ann mode ---
    /// Flat weight vector for a fixed-topology MLP (IN→HID→OUT). Layout:
    /// W1[IN*HID], b1[HID], W2[HID*OUT], b2[OUT]. Empty => use FSM.
    pub ann_weights: Vec<f32>,

    // --- Indirect encoding / development (T2.3); used only in --brain cppn ---
    /// Compact genotype: a CPPN develops these into a perturbation of
    /// `ann_weights` (seed + develop(cppn_genes)). genotype dim (16) <
    /// phenotype dim (155) — the hallmark of indirect encoding.
    pub cppn_genes: Vec<f32>,

    // --- Mushroom body modular brain (T8); used only in --brain mb mode ---
    /// Weight vector for the modular AL→MB→Output brain. Layout:
    /// W_al[AL_IN*AL_GLOM] + b_al[AL_GLOM] + W_lat[AL_GLOM*AL_GLOM]
    /// + W_kc[AL_GLOM*MB_KC] + b_kc[MB_KC] + W_out[MB_KC*MB_OUT] + b_out[MB_OUT]
    pub mb_weights: Vec<f32>,
    /// T16 dual-channel dopamine gains (PAM/PPL1 analog): reward channel
    /// (food pickup/delivery → LTP) and punish channel (damage → LTD).
    /// Evolvable. Old single-channel TOMLs load via serde default 0.5.
    #[serde(default = "default_dopamine_gain")]
    pub mb_dopamine_reward_gain: f32,
    #[serde(default = "default_dopamine_gain")]
    pub mb_dopamine_punish_gain: f32,
    /// T17: satiety modulation — reward dopamine gain scales with energy
    /// deficit (hungry forager → bigger reward → stronger LTP). Drive-
    /// dependent reward, biologically. Default 0.5 (centered, avg ≈ base).
    #[serde(default = "default_satiety_gain")]
    pub mb_satiety_gain: f32,
    /// T13 neurogenesis: KCs grown per delivery (developmental structural
    /// plasticity — the MB grows with foraging experience across a lifetime)
    pub mb_neurogenesis: f32,
    /// T14 CPG: base gait frequency (leg phase-oscillator rate). Modulated by
    /// octopamine arousal → faster gait → faster walk (neuromodulator→motion).
    pub cpg_freq: f32,

    // --- Central complex (CX) ring attractor (T9); used only in --brain cx ---
    /// recurrent self-excitation gain of the cosine ring kernel (bump sustain)
    pub cx_bump_gain: f32,
    /// global lateral inhibition (winner-take-all contrast, single bump)
    pub cx_inhibition: f32,
    /// sun-compass sensory injection strength (anchors bump to true heading,
    /// correcting integration drift — polarized-light compass analog)
    pub cx_compass_gain: f32,
    /// angular-velocity → bump shift scaling (1.0 = veridical rotation)
    pub cx_shift_gain: f32,
    /// home-vector integrator leak per tick (CPU4 analog memory decay)
    pub cx_hv_leak: f32,

    // --- sensory, action and social loop (neural architecture phases 1–5) ---
    #[serde(default = "default_sensory_adapt_rate")]
    pub sensory_adapt_rate: f32,
    #[serde(default = "default_sensory_adapt_strength")]
    pub sensory_adapt_strength: f32,
    #[serde(default)]
    pub sensory_noise: f32,
    #[serde(default = "default_al_inhibition")]
    pub al_inhibition: f32,
    #[serde(default)]
    pub compass_noise: f32,
    #[serde(default)]
    pub compass_bias: f32,
    #[serde(default = "default_cx_motor_gain")]
    pub cx_motor_gain: f32,
    #[serde(default = "default_motor_speed_gain")]
    pub motor_speed_gain: f32,
    #[serde(default = "default_social_contact_gain")]
    pub social_contact_gain: f32,
    #[serde(default = "default_mb_rpe_lr")]
    pub mb_rpe_lr: f32,
    #[serde(default = "default_mb_eligibility_decay")]
    pub mb_eligibility_decay: f32,
}

fn default_sensory_adapt_rate() -> f32 {
    0.08
}
fn default_sensory_adapt_strength() -> f32 {
    0.6
}
fn default_al_inhibition() -> f32 {
    0.25
}
fn default_cx_motor_gain() -> f32 {
    0.6
}
fn default_motor_speed_gain() -> f32 {
    1.0
}
fn default_social_contact_gain() -> f32 {
    0.25
}
fn default_mb_rpe_lr() -> f32 {
    0.01
}
fn default_mb_eligibility_decay() -> f32 {
    0.9
}

pub const CPPN_GENES: usize = 16;

pub const ANN_IN: usize = 9;
pub const ANN_HID: usize = 10;
pub const ANN_OUT: usize = 5;
pub const ANN_W_MIN: f32 = -4.0;
pub const ANN_W_MAX: f32 = 4.0;

// T8 modular brain dimensions
pub const MB_AL_INPUTS: usize = 20;
pub const MB_AL_GLOM: usize = 12;
pub const MB_KC: usize = 64;
pub const MB_OUT: usize = 5;
pub const KC_THRESH: f32 = 0.15;
pub const DOPAMINE_THRESH: f32 = 0.1;
/// T16: dopamine ceiling (both reward + punish channels) — keeps the
/// neuromodulator bounded so a burst of deliveries/damage can't push it
/// arbitrarily high. Delivery + pickup release both cap at this value.
pub const DOPAMINE_MAX: f32 = 2.0;

/// T16: serde default for the dual dopamine gains so old single-channel TOMLs
/// (which carry `mb_dopamine_gain`, missing the new fields) still deserialize.
fn default_dopamine_gain() -> f32 {
    0.5
}

/// T17: serde default for the satiety gain (new field) — old TOMLs load 0.5.
fn default_satiety_gain() -> f32 {
    0.5
}
/// T17: number of hand-wired detector KCs at the start of the KC array (the
/// foraging-prior detectors in mb_seed, KC 0-9). W_kc plasticity skips these
/// so the innate steering/deposit detectors keep stable thresholds.
pub const MB_DETECTOR_KCS: usize = 10;

/// T18: output valence compartment boundary. Outputs 0..MB_APPROACH_OUT
/// (turn/trail/home) are "approach" — reward→LTP, punish→LTD. Outputs
/// MB_APPROACH_OUT..MB_OUT (alarm/attack) are "avoidance" — opponent
/// (punish→LTP, reward→LTD), mirroring PAM/PPL1 DAN compartments driving
/// approach vs avoidance MBONs in parallel.
pub const MB_APPROACH_OUT: usize = 3;

/// T13: initial active KC count (immature MB at eclosion); grows toward MB_KC
/// with foraging experience (neurogenesis). 24 = 37.5% of adult MB (biologically
/// plausible immature calyx). Empirically the gated value closest to the
/// ungated baseline (24→40 collected vs 64→45; 32/40/48/56 all worse).
pub const MB_KC_INIT: usize = 24;

// T14 CPG (central pattern generator): tripod gait — 6 legs, even legs
// (0,2,4) swing together, odd legs (1,3,5) anti-phase. The phase relationship
// is hardwired (CPG half-center output); frequency is octopamine-modulated.
pub const CPG_LEGS: usize = 6;
pub const CPG_AROUSAL_GAIN: f32 = 0.3; // octopamine → gait-frequency gain

// T11 neuromodulator dynamics (motivation/emotion, experience-driven)
pub const OCT_FORAGE_GAIN: f32 = 0.02; // octopamine rise/tick during active foraging
pub const OCT_REST_DECAY: f32 = 0.03; // octopamine fall/tick while carrying/resting
pub const OCT_MAX: f32 = 1.0; // arousal ceiling

// T9 central complex: ring-attractor heading units (≤32 for perf).
pub const CX_N: usize = 16;

pub fn mb_weight_count() -> usize {
    MB_AL_INPUTS * MB_AL_GLOM  // W_al: convergence
    + MB_AL_GLOM               // b_al
    + MB_AL_GLOM * MB_AL_GLOM  // W_lat: lateral inhibition
    + MB_AL_GLOM * MB_KC       // W_kc: AL→KC projection
    + MB_KC                    // b_kc
    + MB_KC * MB_OUT           // W_out: KC→output
    + MB_OUT // b_out
}

pub fn ann_weight_count() -> usize {
    ANN_IN * ANN_HID + ANN_HID + ANN_HID * ANN_OUT + ANN_OUT
}

/// Indirect-encoding developmental map (T2.3): a compact genotype (CPPN_GENES
/// amplitudes) develops into a perturbation of the phenotype weight vector.
/// Each weight i (at normalized position t=i/n) gets sum_k genes[k]·cos(k·π·t):
/// a smooth cosine-basis modulation. With all-zero genes → zero perturbation
/// → phenotype = foraging seed (so it forages at gen 0).
pub fn develop_cppn(genes: &[f32], n: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; n];
    let g = if genes.is_empty() { &[][..] } else { genes };
    for i in 0..n {
        let t = i as f32 / n.max(1) as f32;
        let mut v = 0.0f32;
        for (k, &amp) in g.iter().enumerate() {
            v += amp * ((k as f32 + 1.0) * std::f32::consts::PI * t).cos();
        }
        out[i] = v * 0.1; // keep the perturbation small relative to seed
    }
    out
}

/// Produce the full phenotype (seed + developed perturbation), clamped.
pub fn develop_phenotype(genome: &Genome) -> Vec<f32> {
    let seed = foraging_ann_seed();
    let dev = develop_cppn(&genome.cppn_genes, seed.len());
    seed.iter()
        .zip(dev.iter())
        .map(|(s, d)| (s + d).clamp(ANN_W_MIN, ANN_W_MAX))
        .collect()
}

/// A hand-designed foraging *seed* for the ANN weights, so the network
/// forages from generation 0 and evolution refines rather than cold-starting.
/// Inputs (9) are pre-gated in `ann_decide`:
///   0 (1-carrying)*sin(trail_rel)   1 carrying*sin(nest_rel)
///   2 (1-carrying)*trail_val        3 carrying
///   4 energy                        5 alarm_val
///   6 sin(alarm_rel)                7 cos(nest_rel)
///   8 noise (per-tick random)       ← lets the network explore (bootstrap)
/// The seed is a near-identity hidden layer + a few output gains implementing
///   turn = (1-carrying)*sin(trail) + carrying*sin(nest) + 0.3*noise
///   dep_trail = carrying ; dep_home = (1-carrying) ; attack ~ alarm_ahead
pub fn foraging_ann_seed() -> Vec<f32> {
    let n = ann_weight_count();
    let mut w = vec![0.0; n];
    let b1 = ANN_IN * ANN_HID; // offset of b1
    let w2 = b1 + ANN_HID; // offset of W2
    let b2 = w2 + ANN_HID * ANN_OUT; // offset of b2
                                     // W1: identity (hidden[h] ≈ tanh(x[h]))
    for h in 0..ANN_HID.min(ANN_IN) {
        w[h * ANN_IN + h] = 1.0;
    }
    // W2: turn(out0) ← hidden0 + hidden1 + 0.3*hidden8(noise)
    w[w2 + 0] = 1.0;
    w[w2 + 1] = 1.0;
    w[w2 + 8] = 0.3;
    // dep_trail(out1) ← hidden3 (carrying)
    w[w2 + 1 * ANN_HID + 3] = 1.0;
    // dep_home(out2) = b2 - hidden3  → (1 - carrying)
    w[w2 + 2 * ANN_HID + 3] = -1.0;
    w[b2 + 2] = 1.0;
    // attack(out4) ← hidden6 (sin alarm_rel)
    w[w2 + 4 * ANN_HID + 6] = 1.0;
    w
}

impl Default for Genome {
    fn default() -> Self {
        Self {
            max_speed: 1.2,
            turn_rate: 0.6,
            food_radius: 3.0,
            nest_radius: 4.0,
            antenna_angle: 0.5,
            antenna_dist: 4.0,
            trail_threshold: 0.01,
            alarm_threshold: 0.05,
            explore_rate: 0.12,
            follow_strength: 3.0,
            aggression: 0.5,
            carry_threshold: 0.5,
            task_switch_threshold: 0.3,
            trail_release_rate: 0.15,
            trail_decay: 0.015,
            alarm_release_rate: 1.0,
            alarm_decay: 0.025,
            home_release_rate: 0.15,
            exploration_baseline: 0.5,
            aggression_baseline: 0.5,
            task_bias: 0.0,
            trail_reinforcement: 0.2,
            energy_drain: 0.0009,
            carry_cost: 0.0016,
            defend_cost: 0.0025,
            recharge_rate: 0.05,
            ann_weights: foraging_ann_seed(),
            cppn_genes: vec![0.0; CPPN_GENES],
            mb_weights: mb_seed(),
            mb_dopamine_reward_gain: 0.5,
            mb_dopamine_punish_gain: 0.5,
            mb_satiety_gain: 0.5,
            mb_neurogenesis: 1.0,
            cpg_freq: 1.0,
            // T9: evolved-tuned defaults (rich_close 12-gen evolution, collected 274->884).
            // Mechanistically: near-zero hv_leak = faithful path integration; lower bump_gain
            // + higher inhibition = sharper/more stable ring bump.
            cx_bump_gain: 0.21,
            cx_inhibition: 0.40,
            cx_compass_gain: 0.5,
            cx_shift_gain: 0.85,
            cx_hv_leak: 0.000035,
            sensory_adapt_rate: default_sensory_adapt_rate(),
            sensory_adapt_strength: default_sensory_adapt_strength(),
            sensory_noise: 0.0,
            al_inhibition: default_al_inhibition(),
            compass_noise: 0.0,
            compass_bias: 0.0,
            cx_motor_gain: default_cx_motor_gain(),
            motor_speed_gain: default_motor_speed_gain(),
            social_contact_gain: default_social_contact_gain(),
            mb_rpe_lr: default_mb_rpe_lr(),
            mb_eligibility_decay: default_mb_eligibility_decay(),
        }
    }
}

// Phase-2 evolution operators. Each field has a valid (lo, hi) range; mutation
// is Gaussian (Box–Muller), crossover is BLX-α blend. Kept self-contained so
// the evolution loop (C4) is a drop-in.
use rand::Rng;

/// Valid range + per-field mutation sigma fraction for one gene.
fn ranges() -> [(&'static str, (f32, f32, f32)); 31] {
    // (field_name, (lo, hi, sigma_fraction))
    [
        ("max_speed", (0.3, 3.0, 0.10)),
        ("turn_rate", (0.05, 1.5, 0.10)),
        ("food_radius", (1.0, 8.0, 0.10)),
        ("nest_radius", (2.0, 10.0, 0.10)),
        ("antenna_angle", (0.1, 1.2, 0.10)),
        ("antenna_dist", (1.0, 8.0, 0.10)),
        ("trail_threshold", (0.001, 0.2, 0.15)),
        ("alarm_threshold", (0.005, 0.3, 0.15)),
        ("explore_rate", (0.01, 0.5, 0.12)),
        ("follow_strength", (0.5, 6.0, 0.12)),
        ("aggression", (0.0, 1.0, 0.15)),
        ("carry_threshold", (0.0, 1.0, 0.10)),
        ("task_switch_threshold", (0.0, 1.0, 0.10)),
        ("trail_release_rate", (0.02, 0.6, 0.12)),
        ("trail_decay", (0.002, 0.05, 0.12)),
        ("alarm_release_rate", (0.2, 3.0, 0.12)),
        ("alarm_decay", (0.005, 0.08, 0.12)),
        ("home_release_rate", (0.02, 0.5, 0.12)),
        ("exploration_baseline", (0.0, 1.0, 0.15)),
        ("aggression_baseline", (0.0, 1.0, 0.15)),
        ("task_bias", (-1.0, 1.0, 0.15)),
        ("trail_reinforcement", (0.0, 1.0, 0.12)),
        ("energy_drain", (0.0002, 0.003, 0.12)),
        ("carry_cost", (0.0004, 0.006, 0.12)),
        ("defend_cost", (0.0006, 0.009, 0.12)),
        ("recharge_rate", (0.01, 0.15, 0.12)),
        ("cx_bump_gain", (0.2, 3.0, 0.12)),
        ("cx_inhibition", (0.0, 1.0, 0.15)),
        ("cx_compass_gain", (0.0, 2.0, 0.12)),
        ("cx_shift_gain", (0.5, 1.5, 0.10)),
        ("cx_hv_leak", (0.0, 0.01, 0.15)),
    ]
}

/// Standard-normal sample via Box–Muller (no extra dep).
fn gauss(rng: &mut impl Rng) -> f32 {
    let u1 = rng.gen::<f32>().max(1e-9);
    let u2 = rng.gen::<f32>();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
}

/// With probability `rate`, perturb `v` by a Gaussian of width `sigma_frac*(hi-lo)`,
/// clamped to [lo, hi].
fn perturb(rng: &mut impl Rng, v: f32, lo: f32, hi: f32, sigma_frac: f32, rate: f32) -> f32 {
    if rng.gen::<f32>() < rate {
        let s = (hi - lo) * sigma_frac;
        (v + gauss(rng) * s).clamp(lo, hi)
    } else {
        v
    }
}

/// BLX-α crossover: child gene uniform in an interval stretched α beyond the
/// parents' span, clamped to the gene's valid range.
fn blx(rng: &mut impl Rng, a: f32, b: f32, lo: f32, hi: f32, alpha: f32) -> f32 {
    let (min, max) = if a < b { (a, b) } else { (b, a) };
    let d = max - min;
    let lox = (min - alpha * d).max(lo);
    let hix = (max + alpha * d).min(hi);
    rng.gen_range(lox..=hix)
}

impl Genome {
    /// Per-field Gaussian mutation. Each field mutated with prob `rate`;
    /// perturbation width = `sigma_frac` × field range.
    pub fn mutate(&self, rng: &mut impl Rng) -> Self {
        let r = ranges();
        let at = |i: usize| r[i].1;
        let mut g = self.clone();
        g.max_speed = perturb(rng, g.max_speed, at(0).0, at(0).1, at(0).2, 0.4);
        g.turn_rate = perturb(rng, g.turn_rate, at(1).0, at(1).1, at(1).2, 0.4);
        g.food_radius = perturb(rng, g.food_radius, at(2).0, at(2).1, at(2).2, 0.4);
        g.nest_radius = perturb(rng, g.nest_radius, at(3).0, at(3).1, at(3).2, 0.4);
        g.antenna_angle = perturb(rng, g.antenna_angle, at(4).0, at(4).1, at(4).2, 0.4);
        g.antenna_dist = perturb(rng, g.antenna_dist, at(5).0, at(5).1, at(5).2, 0.4);
        g.trail_threshold = perturb(rng, g.trail_threshold, at(6).0, at(6).1, at(6).2, 0.4);
        g.alarm_threshold = perturb(rng, g.alarm_threshold, at(7).0, at(7).1, at(7).2, 0.4);
        g.explore_rate = perturb(rng, g.explore_rate, at(8).0, at(8).1, at(8).2, 0.4);
        g.follow_strength = perturb(rng, g.follow_strength, at(9).0, at(9).1, at(9).2, 0.4);
        g.aggression = perturb(rng, g.aggression, at(10).0, at(10).1, at(10).2, 0.4);
        g.carry_threshold = perturb(rng, g.carry_threshold, at(11).0, at(11).1, at(11).2, 0.4);
        g.task_switch_threshold = perturb(
            rng,
            g.task_switch_threshold,
            at(12).0,
            at(12).1,
            at(12).2,
            0.4,
        );
        g.trail_release_rate =
            perturb(rng, g.trail_release_rate, at(13).0, at(13).1, at(13).2, 0.4);
        g.trail_decay = perturb(rng, g.trail_decay, at(14).0, at(14).1, at(14).2, 0.4);
        g.alarm_release_rate =
            perturb(rng, g.alarm_release_rate, at(15).0, at(15).1, at(15).2, 0.4);
        g.alarm_decay = perturb(rng, g.alarm_decay, at(16).0, at(16).1, at(16).2, 0.4);
        g.home_release_rate = perturb(rng, g.home_release_rate, at(17).0, at(17).1, at(17).2, 0.4);
        g.exploration_baseline = perturb(
            rng,
            g.exploration_baseline,
            at(18).0,
            at(18).1,
            at(18).2,
            0.4,
        );
        g.aggression_baseline = perturb(
            rng,
            g.aggression_baseline,
            at(19).0,
            at(19).1,
            at(19).2,
            0.4,
        );
        g.task_bias = perturb(rng, g.task_bias, at(20).0, at(20).1, at(20).2, 0.4);
        g.trail_reinforcement = perturb(
            rng,
            g.trail_reinforcement,
            at(21).0,
            at(21).1,
            at(21).2,
            0.4,
        );
        g.energy_drain = perturb(rng, g.energy_drain, at(22).0, at(22).1, at(22).2, 0.4);
        g.carry_cost = perturb(rng, g.carry_cost, at(23).0, at(23).1, at(23).2, 0.4);
        g.defend_cost = perturb(rng, g.defend_cost, at(24).0, at(24).1, at(24).2, 0.4);
        g.recharge_rate = perturb(rng, g.recharge_rate, at(25).0, at(25).1, at(25).2, 0.4);
        g.cx_bump_gain = perturb(rng, g.cx_bump_gain, at(26).0, at(26).1, at(26).2, 0.4);
        g.cx_inhibition = perturb(rng, g.cx_inhibition, at(27).0, at(27).1, at(27).2, 0.4);
        g.cx_compass_gain = perturb(rng, g.cx_compass_gain, at(28).0, at(28).1, at(28).2, 0.4);
        g.cx_shift_gain = perturb(rng, g.cx_shift_gain, at(29).0, at(29).1, at(29).2, 0.4);
        g.cx_hv_leak = perturb(rng, g.cx_hv_leak, at(30).0, at(30).1, at(30).2, 0.4);
        // ANN weights (vector, not in the per-field table)
        let n = ann_weight_count();
        if g.ann_weights.len() != n {
            g.ann_weights = vec![0.0; n];
        }
        for w in g.ann_weights.iter_mut() {
            *w = perturb(rng, *w, ANN_W_MIN, ANN_W_MAX, 0.15, 0.5);
        }
        // CPPN genes (compact indirect-encoding genotype)
        if g.cppn_genes.len() != CPPN_GENES {
            g.cppn_genes = vec![0.0; CPPN_GENES];
        }
        for w in g.cppn_genes.iter_mut() {
            *w = perturb(rng, *w, -1.0, 1.0, 0.2, 0.5);
        }
        // MB weights (T8)
        if g.mb_weights.len() != mb_weight_count() {
            g.mb_weights = vec![0.0; mb_weight_count()];
        }
        for w in g.mb_weights.iter_mut() {
            *w = perturb(rng, *w, ANN_W_MIN, ANN_W_MAX, 0.15, 0.5);
        }
        g.mb_dopamine_reward_gain = perturb(rng, g.mb_dopamine_reward_gain, 0.1, 1.0, 0.15, 0.4);
        g.mb_dopamine_punish_gain = perturb(rng, g.mb_dopamine_punish_gain, 0.1, 1.0, 0.15, 0.4);
        g.mb_satiety_gain = perturb(rng, g.mb_satiety_gain, 0.0, 1.5, 0.15, 0.4);
        g.mb_neurogenesis = perturb(rng, g.mb_neurogenesis, 0.2, 3.0, 0.15, 0.6);
        g.cpg_freq = perturb(rng, g.cpg_freq, 0.5, 2.0, 0.15, 0.4);
        g.sensory_adapt_rate = perturb(rng, g.sensory_adapt_rate, 0.0, 0.5, 0.15, 0.4);
        g.sensory_adapt_strength = perturb(rng, g.sensory_adapt_strength, 0.0, 1.0, 0.15, 0.4);
        g.sensory_noise = perturb(rng, g.sensory_noise, 0.0, 0.5, 0.15, 0.4);
        g.al_inhibition = perturb(rng, g.al_inhibition, 0.0, 1.0, 0.15, 0.4);
        g.compass_noise = perturb(rng, g.compass_noise, 0.0, 1.0, 0.15, 0.4);
        g.compass_bias = perturb(rng, g.compass_bias, -1.0, 1.0, 0.15, 0.4);
        g.cx_motor_gain = perturb(rng, g.cx_motor_gain, 0.0, 2.0, 0.15, 0.4);
        g.motor_speed_gain = perturb(rng, g.motor_speed_gain, 0.2, 2.0, 0.15, 0.4);
        g.social_contact_gain = perturb(rng, g.social_contact_gain, 0.0, 1.0, 0.15, 0.4);
        g.mb_rpe_lr = perturb(rng, g.mb_rpe_lr, 0.0, 0.1, 0.15, 0.4);
        g.mb_eligibility_decay = perturb(rng, g.mb_eligibility_decay, 0.0, 1.0, 0.15, 0.4);
        g
    }

    /// BLX-α crossover producing one child.
    pub fn crossover(&self, other: &Self, rng: &mut impl Rng) -> Self {
        let r = ranges();
        let at = |i: usize| r[i].1;
        let a = 0.5;
        // ANN weights: BLX per element
        let n = ann_weight_count();
        let mut child_ann = vec![0.0; n];
        let s_len = self.ann_weights.len();
        let o_len = other.ann_weights.len();
        for i in 0..n {
            let sv = if i < s_len { self.ann_weights[i] } else { 0.0 };
            let ov = if i < o_len { other.ann_weights[i] } else { 0.0 };
            child_ann[i] = blx(rng, sv, ov, ANN_W_MIN, ANN_W_MAX, a);
        }
        // CPPN genes crossover
        let mut child_cppn = vec![0.0; CPPN_GENES];
        for i in 0..CPPN_GENES {
            let sv = self.cppn_genes.get(i).copied().unwrap_or(0.0);
            let ov = other.cppn_genes.get(i).copied().unwrap_or(0.0);
            child_cppn[i] = blx(rng, sv, ov, -1.0, 1.0, a);
        }
        // MB weights crossover (T8)
        let mbn = mb_weight_count();
        let mut child_mb = vec![0.0; mbn];
        for i in 0..mbn {
            let sv = self.mb_weights.get(i).copied().unwrap_or(0.0);
            let ov = other.mb_weights.get(i).copied().unwrap_or(0.0);
            child_mb[i] = blx(rng, sv, ov, ANN_W_MIN, ANN_W_MAX, a);
        }
        let child_dopamine_reward_gain = blx(
            rng,
            self.mb_dopamine_reward_gain,
            other.mb_dopamine_reward_gain,
            0.1,
            1.0,
            a,
        );
        let child_dopamine_punish_gain = blx(
            rng,
            self.mb_dopamine_punish_gain,
            other.mb_dopamine_punish_gain,
            0.1,
            1.0,
            a,
        );
        let child_satiety_gain = blx(
            rng,
            self.mb_satiety_gain,
            other.mb_satiety_gain,
            0.0,
            1.5,
            a,
        );
        let child_neurogenesis = blx(
            rng,
            self.mb_neurogenesis,
            other.mb_neurogenesis,
            0.2,
            3.0,
            a,
        );
        let child_cpg_freq = blx(rng, self.cpg_freq, other.cpg_freq, 0.5, 2.0, a);
        Genome {
            max_speed: blx(rng, self.max_speed, other.max_speed, at(0).0, at(0).1, a),
            turn_rate: blx(rng, self.turn_rate, other.turn_rate, at(1).0, at(1).1, a),
            food_radius: blx(
                rng,
                self.food_radius,
                other.food_radius,
                at(2).0,
                at(2).1,
                a,
            ),
            nest_radius: blx(
                rng,
                self.nest_radius,
                other.nest_radius,
                at(3).0,
                at(3).1,
                a,
            ),
            antenna_angle: blx(
                rng,
                self.antenna_angle,
                other.antenna_angle,
                at(4).0,
                at(4).1,
                a,
            ),
            antenna_dist: blx(
                rng,
                self.antenna_dist,
                other.antenna_dist,
                at(5).0,
                at(5).1,
                a,
            ),
            trail_threshold: blx(
                rng,
                self.trail_threshold,
                other.trail_threshold,
                at(6).0,
                at(6).1,
                a,
            ),
            alarm_threshold: blx(
                rng,
                self.alarm_threshold,
                other.alarm_threshold,
                at(7).0,
                at(7).1,
                a,
            ),
            explore_rate: blx(
                rng,
                self.explore_rate,
                other.explore_rate,
                at(8).0,
                at(8).1,
                a,
            ),
            follow_strength: blx(
                rng,
                self.follow_strength,
                other.follow_strength,
                at(9).0,
                at(9).1,
                a,
            ),
            aggression: blx(
                rng,
                self.aggression,
                other.aggression,
                at(10).0,
                at(10).1,
                a,
            ),
            carry_threshold: blx(
                rng,
                self.carry_threshold,
                other.carry_threshold,
                at(11).0,
                at(11).1,
                a,
            ),
            task_switch_threshold: blx(
                rng,
                self.task_switch_threshold,
                other.task_switch_threshold,
                at(12).0,
                at(12).1,
                a,
            ),
            trail_release_rate: blx(
                rng,
                self.trail_release_rate,
                other.trail_release_rate,
                at(13).0,
                at(13).1,
                a,
            ),
            trail_decay: blx(
                rng,
                self.trail_decay,
                other.trail_decay,
                at(14).0,
                at(14).1,
                a,
            ),
            alarm_release_rate: blx(
                rng,
                self.alarm_release_rate,
                other.alarm_release_rate,
                at(15).0,
                at(15).1,
                a,
            ),
            alarm_decay: blx(
                rng,
                self.alarm_decay,
                other.alarm_decay,
                at(16).0,
                at(16).1,
                a,
            ),
            home_release_rate: blx(
                rng,
                self.home_release_rate,
                other.home_release_rate,
                at(17).0,
                at(17).1,
                a,
            ),
            exploration_baseline: blx(
                rng,
                self.exploration_baseline,
                other.exploration_baseline,
                at(18).0,
                at(18).1,
                a,
            ),
            aggression_baseline: blx(
                rng,
                self.aggression_baseline,
                other.aggression_baseline,
                at(19).0,
                at(19).1,
                a,
            ),
            task_bias: blx(rng, self.task_bias, other.task_bias, at(20).0, at(20).1, a),
            trail_reinforcement: blx(
                rng,
                self.trail_reinforcement,
                other.trail_reinforcement,
                at(21).0,
                at(21).1,
                a,
            ),
            energy_drain: blx(
                rng,
                self.energy_drain,
                other.energy_drain,
                at(22).0,
                at(22).1,
                a,
            ),
            carry_cost: blx(
                rng,
                self.carry_cost,
                other.carry_cost,
                at(23).0,
                at(23).1,
                a,
            ),
            defend_cost: blx(
                rng,
                self.defend_cost,
                other.defend_cost,
                at(24).0,
                at(24).1,
                a,
            ),
            recharge_rate: blx(
                rng,
                self.recharge_rate,
                other.recharge_rate,
                at(25).0,
                at(25).1,
                a,
            ),
            cx_bump_gain: blx(
                rng,
                self.cx_bump_gain,
                other.cx_bump_gain,
                at(26).0,
                at(26).1,
                a,
            ),
            cx_inhibition: blx(
                rng,
                self.cx_inhibition,
                other.cx_inhibition,
                at(27).0,
                at(27).1,
                a,
            ),
            cx_compass_gain: blx(
                rng,
                self.cx_compass_gain,
                other.cx_compass_gain,
                at(28).0,
                at(28).1,
                a,
            ),
            cx_shift_gain: blx(
                rng,
                self.cx_shift_gain,
                other.cx_shift_gain,
                at(29).0,
                at(29).1,
                a,
            ),
            cx_hv_leak: blx(
                rng,
                self.cx_hv_leak,
                other.cx_hv_leak,
                at(30).0,
                at(30).1,
                a,
            ),
            ann_weights: child_ann,
            cppn_genes: child_cppn,
            mb_weights: child_mb,
            mb_dopamine_reward_gain: child_dopamine_reward_gain,
            mb_dopamine_punish_gain: child_dopamine_punish_gain,
            mb_satiety_gain: child_satiety_gain,
            mb_neurogenesis: child_neurogenesis,
            cpg_freq: child_cpg_freq,
            sensory_adapt_rate: blx(
                rng,
                self.sensory_adapt_rate,
                other.sensory_adapt_rate,
                0.0,
                0.5,
                a,
            ),
            sensory_adapt_strength: blx(
                rng,
                self.sensory_adapt_strength,
                other.sensory_adapt_strength,
                0.0,
                1.0,
                a,
            ),
            sensory_noise: blx(rng, self.sensory_noise, other.sensory_noise, 0.0, 0.5, a),
            al_inhibition: blx(rng, self.al_inhibition, other.al_inhibition, 0.0, 1.0, a),
            compass_noise: blx(rng, self.compass_noise, other.compass_noise, 0.0, 1.0, a),
            compass_bias: blx(rng, self.compass_bias, other.compass_bias, -1.0, 1.0, a),
            cx_motor_gain: blx(rng, self.cx_motor_gain, other.cx_motor_gain, 0.0, 2.0, a),
            motor_speed_gain: blx(
                rng,
                self.motor_speed_gain,
                other.motor_speed_gain,
                0.2,
                2.0,
                a,
            ),
            social_contact_gain: blx(
                rng,
                self.social_contact_gain,
                other.social_contact_gain,
                0.0,
                1.0,
                a,
            ),
            mb_rpe_lr: blx(rng, self.mb_rpe_lr, other.mb_rpe_lr, 0.0, 0.1, a),
            mb_eligibility_decay: blx(
                rng,
                self.mb_eligibility_decay,
                other.mb_eligibility_decay,
                0.0,
                1.0,
                a,
            ),
        }
    }

    /// Flattened, normalized trait vector for genotypic distance (niching, T2.4).
    ///
    /// It contains every evolvable scalar, followed by canonical-size ANN,
    /// CPPN, and MB vectors. Missing entries in a malformed legacy genome are
    /// zero-filled so distance remains symmetric and no inherited parameter is
    /// silently omitted by a length mismatch.
    pub fn trait_vec(g: &Genome) -> Vec<f32> {
        let r = ranges();
        let scalars = [
            g.max_speed,
            g.turn_rate,
            g.food_radius,
            g.nest_radius,
            g.antenna_angle,
            g.antenna_dist,
            g.trail_threshold,
            g.alarm_threshold,
            g.explore_rate,
            g.follow_strength,
            g.aggression,
            g.carry_threshold,
            g.task_switch_threshold,
            g.trail_release_rate,
            g.trail_decay,
            g.alarm_release_rate,
            g.alarm_decay,
            g.home_release_rate,
            g.exploration_baseline,
            g.aggression_baseline,
            g.task_bias,
            g.trail_reinforcement,
            g.energy_drain,
            g.carry_cost,
            g.defend_cost,
            g.recharge_rate,
            g.cx_bump_gain,
            g.cx_inhibition,
            g.cx_compass_gain,
            g.cx_shift_gain,
            g.cx_hv_leak,
        ];
        let mut v: Vec<f32> = scalars
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                let (lo, hi, _) = r[i].1;
                ((s - lo) / (hi - lo).max(1e-9)).clamp(0.0, 1.0)
            })
            .collect();

        // These neural scalars are evolvable outside `ranges()`, so normalize
        // them against the exact bounds used by mutate/crossover/in_range.
        v.extend([
            ((g.mb_dopamine_reward_gain - 0.1) / 0.9).clamp(0.0, 1.0),
            ((g.mb_dopamine_punish_gain - 0.1) / 0.9).clamp(0.0, 1.0),
            (g.mb_satiety_gain / 1.5).clamp(0.0, 1.0),
            ((g.mb_neurogenesis - 0.2) / 2.8).clamp(0.0, 1.0),
            ((g.cpg_freq - 0.5) / 1.5).clamp(0.0, 1.0),
            (g.sensory_adapt_rate / 0.5).clamp(0.0, 1.0),
            g.sensory_adapt_strength.clamp(0.0, 1.0),
            (g.sensory_noise / 0.5).clamp(0.0, 1.0),
            g.al_inhibition.clamp(0.0, 1.0),
            (g.compass_noise / 1.0).clamp(0.0, 1.0),
            ((g.compass_bias + 1.0) / 2.0).clamp(0.0, 1.0),
            (g.cx_motor_gain / 2.0).clamp(0.0, 1.0),
            ((g.motor_speed_gain - 0.2) / 1.8).clamp(0.0, 1.0),
            g.social_contact_gain.clamp(0.0, 1.0),
            (g.mb_rpe_lr / 0.1).clamp(0.0, 1.0),
            g.mb_eligibility_decay.clamp(0.0, 1.0),
        ]);

        // Fixed vector dimensions make each inherited neural weight contribute
        // equally and avoid the previous `min(len_a, len_b)` tail omission.
        for i in 0..ann_weight_count() {
            let w = g.ann_weights.get(i).copied().unwrap_or(0.0);
            v.push((w / 8.0).clamp(-1.0, 1.0));
        }
        for i in 0..CPPN_GENES {
            let gene = g.cppn_genes.get(i).copied().unwrap_or(0.0);
            v.push(gene.clamp(-1.0, 1.0));
        }
        for i in 0..mb_weight_count() {
            let w = g.mb_weights.get(i).copied().unwrap_or(0.0);
            v.push((w / 8.0).clamp(-1.0, 1.0));
        }
        v
    }

    /// Euclidean genotypic distance between two genomes (niching).
    pub fn distance(a: &Genome, b: &Genome) -> f64 {
        let va = Genome::trait_vec(a);
        let vb = Genome::trait_vec(b);
        debug_assert_eq!(va.len(), vb.len(), "trait vectors must be canonical-sized");
        let mut s = 0.0f64;
        for (av, bv) in va.iter().zip(vb.iter()) {
            let d = (*av - *bv) as f64;
            s += d * d;
        }
        s.sqrt() / (va.len().max(1) as f64).sqrt() // normalized per-dimension
    }

    /// Random genome within valid ranges (seeds the initial population in C2/C4).
    pub fn random(rng: &mut impl Rng) -> Self {
        let r = ranges();
        let mut g = Genome::default();
        g.max_speed = rng.gen_range(r[0].1 .0..=r[0].1 .1);
        g.turn_rate = rng.gen_range(r[1].1 .0..=r[1].1 .1);
        g.food_radius = rng.gen_range(r[2].1 .0..=r[2].1 .1);
        g.nest_radius = rng.gen_range(r[3].1 .0..=r[3].1 .1);
        g.antenna_angle = rng.gen_range(r[4].1 .0..=r[4].1 .1);
        g.antenna_dist = rng.gen_range(r[5].1 .0..=r[5].1 .1);
        g.trail_threshold = rng.gen_range(r[6].1 .0..=r[6].1 .1);
        g.alarm_threshold = rng.gen_range(r[7].1 .0..=r[7].1 .1);
        g.explore_rate = rng.gen_range(r[8].1 .0..=r[8].1 .1);
        g.follow_strength = rng.gen_range(r[9].1 .0..=r[9].1 .1);
        g.aggression = rng.gen_range(r[10].1 .0..=r[10].1 .1);
        g.carry_threshold = rng.gen_range(r[11].1 .0..=r[11].1 .1);
        g.task_switch_threshold = rng.gen_range(r[12].1 .0..=r[12].1 .1);
        g.trail_release_rate = rng.gen_range(r[13].1 .0..=r[13].1 .1);
        g.trail_decay = rng.gen_range(r[14].1 .0..=r[14].1 .1);
        g.alarm_release_rate = rng.gen_range(r[15].1 .0..=r[15].1 .1);
        g.alarm_decay = rng.gen_range(r[16].1 .0..=r[16].1 .1);
        g.home_release_rate = rng.gen_range(r[17].1 .0..=r[17].1 .1);
        g.exploration_baseline = rng.gen_range(r[18].1 .0..=r[18].1 .1);
        g.aggression_baseline = rng.gen_range(r[19].1 .0..=r[19].1 .1);
        g.task_bias = rng.gen_range(r[20].1 .0..=r[20].1 .1);
        g.trail_reinforcement = rng.gen_range(r[21].1 .0..=r[21].1 .1);
        g.energy_drain = rng.gen_range(r[22].1 .0..=r[22].1 .1);
        g.carry_cost = rng.gen_range(r[23].1 .0..=r[23].1 .1);
        g.defend_cost = rng.gen_range(r[24].1 .0..=r[24].1 .1);
        g.recharge_rate = rng.gen_range(r[25].1 .0..=r[25].1 .1);
        g.cx_bump_gain = rng.gen_range(r[26].1 .0..=r[26].1 .1);
        g.cx_inhibition = rng.gen_range(r[27].1 .0..=r[27].1 .1);
        g.cx_compass_gain = rng.gen_range(r[28].1 .0..=r[28].1 .1);
        g.cx_shift_gain = rng.gen_range(r[29].1 .0..=r[29].1 .1);
        g.cx_hv_leak = rng.gen_range(r[30].1 .0..=r[30].1 .1);
        let n = ann_weight_count();
        g.ann_weights = (0..n).map(|_| rng.gen_range(-1.0..=1.0)).collect();
        g.cppn_genes = (0..CPPN_GENES).map(|_| rng.gen_range(-1.0..=1.0)).collect();
        g.mb_weights = (0..mb_weight_count())
            .map(|_| rng.gen_range(-1.0..=1.0))
            .collect();
        g.mb_dopamine_reward_gain = rng.gen_range(0.2..=0.8);
        g.mb_dopamine_punish_gain = rng.gen_range(0.2..=0.8);
        g.mb_satiety_gain = rng.gen_range(0.0..=1.0);
        g.mb_neurogenesis = rng.gen_range(0.5..=2.0);
        g.cpg_freq = rng.gen_range(0.5..=2.0);
        g
    }

    /// Assert (panic-free) that every field is within its valid range. Used by
    /// the headless genome test and as a sanity gate after mutate/crossover.
    pub fn in_range(&self) -> bool {
        let r = ranges();
        let vals = [
            self.max_speed,
            self.turn_rate,
            self.food_radius,
            self.nest_radius,
            self.antenna_angle,
            self.antenna_dist,
            self.trail_threshold,
            self.alarm_threshold,
            self.explore_rate,
            self.follow_strength,
            self.aggression,
            self.carry_threshold,
            self.task_switch_threshold,
            self.trail_release_rate,
            self.trail_decay,
            self.alarm_release_rate,
            self.alarm_decay,
            self.home_release_rate,
            self.exploration_baseline,
            self.aggression_baseline,
            self.task_bias,
            self.trail_reinforcement,
            self.energy_drain,
            self.carry_cost,
            self.defend_cost,
            self.recharge_rate,
            self.cx_bump_gain,
            self.cx_inhibition,
            self.cx_compass_gain,
            self.cx_shift_gain,
            self.cx_hv_leak,
        ];
        vals.iter()
            .enumerate()
            .all(|(i, &v)| v >= r[i].1 .0 && v <= r[i].1 .1)
            && self.ann_weights.len() == ann_weight_count()
            && self
                .ann_weights
                .iter()
                .all(|&w| w >= ANN_W_MIN && w <= ANN_W_MAX)
            && self.cppn_genes.len() == CPPN_GENES
            && self.cppn_genes.iter().all(|&w| w >= -1.0 && w <= 1.0)
            && self.mb_weights.len() == mb_weight_count()
            && self
                .mb_weights
                .iter()
                .all(|&w| w >= ANN_W_MIN && w <= ANN_W_MAX)
            && self.mb_dopamine_reward_gain >= 0.1
            && self.mb_dopamine_reward_gain <= 1.0
            && self.mb_dopamine_punish_gain >= 0.1
            && self.mb_dopamine_punish_gain <= 1.0
            && self.mb_satiety_gain >= 0.0
            && self.mb_satiety_gain <= 1.5
            && self.mb_neurogenesis >= 0.2
            && self.mb_neurogenesis <= 3.0
            && self.cpg_freq >= 0.5
            && self.cpg_freq <= 2.0
            && self.sensory_adapt_rate >= 0.0
            && self.sensory_adapt_rate <= 0.5
            && self.sensory_adapt_strength >= 0.0
            && self.sensory_adapt_strength <= 1.0
            && self.sensory_noise >= 0.0
            && self.sensory_noise <= 0.5
            && self.al_inhibition >= 0.0
            && self.al_inhibition <= 1.0
            && self.compass_noise >= 0.0
            && self.compass_noise <= 1.0
            && self.compass_bias >= -1.0
            && self.compass_bias <= 1.0
            && self.cx_motor_gain >= 0.0
            && self.cx_motor_gain <= 2.0
            && self.motor_speed_gain >= 0.2
            && self.motor_speed_gain <= 2.0
            && self.social_contact_gain >= 0.0
            && self.social_contact_gain <= 1.0
            && self.mb_rpe_lr >= 0.0
            && self.mb_rpe_lr <= 0.1
            && self.mb_eligibility_decay >= 0.0
            && self.mb_eligibility_decay <= 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn rng() -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(123)
    }

    #[test]
    fn default_is_in_range() {
        assert!(Genome::default().in_range());
    }

    #[test]
    fn random_is_in_range() {
        let mut r = rng();
        for _ in 0..200 {
            assert!(Genome::random(&mut r).in_range());
        }
    }

    #[test]
    fn mutate_stays_in_range() {
        let mut r = rng();
        let mut g = Genome::default();
        for _ in 0..500 {
            g = g.mutate(&mut r);
            assert!(g.in_range(), "mutated genome out of range");
        }
    }

    #[test]
    fn crossover_stays_in_range() {
        let mut r = rng();
        let a = Genome::random(&mut r);
        let b = Genome::random(&mut r);
        for _ in 0..200 {
            let child = a.crossover(&b, &mut r);
            assert!(child.in_range());
        }
    }

    #[test]
    fn distance_identity_and_difference() {
        let g = Genome::default();
        assert!(Genome::distance(&g, &g) < 1e-9, "self-distance not ~0");
        let mut r = rng();
        let h = g.mutate(&mut r);
        assert!(
            Genome::distance(&g, &h) > 1e-6,
            "mutated genome has ~0 distance"
        );
    }

    #[test]
    fn distance_includes_every_mb_neural_parameter() {
        let base = Genome::default();
        let mut variants = Vec::new();

        let mut reward = base.clone();
        reward.mb_dopamine_reward_gain = 0.9;
        variants.push(("reward dopamine gain", reward));

        let mut punish = base.clone();
        punish.mb_dopamine_punish_gain = 0.9;
        variants.push(("punish dopamine gain", punish));

        let mut satiety = base.clone();
        satiety.mb_satiety_gain = 1.2;
        variants.push(("satiety gain", satiety));

        let mut neurogenesis = base.clone();
        neurogenesis.mb_neurogenesis = 2.0;
        variants.push(("neurogenesis", neurogenesis));

        let mut cpg = base.clone();
        cpg.cpg_freq = 1.5;
        variants.push(("CPG frequency", cpg));

        let mut mb_weight = base.clone();
        mb_weight.mb_weights[0] += 1.0;
        variants.push(("MB weight", mb_weight));

        for (name, variant) in variants {
            assert!(
                Genome::distance(&base, &variant) > 1e-6,
                "{name} must contribute to genotypic distance"
            );
        }
    }

    #[test]
    fn trait_vec_has_canonical_length_for_malformed_weight_vectors() {
        let base = Genome::default();
        let mut malformed = base.clone();
        malformed.ann_weights.truncate(1);
        malformed.cppn_genes.truncate(1);
        malformed.mb_weights.truncate(1);
        assert_eq!(
            Genome::trait_vec(&base).len(),
            Genome::trait_vec(&malformed).len()
        );
        assert!(Genome::distance(&base, &malformed) > 1e-6);
    }

    #[test]
    fn mutate_changes_something() {
        let mut r = rng();
        let g = Genome::default();
        // at least one of 20 mutations must differ from default
        let mut any_diff = false;
        for _ in 0..20 {
            let m = g.mutate(&mut r);
            if (m.follow_strength - g.follow_strength).abs() > 1e-6
                || (m.explore_rate - g.explore_rate).abs() > 1e-6
            {
                any_diff = true;
            }
        }
        assert!(any_diff, "mutation never changes key fields");
    }
}

/// T15: hand-designed *foraging seed* for the modular AL→MB→Output brain.
///
/// The MB's binary KCs (threshold → {0,1}) cannot encode a signed graded turn,
/// so the prior sign-splits each signed signal into L/R detector KCs and wires
/// them with opposite-sign readout weights — a bang-bang foraging controller:
///   • outbound (not carrying): steer toward Trail (input3 = (1-c)·ts·trail_gate),
///     lay Home pheromone (matches FSM's outbound Home marking).
///   • inbound (carrying): steer toward the nest (input15 = c·ns), lay Trail
///     pheromone (classic ACO — shorter return route gets denser Trail).
///   • no trail: a sign-split noise pair drives a symmetric random walk
///     (exploration) since the trail-gate zeroes the steer signal.
///
/// Detector KCs 0-9 are hand-wired (override the deterministic spread); KCs
/// 10-63 keep the deterministic `(k, k+4, k+8) mod 12` projection as the
/// learnable substrate STDP/evolution refines. The deterministic geometry is
/// preserved (no RNG), only the detector KCs' weights/biases are overridden.
pub fn mb_seed() -> Vec<f32> {
    let n = mb_weight_count();
    let mut w = vec![0.0; n];
    let al_in = MB_AL_INPUTS;
    let al_g = MB_AL_GLOM;
    let kc_n = MB_KC;
    let off_b_al = al_in * al_g;
    let off_w_lat = off_b_al + al_g;
    let off_w_kc = off_w_lat + al_g * al_g;
    let off_b_kc = off_w_kc + al_g * kc_n;
    let off_w_out = off_b_kc + kc_n;

    // --- W_al: glomerulus ← input assignments (foraging-relevant signals) ---
    // input layout (assembled in mb_decide): 0 trail_val, 1 home_val, 2 alarm_val,
    // 3 out_steer, 4 ts, 5 ns, 6 as_, 7 noise, 8-11 coss, 12 c, 13 energy,
    // 14 alarm_val, 15 in_steer, 16-19 vision/prox.
    let set_al = |w: &mut [f32], gl: usize, inp: usize, wt: f32| {
        w[gl * al_in + inp] = wt;
    };
    set_al(&mut w, 0, 3, 1.0); // glom0 ← out_steer (outbound trail steer, gated)
    set_al(&mut w, 1, 15, 1.0); // glom1 ← in_steer (inbound nest steer)
    set_al(&mut w, 2, 12, -1.0); // glom2 ← -c → tanh(b_al - c) = not-carrying detector
    w[off_b_al + 2] = 1.0; //   b_al so tanh(1-c): 0.76 when not carrying, 0 when carrying
    set_al(&mut w, 3, 12, 1.0); // glom3 ← c (carrying detector)
    set_al(&mut w, 4, 2, 1.0); // glom4 ← alarm_val (concentration)
    set_al(&mut w, 5, 6, 1.0); // glom5 ← as_ (alarm steer, signed)
    set_al(&mut w, 6, 1, 1.0); // glom6 ← home_val
    set_al(&mut w, 7, 7, 1.0); // glom7 ← noise (exploration drive)
                               // T10 multi-modal: vision + proximity converge onto glomeruli 8-11.
    set_al(&mut w, 8, 16, 1.0);
    set_al(&mut w, 8, 8, 1.0); // glom8 ← v_sin + trail_cos
    set_al(&mut w, 9, 17, 1.0);
    set_al(&mut w, 9, 9, 1.0); // glom9 ← v_cos + nest_cos
    set_al(&mut w, 10, 18, 1.0);
    set_al(&mut w, 10, 10, 1.0); // glom10 ← v_prox + alarm_cos
    set_al(&mut w, 11, 19, 1.0);
    set_al(&mut w, 11, 11, 1.0); // glom11 ← nest_prox + nc

    // --- W_lat: small uniform lateral inhibition (contrast / WTA) ---
    for i in 0..al_g {
        for j in 0..al_g {
            if i != j {
                w[off_w_lat + i * al_g + j] = 0.1;
            }
        }
    }

    // --- W_kc + b_kc: deterministic spread, then detector KC overrides ---
    for k in 0..kc_n {
        let g1 = k % al_g;
        let g2 = (k + 4) % al_g;
        let g3 = (k + 8) % al_g;
        w[off_w_kc + k * al_g + g1] = 1.0;
        w[off_w_kc + k * al_g + g2] = 1.0;
        w[off_w_kc + k * al_g + g3] = 1.0;
        w[off_b_kc + k] = 0.0;
    }
    // detector KC: read a single glom `gl` with signed weight `wt` + bias `bk`.
    // Clears the 3 deterministic connections first so the KC is a clean detector.
    let detector = |w: &mut [f32], k: usize, gl: usize, wt: f32, bk: f32| {
        for g in [k % al_g, (k + 4) % al_g, (k + 8) % al_g] {
            w[off_w_kc + k * al_g + g] = 0.0;
        }
        w[off_w_kc + k * al_g + gl] = wt;
        w[off_b_kc + k] = bk;
    };
    // sign-split steer detectors (W=±3, b=-0.3). These drive the binary
    // out0 readout / STDP substrate; the *behavioural* turn is a graded
    // AL→turn reflex (see mb_decide), so these thresholds only govern STDP
    // spike timing, not the live steering.
    detector(&mut w, 0, 0, 3.0, -0.3); // out_L: outbound trail to the left
    detector(&mut w, 1, 0, -3.0, -0.3); // out_R: outbound trail to the right
    detector(&mut w, 2, 1, 3.0, -0.3); // in_L:  inbound nest to the left
    detector(&mut w, 3, 1, -3.0, -0.3); // in_R:  inbound nest to the right
                                        // carrying-state detectors (strong signal → b=-0.2, fires reliably)
    detector(&mut w, 4, 2, 3.0, -0.2); // not_carrying
    detector(&mut w, 5, 3, 3.0, -0.2); // carrying
                                       // alarm steer detectors (sign-split)
    detector(&mut w, 6, 5, 3.0, -0.3); // alarm_L
    detector(&mut w, 7, 5, -3.0, -0.3); // alarm_R
                                        // noise detectors (sign-split → symmetric random walk when no trail)
    detector(&mut w, 8, 7, 3.0, -0.3); // noise_pos → turn one way
    detector(&mut w, 9, 7, -3.0, -0.3); // noise_neg → turn the other

    // --- W_out: readout (turn / dep_trail / dep_home / dep_alarm / attack) ---
    let set_out = |w: &mut [f32], o: usize, k: usize, wt: f32| {
        w[off_w_out + o * kc_n + k] = wt;
    };
    // out0 (turn): sign-split steer + noise. NOTE the live turn is a graded
    // AL→turn reflex in mb_decide (binary KCs can't do proportional steering);
    // this out0 wiring is kept as the STDP substrate so learning can refine
    // the innate reflex's pheromone-state coupling over a lifetime.
    set_out(&mut w, 0, 0, 2.0);
    set_out(&mut w, 0, 1, -2.0);
    set_out(&mut w, 0, 2, 2.0);
    set_out(&mut w, 0, 3, -2.0);
    set_out(&mut w, 0, 8, 0.4);
    set_out(&mut w, 0, 9, -0.4);
    // out1 (dep_trail): carrying → lay Trail on the return leg (ACO).
    set_out(&mut w, 1, 5, 1.0);
    // out2 (dep_home): not carrying → lay Home outbound (gradient back to nest).
    set_out(&mut w, 2, 4, 1.0);
    // out3 (dep_alarm) + out4 (attack): alarm either side → deposit + engage.
    set_out(&mut w, 3, 6, 1.0);
    set_out(&mut w, 3, 7, 1.0);
    set_out(&mut w, 4, 6, 1.0);
    set_out(&mut w, 4, 7, 1.0);

    w
}
