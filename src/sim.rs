//! Simulator — owns world + ants + RNG, drives one fixed-timestep tick.

use crate::ant::{Ant, State};
use crate::genome::Genome;
use crate::world::spatial_hash::SpatialHash;
use crate::world::{Channel, Vec2, World};
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use std::collections::VecDeque;

/// A vertical counting gate placed on one arm of the two-bridge scenario.
/// A crossing is counted from an ant's actual movement segment, not from
/// pheromone concentration, so this is a traffic-flow measurement.
#[derive(Clone, Copy, Debug)]
pub struct BridgeGate {
    pub x: f32,
    pub y_min: f32,
    pub y_max: f32,
}

/// Cumulative movements through the short and long bridge gates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BridgeFlow {
    pub short_outbound: u64,
    pub long_outbound: u64,
    pub short_inbound: u64,
    pub long_inbound: u64,
}

impl BridgeFlow {
    pub fn inbound_total(self) -> u64 {
        self.short_inbound + self.long_inbound
    }

    pub fn short_inbound_fraction(self) -> Option<f32> {
        let total = self.inbound_total();
        (total > 0).then(|| self.short_inbound as f32 / total as f32)
    }
}

/// Grid-distance audit of the two bridge arms. These lengths are computed
/// against the same wall occupancy used by ant motion, with each route forced
/// to cross its corresponding counting gate exactly once on the outward leg.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BridgeRouteLengths {
    pub short_steps: u32,
    pub long_steps: u32,
}

impl BridgeRouteLengths {
    pub fn detour_ratio(self) -> f32 {
        self.long_steps as f32 / self.short_steps.max(1) as f32
    }
}

/// Time-local allocation of the model's observable behavior states. These are
/// action labels, not morphological castes or a direct species-level measure.
#[derive(Clone, Copy, Debug, Default)]
pub struct TaskFractions {
    pub foraging: f32,
    pub defense: f32,
    pub brood_care: f32,
}

/// Aggregate, fixed-size brain readout. It intentionally contains summaries
/// rather than per-ant activity arrays, so it can be sampled in long assays.
#[derive(Clone, Copy, Debug, Default)]
pub struct BrainTelemetry {
    pub ants: usize,
    pub mean_turn_drive: f32,
    pub mean_lh_turn: f32,
    pub mean_mb_approach: f32,
    pub mean_mb_avoidance: f32,
    pub mean_cx_turn: f32,
    pub mean_rpe: f32,
    pub mean_eligibility: f32,
}

impl BrainTelemetry {
    pub fn from_ants(ants: &[Ant]) -> Self {
        let mut telemetry = Self {
            ants: ants.len(),
            ..Self::default()
        };
        if ants.is_empty() {
            return telemetry;
        }
        for ant in ants {
            telemetry.mean_turn_drive += ant.last_command.turn_drive;
            telemetry.mean_lh_turn += ant.last_command.evidence.lh_turn;
            telemetry.mean_mb_approach += ant.last_command.evidence.mb_approach;
            telemetry.mean_mb_avoidance += ant.last_command.evidence.mb_avoidance;
            telemetry.mean_cx_turn += ant.last_command.evidence.cx_turn;
            telemetry.mean_rpe += ant.last_rpe;
            telemetry.mean_eligibility += ant.eligibility_trace;
        }
        let n = ants.len() as f32;
        telemetry.mean_turn_drive /= n;
        telemetry.mean_lh_turn /= n;
        telemetry.mean_mb_approach /= n;
        telemetry.mean_mb_avoidance /= n;
        telemetry.mean_cx_turn /= n;
        telemetry.mean_rpe /= n;
        telemetry.mean_eligibility /= n;
        telemetry
    }
}

impl TaskFractions {
    pub fn from_ants(ants: &[Ant]) -> Self {
        if ants.is_empty() {
            return Self::default();
        }
        let mut result = Self::default();
        for ant in ants {
            match ant.state {
                State::Explore | State::FollowTrail | State::CarryReturn => result.foraging += 1.0,
                State::Alarm | State::Defend => result.defense += 1.0,
                State::Nurse => result.brood_care += 1.0,
            }
        }
        let n = ants.len() as f32;
        result.foraging /= n;
        result.defense /= n;
        result.brood_care /= n;
        result
    }
}

pub struct Simulator {
    pub world: World,
    pub ants: Vec<Ant>,
    pub tick: u64,
    pub rng: ChaCha8Rng,
    pub paused: bool,
    /// food collected (colony-level metric)
    pub collected: f32,
    /// colony shared energy pool (T7.3 trophallaxis): foragers deposit here
    /// on delivery, nest ants draw from it — the "shared stomach."
    pub colony_energy: f32,
    /// per-colony collected (T4.3 coevolution)
    pub collected_a: f32,
    pub collected_b: f32,
    pub seed: u64,
    /// per-source delivery counts (telemetry for M3/M5)
    pub source_visits: Vec<u32>,
    /// enable ant-ant interaction bookkeeping (spatial hash rebuild each tick)
    pub interact: bool,
    /// use the fixed, evolution-only ANN decision layer (T2.2) instead of the FSM
    pub brain_ann: bool,
    /// use the LIF spiking decision layer (T3-gap9) with lifetime STDP
    pub brain_snn: bool,
    pub brain_mb: bool,
    /// FSM + central-complex (CX) neural path integration (T9)
    pub brain_cx: bool,
    /// Opt-in circuit composition: PN/LH + MB + CX compete through the shared
    /// ActionCommand interface. Legacy modes remain isolated baselines.
    pub brain_integrated: bool,
    /// territorial war mode (T6.3): ants attack enemy-colony ants
    pub war: bool,
    /// seasonal/ecological dynamics (T7.7): food regrows, predators raid
    pub seasonal: bool,
    /// Counterfactual perturbation flags used to measure model-internal
    /// sensitivity. They are abstract implementation switches, not one-to-one
    /// wet-lab interventions or evidence of biological sufficiency.
    pub ablate_octopamine: bool,
    pub ablate_trail: bool,
    pub ablate_eclosion: bool,
    /// T22: home-vector (path integration) ablation — zero home_hx/hy + cx_hv
    /// each tick so ants cannot path-integrate home (PI-lesion analog).
    pub ablate_homevector: bool,
    /// T22: reward-dopamine ablation — skip dopamine_reward release (PAM-silence
    /// analog; gates appetitive LTP off).
    pub ablate_reward: bool,
    /// T22: punish-dopamine ablation — skip dopamine_punish release (PPL1-silence
    /// analog; gates aversive LTD off).
    pub ablate_punish: bool,
    /// T22: vision ablation (blinding analog) — propagated to per-ant flag,
    /// sensors skips visual detection.
    pub ablate_vision: bool,
    /// T22: STDP ablation (plasticity-blockade analog) — propagated to per-ant
    /// flag, mb/snn skip lifetime synaptic plasticity.
    pub ablate_stdp: bool,
    /// Early sensory and social counterfactual switches. They are model-level
    /// implementation perturbations, not biological interventions.
    pub ablate_al_inhibition: bool,
    pub ablate_pn_multichannel: bool,
    pub ablate_lh_reflex: bool,
    pub ablate_orn: bool,
    pub ablate_compass: bool,
    pub ablate_cx_motor: bool,
    /// Disconnect octopamine from the CPG frequency/speed feedback while
    /// retaining the deterministic tripod oscillator itself.
    pub ablate_cpg_feedback: bool,
    pub ablate_contact: bool,
    /// Enables serial antenna-contact events without changing default baseline.
    pub social_contact: bool,
    /// Gates are registered only by the two-bridge scenario.
    pub bridge_gates: Option<[BridgeGate; 2]>,
    /// Cumulative actual ant traffic through the registered bridge gates.
    pub bridge_flow: BridgeFlow,
    pub spatial: SpatialHash,
}

impl Simulator {
    pub fn new(width: usize, height: usize, seed: u64, genome: &Genome) -> Self {
        let mut world = World::new(
            width,
            height,
            Vec2::new(width as f32 / 2.0, height as f32 / 2.0),
            6.0,
        );
        // The genome owns Trail's behavioral and chemical parameters. Keep the
        // field's Trail evaporation synchronized so `trail_decay` controls the
        // same signal that ants sense and follow (rather than a dormant gene).
        world.decay[World::ch_idx(Channel::Trail)] = genome.trail_decay;
        // default food source for the single-ant M2 loop
        world.food.push(crate::world::FoodSource {
            pos: Vec2::new(width as f32 * 0.8, height as f32 * 0.5),
            radius: 4.0,
            amount: 1_000_000.0, // effectively non-depleting; M3 steady state
        });

        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let n = 40; // M1/M2 colony size; tune via GUI in M3
        let mut ants = Vec::with_capacity(n);
        for i in 0..n {
            ants.push(spawn_ant(width, height, genome, &mut rng, seed, i as u64));
        }
        Self {
            world,
            ants,
            tick: 0,
            rng,
            paused: false,
            collected: 0.0,
            colony_energy: 0.0,
            collected_a: 0.0,
            collected_b: 0.0,
            seed,
            source_visits: Vec::new(),
            interact: false,
            brain_ann: false,
            brain_snn: false,
            brain_mb: false,
            brain_cx: false,
            brain_integrated: false,
            war: false,
            seasonal: false,
            ablate_octopamine: false,
            ablate_trail: false,
            ablate_eclosion: false,
            ablate_homevector: false,
            ablate_reward: false,
            ablate_punish: false,
            ablate_vision: false,
            ablate_stdp: false,
            ablate_al_inhibition: false,
            ablate_pn_multichannel: false,
            ablate_lh_reflex: false,
            ablate_orn: false,
            ablate_compass: false,
            ablate_cx_motor: false,
            ablate_cpg_feedback: false,
            ablate_contact: false,
            social_contact: false,
            bridge_gates: None,
            bridge_flow: BridgeFlow::default(),
            spatial: SpatialHash::new(width, height),
        }
    }

    pub fn brain_telemetry(&self) -> BrainTelemetry {
        BrainTelemetry::from_ants(&self.ants)
    }

    pub fn step(&mut self) {
        if self.paused {
            return;
        }
        // 1. chemistry (diffuse + evaporate) — writes its own buffers
        self.world.step_chemistry();

        // 2. ant step in parallel. Each ant owns its RNG, and deposits are
        //    queued (not applied inline) so there's no shared mutable field.
        //    Disjoint borrows of self.ants / self.world via a free fn.
        // Keep positions for serial bridge-gate flow accounting after movement.
        let previous_positions: Vec<Vec2> = self.ants.iter().map(|a| a.pos).collect();
        // T22 ablation: zero octopamine each tick so the arousal pathway can't
        // drive exploration/vigor/aggression/CPG-speed (OA-knockdown analog).
        if self.ablate_octopamine {
            for a in self.ants.iter_mut() {
                a.octopamine = 0.0;
            }
        }
        // T22 ablation: propagate per-ant ablation flags (vision / STDP) and
        // zero the path-integration home vector (PI-lesion analog).
        if self.ablate_homevector {
            for a in self.ants.iter_mut() {
                a.home_hx = 0.0;
                a.home_hy = 0.0;
                a.cx_hv_x = 0.0;
                a.cx_hv_y = 0.0;
            }
        }
        for a in self.ants.iter_mut() {
            a.ablate_vision = self.ablate_vision;
            a.ablate_stdp = self.ablate_stdp;
            a.ablate_octopamine = self.ablate_octopamine;
            a.ablate_al_inhibition = self.ablate_al_inhibition;
            a.ablate_pn_multichannel = self.ablate_pn_multichannel;
            a.ablate_lh_reflex = self.ablate_lh_reflex;
            a.ablate_orn = self.ablate_orn;
            a.ablate_compass = self.ablate_compass;
            a.ablate_cx_motor = self.ablate_cx_motor;
            a.ablate_cpg_feedback = self.ablate_cpg_feedback;
        }
        step_ants(
            &mut self.ants,
            &self.world,
            self.brain_ann,
            self.brain_snn,
            self.brain_mb,
            self.brain_cx,
            self.brain_integrated,
        );
        self.record_bridge_crossings(&previous_positions);

        // 3. serial flush: apply deposits + attacks + sum deliveries (race-free).
        if self.source_visits.len() < self.world.food.len() {
            self.source_visits.resize(self.world.food.len(), 0);
        }
        for a in self.ants.iter() {
            for (ch, x, y, amt) in &a.pending_deposits {
                // T22 ablation: skip Trail deposits (pheromone-disruption analog
                // — no recruitment trail, ACO can't form).
                if self.ablate_trail && *ch == Channel::Trail {
                    continue;
                }
                self.world.deposit(*ch, *x, *y, *amt);
            }
            for (idx, dmg) in &a.pending_attacks {
                if let Some(e) = self.world.enemies.get_mut(*idx) {
                    e.health -= *dmg;
                }
            }
            // T4.3: deplete food on pickup (enables competition for limited food)
            if let Some(i) = a.pending_pickup {
                if let Some(f) = self.world.food.get_mut(i) {
                    if f.amount > 0.0 {
                        f.amount -= 1.0;
                    }
                }
            }
            self.collected += a.delivered as f32;
            if a.delivered > 0 {
                match a.colony_id {
                    1 => self.collected_b += a.delivered as f32,
                    _ => self.collected_a += a.delivered as f32,
                }
                // trophallaxis (T7.3): delivery adds energy to shared pool
                self.colony_energy += a.delivered as f32 * 0.5;
            }
            if let Some(i) = a.delivered_source {
                if i < self.source_visits.len() {
                    self.source_visits[i] += 1;
                }
            }
        }
        // T16: reward-dopamine release on delivery/pickup (PAM analog → LTP).
        // T17: satiety modulation — effective reward gain scales with energy
        // deficit (hungry forager → bigger reward → stronger LTP). Centered
        // (energy=0.5 → base) so avg behavior ≈ Tier 16 baseline.
        // T13: delivery also triggers neurogenesis — the MB grows with
        // foraging experience (developmental structural plasticity).
        for a in self.ants.iter_mut() {
            let eff_reward = (a.genome.mb_dopamine_reward_gain
                * (1.0 + a.genome.mb_satiety_gain * (1.0 - 2.0 * a.energy)))
                .max(0.0);
            // T22 ablation: skip reward-dopamine release (PAM-silence analog).
            let eff_reward = if self.ablate_reward { 0.0 } else { eff_reward };
            if a.delivered > 0 {
                a.dopamine_reward =
                    (a.dopamine_reward + eff_reward).min(crate::genome::DOPAMINE_MAX);
                a.grow_mb(a.genome.mb_neurogenesis * a.delivered as f32);
            }
            if a.pending_pickup.is_some() {
                a.dopamine_reward =
                    (a.dopamine_reward + eff_reward * 0.5).min(crate::genome::DOPAMINE_MAX);
            }
        }
        // RPE update is deliberately after serial consequences (pickup,
        // delivery, injury) and before the next sensory decision. Eligibility
        // itself is accumulated by each ant during its parallel brain step.
        for a in self.ants.iter_mut() {
            let reward = a.delivered as f32 + if a.pending_pickup.is_some() { 0.5 } else { 0.0 };
            let punishment = if a.health <= 0.0 { 1.0 } else { 0.0 };
            let delta = reward - punishment - a.value_estimate;
            a.last_rpe = delta;
            a.value_estimate += a.genome.mb_rpe_lr * delta;
            let scaled = delta * a.eligibility_trace.clamp(0.0, 1.0);
            if scaled >= 0.0 && !self.ablate_reward {
                a.dopamine_reward = (a.dopamine_reward + scaled * a.genome.mb_dopamine_reward_gain)
                    .min(crate::genome::DOPAMINE_MAX);
            } else if scaled < 0.0 && !self.ablate_punish {
                a.dopamine_punish = (a.dopamine_punish
                    + -scaled * a.genome.mb_dopamine_punish_gain)
                    .min(crate::genome::DOPAMINE_MAX);
            }
        }
        // trophallaxis (T7.3): redistribute colony energy to nest ants.
        // Nest ants (within nest_radius) draw from the shared pool — modeling
        // mouth-to-mouth food sharing. Foragers deposit (above), nest-mates draw.
        if self.colony_energy > 0.0 && !self.ants.is_empty() {
            let nr2 = self.world.nest_radius * self.world.nest_radius;
            let nx = self.world.nest.x;
            let ny = self.world.nest.y;
            let rr = self.ants[0].genome.recharge_rate;
            let mut pool = self.colony_energy;
            for a in self.ants.iter_mut() {
                if pool <= 0.0 {
                    break;
                }
                let dx = a.pos.x - nx;
                let dy = a.pos.y - ny;
                if dx * dx + dy * dy <= nr2 && a.energy < 1.0 {
                    let transfer = rr.min(pool).min(1.0 - a.energy);
                    a.energy += transfer;
                    pool -= transfer;
                }
            }
            self.colony_energy = pool;
        }
        // territorial war (T6.3): melee — ants damage nearby enemy-colony ants.
        // T18.4: O(n²) pairwise scan replaced with the spatial hash (rebuild +
        // query_near within WAR_RANGE), so large war colonies scale O(n·k).
        // Gated by `war`.
        if self.war {
            const WAR_RANGE: f32 = 2.5;
            const WAR_DMG: f32 = 0.15;
            let n = self.ants.len();
            let pos: Vec<Vec2> = self.ants.iter().map(|a| a.pos).collect();
            let cols: Vec<u8> = self.ants.iter().map(|a| a.colony_id).collect();
            let deads: Vec<bool> = self.ants.iter().map(|a| a.dead).collect();
            self.spatial.rebuild(&pos);
            let mut dmg = vec![0.0f32; n];
            for i in 0..n {
                if deads[i] {
                    continue;
                }
                let pi = pos[i];
                let ci = cols[i];
                // count each enemy pair once (j > i); add WAR_DMG to both.
                self.spatial.query_near(&pos, pi, WAR_RANGE, |j, _d2| {
                    if j > i && !deads[j] && cols[j] != ci {
                        dmg[i] += WAR_DMG;
                        dmg[j] += WAR_DMG;
                    }
                });
            }
            for (i, a) in self.ants.iter_mut().enumerate() {
                if dmg[i] > 0.0 {
                    a.health -= dmg[i];
                    // T16: damage is the aversive US → release punish dopamine
                    // (PPL1 analog) so MB/SNN STDP does LTD on the KCs/hidden
                    // units active when the ant was hurt → avoidance learning.
                    // T22 ablation: skip punish release (PPL1-silence analog).
                    if !self.ablate_punish {
                        a.dopamine_punish = (a.dopamine_punish + a.genome.mb_dopamine_punish_gain)
                            .min(crate::genome::DOPAMINE_MAX);
                    }
                    if a.health <= 0.0 {
                        a.dead = true;
                    }
                }
            }
        }
        // remove dead enemies (indices were valid during the parallel step)
        self.world.enemies.retain(|e| e.health > 0.0);
        // remove starved ants (T1.2)
        self.ants.retain(|a| !a.dead);
        // brood stigmergy (T4.1): neglect decays, nurses' tending grows it.
        const BROOD_DECAY: f32 = 0.0025;
        self.world.brood *= 1.0 - BROOD_DECAY;
        let mut tend = 0.0f32;
        for a in self.ants.iter() {
            tend += a.pending_brood;
        }
        self.world.brood += tend;
        if self.world.brood < 0.0 {
            self.world.brood = 0.0;
        }
        // T7.5 brood-to-adult: brood ecloses into new ants when tended enough.
        // Each new ant consumes brood biomass and starts at the nest (age 0,
        // young → Nurse). Colony growth is gated by nursing + food (colony_energy
        // feeds nurses). Capped to prevent runaway growth/perf degradation.
        const BROOD_ECLOSION: f32 = 50.0;
        const MAX_COLONY: usize = 1000;
        // T22 ablation: skip eclosion (nurse-removal/brood-development analog
        // — brood never matures into adults, colony can't grow).
        while !self.ablate_eclosion
            && self.world.brood >= BROOD_ECLOSION
            && self.ants.len() < MAX_COLONY
        {
            self.world.brood -= BROOD_ECLOSION;
            let mut er = ChaCha8Rng::seed_from_u64(self.seed ^ (self.tick * 0xEC10));
            let g = &self.ants[0].genome;
            let na = spawn_ant(
                self.world.width,
                self.world.height,
                g,
                &mut er,
                self.seed,
                self.ants.len() as u64,
            );
            self.ants.push(na);
        }
        // T7.7 seasonal dynamics: food regrows (renewable resource) +
        // periodic predator raids (every 500 ticks near the nest).
        if self.seasonal {
            for f in self.world.food.iter_mut() {
                f.amount = (f.amount + 0.3).min(300.0); // regrowth, capped
            }
            if self.tick > 0 && self.tick.is_multiple_of(500) {
                let nest = self.world.nest;
                let th = (self.tick as f32 * 0.1).sin();
                let r = self.world.nest_radius + 15.0;
                self.world.enemies.push(crate::world::Enemy {
                    pos: crate::world::Vec2::new(nest.x + r * th.cos(), nest.y + r * th.sin()),
                    radius: 2.0,
                    health: 10.0,
                });
            }
        }
        // Serial social feedback: pairs are discovered through the spatial
        // hash, but no ant writes another ant's state during the parallel step.
        // A carrier offers a local recruitment direction to a same-colony peer.
        if self.social_contact && !self.ablate_contact {
            const CONTACT_RANGE: f32 = 2.0;
            let pos: Vec<Vec2> = self.ants.iter().map(|a| a.pos).collect();
            let colonies: Vec<u8> = self.ants.iter().map(|a| a.colony_id).collect();
            let carriers: Vec<bool> = self.ants.iter().map(|a| a.carrying).collect();
            self.spatial.rebuild(&pos);
            let mut events = Vec::new();
            for i in 0..pos.len() {
                self.spatial
                    .query_near(&pos, pos[i], CONTACT_RANGE, |j, _| {
                        if j > i && colonies[i] == colonies[j] {
                            let (offer, accept) = match (carriers[i], carriers[j]) {
                                (true, false) => (i, j),
                                (false, true) => (j, i),
                                _ => return,
                            };
                            let bearing =
                                (pos[offer].y - pos[accept].y).atan2(pos[offer].x - pos[accept].x);
                            events.push((offer, accept, bearing));
                        }
                    });
            }
            for a in self.ants.iter_mut() {
                a.contact_signal *= 0.8;
                a.recruit_signal *= 0.8;
            }
            for (offer, accept, bearing) in events {
                let gain = self.ants[offer].genome.social_contact_gain;
                self.ants[offer].contact_signal = 1.0;
                self.ants[accept].contact_signal = 1.0;
                self.ants[accept].recruit_signal =
                    (self.ants[accept].recruit_signal + gain).min(1.0);
                self.ants[accept].recruit_bearing = bearing;
            }
        } else {
            for a in self.ants.iter_mut() {
                a.contact_signal = 0.0;
                a.recruit_signal = 0.0;
            }
        }
        // ant-ant interaction bookkeeping (exercises spatial hash when enabled)
        if self.interact {
            let pos: Vec<Vec2> = self.ants.iter().map(|a| a.pos).collect();
            self.spatial.rebuild(&pos);
        }

        self.tick += 1;
    }

    /// Audit the shortest traversable grid route for each registered arm.
    /// This verifies the geometry independently of ant behavior and pheromone
    /// levels, so a traffic preference cannot be attributed to a mislabeled
    /// "short" gate.
    pub fn bridge_route_lengths(&self) -> Option<BridgeRouteLengths> {
        let [short_gate, long_gate] = self.bridge_gates?;
        let food = self.world.food.first()?;
        let short_steps =
            shortest_path_through_gate(&self.world, self.world.nest, food.pos, short_gate)?;
        let long_steps =
            shortest_path_through_gate(&self.world, self.world.nest, food.pos, long_gate)?;
        Some(BridgeRouteLengths {
            short_steps,
            long_steps,
        })
    }

    /// Record each line-segment crossing through a registered bridge gate.
    /// The strict half-open direction test prevents repeated counts when an ant
    /// lands on a gate line for more than one tick.
    fn record_bridge_crossings(&mut self, previous_positions: &[Vec2]) {
        let Some([short_gate, long_gate]) = self.bridge_gates else {
            return;
        };
        for (before, ant) in previous_positions.iter().zip(&self.ants) {
            if crosses_gate(*before, ant.pos, short_gate) {
                if ant.pos.x > before.x {
                    self.bridge_flow.short_outbound += 1;
                } else {
                    self.bridge_flow.short_inbound += 1;
                }
            }
            if crosses_gate(*before, ant.pos, long_gate) {
                if ant.pos.x > before.x {
                    self.bridge_flow.long_outbound += 1;
                } else {
                    self.bridge_flow.long_inbound += 1;
                }
            }
        }
    }

    pub fn reset(&mut self, seed: u64, genome: &Genome) {
        let w = self.world.width;
        let h = self.world.height;
        *self = Self::new(w, h, seed, genome);
    }

    /// Replace the colony size (re-spawns ants around the nest).
    pub fn set_colony_size(&mut self, n: usize, genome: &Genome) {
        let mut rng = ChaCha8Rng::seed_from_u64(self.seed ^ 0xA5A5);
        let mut ants = Vec::with_capacity(n);
        for i in 0..n as u64 {
            ants.push(spawn_ant(
                self.world.width,
                self.world.height,
                genome,
                &mut rng,
                self.seed,
                i,
            ));
        }
        let center = Vec2::new(
            self.world.width as f32 / 2.0,
            self.world.height as f32 / 2.0,
        );
        let dx = self.world.nest.x - center.x;
        let dy = self.world.nest.y - center.y;
        for ant in &mut ants {
            ant.pos.x += dx;
            ant.pos.y += dy;
        }
        self.ants = ants;
    }

    /// T4.3: spawn two colonies (genome_a id 0, genome_b id 1) sharing the
    /// world, competing for the same (limited) food.
    pub fn set_two_colonies(&mut self, n: usize, genome_a: &Genome, genome_b: &Genome) {
        let mut rng = ChaCha8Rng::seed_from_u64(self.seed ^ 0xC0DE);
        let mut ants = Vec::with_capacity(n);
        for i in 0..n as u64 {
            let (g, id) = if i % 2 == 0 {
                (genome_a, 0u8)
            } else {
                (genome_b, 1u8)
            };
            let mut a = spawn_ant(
                self.world.width,
                self.world.height,
                g,
                &mut rng,
                self.seed,
                i,
            );
            a.colony_id = id;
            ants.push(a);
        }
        let center = Vec2::new(
            self.world.width as f32 / 2.0,
            self.world.height as f32 / 2.0,
        );
        let dx = self.world.nest.x - center.x;
        let dy = self.world.nest.y - center.y;
        for ant in &mut ants {
            ant.pos.x += dx;
            ant.pos.y += dy;
        }
        self.ants = ants;
    }

    /// Spawn a colony where each ant's genome is drawn (with a light mutation)
    /// from a `pool` — used by multi-level selection (C5), where ants with
    /// different genotypes compete within one colony and the best carriers
    /// propagate.
    pub fn set_colony_from_pool(&mut self, pool: &[Genome], n: usize, rng: &mut ChaCha8Rng) {
        use rand::Rng;
        let mut ants = Vec::with_capacity(n);
        for i in 0..n as u64 {
            let base = &pool[rng.gen_range(0..pool.len())];
            let g = base.mutate(rng);
            ants.push(spawn_ant(
                self.world.width,
                self.world.height,
                &g,
                rng,
                self.seed,
                i,
            ));
        }
        self.ants = ants;
    }
    /// from `base` — heterogeneous genotypes within one colony. This is the
    /// phase-2 stand-in for genetic variation that selection acts on.
    pub fn set_colony_diverse(&mut self, n: usize, base: &Genome) {
        let mut rng = ChaCha8Rng::seed_from_u64(self.seed ^ 0xA5A5);
        let mut ants = Vec::with_capacity(n);
        for i in 0..n as u64 {
            let g = base.mutate(&mut rng);
            ants.push(spawn_ant(
                self.world.width,
                self.world.height,
                &g,
                &mut rng,
                self.seed,
                i,
            ));
        }
        self.ants = ants;
    }

    /// Apply an environment preset (clears food/enemies/walls/field, sets new).
    pub fn apply_environment(&mut self, env: &crate::environment::Environment) {
        self.world.food = env.foods.clone();
        self.world.enemies = env.enemies.clone();
        self.world.walls = env.walls.clone();
        self.world.field.clear();
        self.collected = 0.0;
        self.tick = 0;
        self.source_visits.clear();
        self.bridge_gates = None;
        self.bridge_flow = BridgeFlow::default();
    }

    /// T1.4 symmetric two-bridge: ONE food on the direct line, a thin vertical
    /// wall with two symmetric gaps (50/50 fork at the nest), PLUS a second
    /// wall block that forces the bottom route to detour far down. So both
    /// routes get traffic but the top is much shorter → pheromone amplifies it
    /// (classic ACO two-bridge).
    pub fn scenario_two_bridge(&mut self) {
        let w = self.world.width as f32;
        let h = self.world.height as f32;
        self.world.nest = Vec2::new(40.0, h * 0.5);
        self.world.food.clear();
        self.world.food.push(crate::world::FoodSource {
            pos: Vec2::new(w - 40.0, h * 0.5),
            radius: 4.0,
            amount: 1_000_000.0,
        });
        self.world.walls.clear();
        // wall 1: vertical, symmetric gaps 24 above/below the direct line
        self.world.walls.push(crate::world::Wall {
            x0: w * 0.30,
            y0: h * 0.5 - 24.0,
            x1: w * 0.32,
            y1: h * 0.5 + 24.0,
        });
        // wall 2: block south of the bottom gap → bottom route must detour
        // down to y>0.80h, across, then back up (long). Top route is clear.
        self.world.walls.push(crate::world::Wall {
            x0: w * 0.34,
            y0: h * 0.5 + 24.0,
            x1: w * 0.66,
            y1: h * 0.80,
        });
        self.world.field.clear();
        self.collected = 0.0;
        self.tick = 0;
        self.source_visits.clear();
        // Gates sit downstream of the long-arm obstacle, not immediately
        // after the fork. A lower-gap ant could otherwise climb back above the
        // obstacle before crossing the old gate plane, contaminating the
        // alleged long-arm traffic with a hybrid shortcut. At x=0.67w the
        // upper gate is reachable only through the short arm, while the lower
        // gate is reachable only after the bottom detour below wall 2.
        self.bridge_gates = Some([
            BridgeGate {
                x: w * 0.67,
                y_min: 0.0,
                y_max: h * 0.5 - 24.0,
            },
            BridgeGate {
                x: w * 0.67,
                y_min: h * 0.80,
                y_max: h,
            },
        ]);
        self.bridge_flow = BridgeFlow::default();
    }

    /// M3 two-source selection: two food sources in divergent directions at
    /// different distances. With gradient-ascent trail following, outbound
    /// ants at the nest climb whichever trail is denser; the nearer source
    /// (shorter round trip → denser Trail) recruits more → positive feedback.
    /// The colony should concentrate foraging on the near source.
    pub fn scenario_two_branch(&mut self) {
        let w = self.world.width as f32;
        let h = self.world.height as f32;
        let nest = self.world.nest;
        self.world.walls.clear();
        self.world.food.clear();
        // near: straight right, ~51 cells
        self.world.food.push(crate::world::FoodSource {
            pos: Vec2::new(nest.x + w * 0.20, nest.y),
            radius: 4.0,
            amount: 1_000_000.0,
        });
        // far: straight up, ~98 cells, divergent & mid-field (not a corner)
        self.world.food.push(crate::world::FoodSource {
            pos: Vec2::new(nest.x, nest.y - h * 0.38),
            radius: 4.0,
            amount: 1_000_000.0,
        });
        self.world.field.clear();
        self.collected = 0.0;
        self.tick = 0;
        self.source_visits.clear();
        self.bridge_gates = None;
        self.bridge_flow = BridgeFlow::default();
    }

    /// M4: drop an enemy near the nest to trigger alarm + collective defense.
    pub fn spawn_enemy_near_nest(&mut self) {
        let nest = self.world.nest;
        let mut rng = ChaCha8Rng::seed_from_u64(self.seed ^ self.tick);
        let th: f32 = rng.gen_range(0.0f32..std::f32::consts::TAU);
        let r = self.world.nest_radius + 8.0;
        self.world.enemies.push(crate::world::Enemy {
            pos: Vec2::new(nest.x + r * th.cos(), nest.y + r * th.sin()),
            radius: 2.0,
            health: 20.0,
        });
    }

    /// M2 default scenario: single food source to the right of the nest.
    pub fn scenario_single(&mut self) {
        let w = self.world.width as f32;
        self.world.food.clear();
        self.world.food.push(crate::world::FoodSource {
            pos: Vec2::new(self.world.nest.x + w * 0.3, self.world.nest.y),
            radius: 4.0,
            amount: 1_000_000.0,
        });
        self.world.field.clear();
        self.collected = 0.0;
        self.tick = 0;
        self.bridge_gates = None;
        self.bridge_flow = BridgeFlow::default();
    }
}

/// Find the shortest 4-neighbor route from `start` to `goal` while requiring
/// its left-to-right crossing of the gate plane to lie within `gate`'s span.
/// The gate is downstream of the fork; requiring this crossing makes each
/// returned length arm-specific while reusing `World::is_wall` occupancy.
fn shortest_path_through_gate(
    world: &World,
    start: Vec2,
    goal: Vec2,
    gate: BridgeGate,
) -> Option<u32> {
    let width = world.width;
    let height = world.height;
    let start_x = start.x.round().clamp(0.0, (width - 1) as f32) as usize;
    let start_y = start.y.round().clamp(0.0, (height - 1) as f32) as usize;
    let goal_x = goal.x.round().clamp(0.0, (width - 1) as f32) as usize;
    let goal_y = goal.y.round().clamp(0.0, (height - 1) as f32) as usize;
    let cell_count = width * height;
    let mut distances = vec![u32::MAX; cell_count * 2];
    let mut queue = VecDeque::new();
    let start_state = (start_y * width + start_x) * 2;
    distances[start_state] = 0;
    queue.push_back((start_x, start_y, false));

    while let Some((x, y, crossed_gate)) = queue.pop_front() {
        let state = (y * width + x) * 2 + usize::from(crossed_gate);
        let distance = distances[state];
        if crossed_gate && x == goal_x && y == goal_y {
            return Some(distance);
        }
        for (dx, dy) in [(1_isize, 0_isize), (-1, 0), (0, 1), (0, -1)] {
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx < 0 || ny < 0 || nx >= width as isize || ny >= height as isize {
                continue;
            }
            let nx = nx as usize;
            let ny = ny as usize;
            if world.is_wall(nx as f32, ny as f32) {
                continue;
            }
            let crosses_plane = (x as f32 <= gate.x && nx as f32 > gate.x)
                || (x as f32 > gate.x && nx as f32 <= gate.x);
            let in_gate_span = y as f32 >= gate.y_min && y as f32 <= gate.y_max;
            // Any crossing of the gate plane must use this arm's gate. This
            // prevents a route from selecting the other fork and merely
            // wandering across the plane later to satisfy the state flag.
            if crosses_plane && !in_gate_span {
                continue;
            }
            let crossed_gate = crossed_gate || crosses_plane;
            let next_state = (ny * width + nx) * 2 + usize::from(crossed_gate);
            if distances[next_state] == u32::MAX {
                distances[next_state] = distance + 1;
                queue.push_back((nx, ny, crossed_gate));
            }
        }
    }
    None
}

fn crosses_gate(before: Vec2, after: Vec2, gate: BridgeGate) -> bool {
    let dx = after.x - before.x;
    if dx.abs() <= f32::EPSILON {
        return false;
    }
    let crosses_outbound = before.x < gate.x && after.x >= gate.x;
    let crosses_inbound = before.x > gate.x && after.x <= gate.x;
    if !crosses_outbound && !crosses_inbound {
        return false;
    }
    let t = (gate.x - before.x) / dx;
    let y = before.y + t * (after.y - before.y);
    y >= gate.y_min && y <= gate.y_max
}

fn spawn_ant(
    w: usize,
    h: usize,
    genome: &Genome,
    rng: &mut ChaCha8Rng,
    world_seed: u64,
    index: u64,
) -> Ant {
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    let r: f32 = rng.gen_range(0.0f32..6.0f32);
    let th: f32 = rng.gen_range(0.0f32..std::f32::consts::TAU);
    let pos = Vec2::new(cx + r * th.cos(), cy + r * th.sin());
    let heading: f32 = rng.gen_range(0.0f32..std::f32::consts::TAU);
    // per-ant RNG: deterministic mix of world seed and ant index
    let ant_seed = world_seed
        .wrapping_mul(0x9E3779B97F4A7C15)
        .wrapping_add(index.wrapping_mul(0x632BE59BD01B5C35));
    let mut ant = Ant::new(pos, heading, genome, ant_seed);
    // Individual age-transition heterogeneity; this is not itself a measured
    // caste proportion. Task allocation is measured from live State labels.
    ant.task_jitter = rng.gen_range(-0.4..=1.0);
    ant
}

fn step_ants(
    ants: &mut [Ant],
    world: &World,
    brain_ann: bool,
    brain_snn: bool,
    brain_mb: bool,
    brain_cx: bool,
    brain_integrated: bool,
) {
    ants.par_iter_mut().for_each(|a| {
        a.update(
            world,
            brain_ann,
            brain_snn,
            brain_mb,
            brain_cx,
            brain_integrated,
        )
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Channel;

    fn sim_two(seed: u64) -> Simulator {
        let g = Genome::default();
        let mut s = Simulator::new(96, 96, seed, &g);
        s.set_colony_size(60, &g);
        s.scenario_single();
        s
    }

    #[test]
    fn serial_contact_offer_is_delivered_next_tick() {
        let g = Genome::default();
        let mut s = Simulator::new(64, 64, 2, &g);
        let mut offer = Ant::new(Vec2::new(20.0, 20.0), 0.0, &g, 1);
        let mut accept = Ant::new(Vec2::new(20.5, 20.0), 0.0, &g, 2);
        offer.carrying = true;
        offer.age = 1_000;
        accept.age = 1_000;
        s.ants = vec![offer, accept];
        s.social_contact = true;
        s.step();
        assert!(s.ants.iter().any(|a| a.recruit_signal > 0.0));
        assert!(s.ants.iter().all(|a| a.contact_signal > 0.0));
    }

    #[test]
    fn genome_trail_decay_configures_trail_chemistry() {
        let g = Genome {
            trail_decay: 0.037,
            ..Genome::default()
        };
        let simulator = Simulator::new(64, 64, 1, &g);
        assert!(
            (simulator.world.decay[World::ch_idx(Channel::Trail)] - g.trail_decay).abs()
                < f32::EPSILON,
            "Trail field decay must come from the genome"
        );
    }

    #[test]
    fn bridge_gate_counts_segment_crossings_once_by_direction() {
        let gate = BridgeGate {
            x: 10.0,
            y_min: 2.0,
            y_max: 4.0,
        };
        assert!(crosses_gate(
            Vec2::new(9.0, 3.0),
            Vec2::new(11.0, 3.0),
            gate
        ));
        assert!(crosses_gate(
            Vec2::new(11.0, 3.0),
            Vec2::new(9.0, 3.0),
            gate
        ));
        assert!(!crosses_gate(
            Vec2::new(9.0, 5.0),
            Vec2::new(11.0, 5.0),
            gate
        ));
        assert!(!crosses_gate(
            Vec2::new(10.0, 3.0),
            Vec2::new(11.0, 3.0),
            gate
        ));
        assert!(!crosses_gate(
            Vec2::new(9.0, 3.0),
            Vec2::new(9.0, 4.0),
            gate
        ));
    }

    #[test]
    fn two_bridge_geometry_audit_confirms_short_arm_is_shorter() {
        let g = Genome::default();
        let mut simulator = Simulator::new(128, 128, 4, &g);
        simulator.scenario_two_bridge();
        let lengths = simulator
            .bridge_route_lengths()
            .expect("both bridge arms should have a traversable route");
        assert!(
            lengths.short_steps < lengths.long_steps,
            "short arm {} must be shorter than long arm {}",
            lengths.short_steps,
            lengths.long_steps
        );
        assert!(lengths.detour_ratio() > 1.0);
    }

    #[test]
    fn task_fractions_use_live_states_not_task_jitter() {
        let mut simulator = sim_two(12);
        simulator.ants.truncate(6);
        let states = [
            State::Explore,
            State::FollowTrail,
            State::CarryReturn,
            State::Alarm,
            State::Defend,
            State::Nurse,
        ];
        for (ant, state) in simulator.ants.iter_mut().zip(states) {
            ant.state = state;
            ant.task_jitter = 123.0;
        }
        let tasks = TaskFractions::from_ants(&simulator.ants);
        assert!((tasks.foraging - 0.5).abs() < 1e-6);
        assert!((tasks.defense - 1.0 / 3.0).abs() < 1e-6);
        assert!((tasks.brood_care - 1.0 / 6.0).abs() < 1e-6);
    }

    #[test]
    fn two_bridge_registers_disjoint_arm_gates() {
        let g = Genome::default();
        let mut simulator = Simulator::new(128, 128, 4, &g);
        simulator.scenario_two_bridge();
        let [short, long] = simulator
            .bridge_gates
            .expect("two bridge should register gates");
        assert_eq!(short.x, long.x);
        assert!(short.y_max < simulator.world.height as f32 * 0.5);
        assert!(long.y_min > simulator.world.height as f32 * 0.5);
        assert!(short.y_max < long.y_min);
    }

    #[test]
    fn determinism_two_runs_equal() {
        let mut a = sim_two(7);
        let mut b = sim_two(7);
        for _ in 0..120 {
            a.step();
            b.step();
        }
        assert!(
            (a.collected - b.collected).abs() < 1e-6,
            "collected differs"
        );
        assert_eq!(a.ants.len(), b.ants.len(), "alive count differs");
    }

    #[test]
    fn integrated_brain_is_deterministic_and_reports_evidence() {
        let mut a = sim_two(17);
        let mut b = sim_two(17);
        a.brain_integrated = true;
        b.brain_integrated = true;
        for _ in 0..120 {
            a.step();
            b.step();
        }
        let ta = a.brain_telemetry();
        let tb = b.brain_telemetry();
        assert!((a.collected - b.collected).abs() < 1e-6);
        assert_eq!(a.ants.len(), b.ants.len());
        assert!(ta.mean_turn_drive.is_finite());
        assert!(ta.mean_lh_turn.is_finite());
        assert!(ta.mean_mb_approach.is_finite());
        assert!(ta.mean_cx_turn.is_finite());
        assert!((ta.mean_turn_drive - tb.mean_turn_drive).abs() < 1e-6);
    }

    #[test]
    fn brood_never_negative() {
        let mut s = sim_two(3);
        for _ in 0..300 {
            s.step();
            assert!(s.world.brood >= 0.0, "brood went negative");
        }
    }

    #[test]
    fn field_never_negative() {
        let mut s = sim_two(9);
        for _ in 0..200 {
            s.step();
        }
        for ch in [
            Channel::Trail,
            Channel::Home,
            Channel::Alarm,
            Channel::Recruitment,
        ] {
            for &v in s.world.field.channel_slice(ch) {
                assert!(v >= 0.0, "field cell negative");
            }
        }
    }

    #[test]
    fn coevolve_within_food_budget() {
        let g = Genome::default();
        let gb = g.mutate(&mut rand_chacha::ChaCha8Rng::seed_from_u64(1));
        let mut s = Simulator::new(96, 96, 5, &g);
        s.set_colony_size(60, &g);
        s.scenario_single();
        for f in s.world.food.iter_mut() {
            f.amount = 50.0; // limited
        }
        let budget: f32 = s.world.food.iter().map(|f| f.amount).sum();
        s.set_two_colonies(60, &g, &gb);
        for _ in 0..400 {
            s.step();
        }
        let total = s.collected_a + s.collected_b;
        assert!(
            total <= budget + 1.0,
            "collected {total} exceeds budget {budget}"
        );
    }

    /// T16: food delivery/pickup releases reward dopamine (PAM analog).
    #[test]
    fn reward_dopamine_released_on_foraging() {
        let g = Genome::default();
        let mut s = Simulator::new(96, 96, 11, &g);
        s.brain_mb = true;
        s.set_colony_size(60, &g);
        s.scenario_single();
        let mut found = false;
        for _ in 0..400 {
            s.step();
            if s.ants.iter().any(|a| a.dopamine_reward > 0.0) {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "no ant ever released reward dopamine (no delivery/pickup?)"
        );
    }

    /// T16: war damage releases punish dopamine (PPL1 analog).
    #[test]
    fn punish_dopamine_released_on_war_damage() {
        let g = Genome::default();
        let gb = g.mutate(&mut rand_chacha::ChaCha8Rng::seed_from_u64(1));
        let mut s = Simulator::new(96, 96, 5, &g);
        s.brain_mb = true;
        s.war = true;
        s.set_two_colonies(40, &g, &gb);
        let mut found = false;
        for _ in 0..600 {
            s.step();
            if s.ants.iter().any(|a| a.dopamine_punish > 0.0) {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "no ant ever released punish dopamine (no war damage?)"
        );
    }

    /// T16: reward dopamine gates LTP. With reward_gain=0.5, foraging drives
    /// LTP → some KC→out weight rises above the innate seed. With reward_gain=0,
    /// LTP is gated off → no weight rises above seed (only baseline LTD).
    #[test]
    fn mb_reward_dopamine_enables_ltp() {
        let seed = Genome::default().mb_weights.clone();
        let run = |reward_gain: f32| -> f32 {
            let g = Genome {
                mb_dopamine_reward_gain: reward_gain,
                ..Genome::default()
            };
            let mut s = Simulator::new(96, 96, 11, &g);
            s.brain_mb = true;
            s.set_colony_size(60, &g);
            s.scenario_single();
            for _ in 0..500 {
                s.step();
            }
            let kc = crate::genome::MB_KC;
            let out = crate::genome::MB_OUT;
            let al_in = crate::genome::MB_AL_INPUTS;
            let al_g = crate::genome::MB_AL_GLOM;
            let off_w_out = al_in * al_g + al_g + al_g * al_g + al_g * kc + kc;
            let mut max_rise = 0.0f32;
            for a in &s.ants {
                for o in 0..out {
                    for k in 0..kc {
                        let idx = off_w_out + o * kc + k;
                        let d = a.learned_mb_w[idx] - seed[idx];
                        if d > max_rise {
                            max_rise = d;
                        }
                    }
                }
            }
            max_rise
        };
        let with_reward = run(0.5);
        let no_reward = run(0.0);
        assert!(
            with_reward > 0.0,
            "reward dopamine should drive LTP: {}",
            with_reward
        );
        assert!(
            with_reward > no_reward,
            "reward should enable more LTP than no-reward: {} vs {}",
            with_reward,
            no_reward
        );
    }

    /// T17: satiety modulation — a hungry forager (low energy) releases more
    /// reward dopamine on delivery than a sated one (high energy), same genome.
    #[test]
    fn satiety_modulates_reward_dopamine() {
        let g = Genome::default();
        let mk = |energy: f32| -> f32 {
            let mut s = Simulator::new(64, 64, 1, &g);
            s.brain_mb = true;
            let mut ant = crate::ant::Ant::new(crate::world::Vec2::new(32.0, 32.0), 0.0, &g, 1);
            ant.carrying = true;
            ant.energy = energy;
            ant.age = 1000; // past nurse_age → foraging logic, not nursing
            s.ants = vec![ant];
            s.step();
            s.ants[0].dopamine_reward
        };
        let hungry = mk(0.1);
        let sated = mk(1.0);
        assert!(
            hungry > sated,
            "hungry forager should release more reward dopamine: {} vs {}",
            hungry,
            sated
        );
    }

    /// T17: AL→KC (W_kc) Hebbian plasticity, reward-gated on substrate KCs.
    /// With reward dopamine, some substrate KC's AL→KC weight rises above the
    /// seed; with reward_gain=0 (gate off) no W_kc weight rises.
    #[test]
    fn mb_wkc_plasticity_reward_ltp() {
        let seed = Genome::default().mb_weights.clone();
        let run = |reward_gain: f32| -> f32 {
            let g = Genome {
                mb_dopamine_reward_gain: reward_gain,
                ..Genome::default()
            };
            let mut s = Simulator::new(96, 96, 11, &g);
            s.brain_mb = true;
            s.set_colony_size(60, &g);
            s.scenario_single();
            for _ in 0..600 {
                s.step();
            }
            let kc = crate::genome::MB_KC;
            let al_g = crate::genome::MB_AL_GLOM;
            let al_in = crate::genome::MB_AL_INPUTS;
            let det = crate::genome::MB_DETECTOR_KCS;
            let off_w_kc = al_in * al_g + al_g + al_g * al_g;
            let mut max_rise = 0.0f32;
            for a in &s.ants {
                for k in det..kc {
                    for gl in 0..al_g {
                        let idx = off_w_kc + k * al_g + gl;
                        let d = a.learned_mb_w[idx] - seed[idx];
                        if d > max_rise {
                            max_rise = d;
                        }
                    }
                }
            }
            max_rise
        };
        let with_reward = run(0.5);
        let no_reward = run(0.0);
        assert!(
            with_reward > 0.0,
            "reward should drive W_kc LTP: {}",
            with_reward
        );
        assert!(
            with_reward > no_reward,
            "reward W_kc LTP {} should exceed no-reward {}",
            with_reward,
            no_reward
        );
    }
}
