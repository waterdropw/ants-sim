//! Ant — body, internal state, FSM. `update` runs sense → brain → motion;
//! the brain (`brain.rs`) sets heading, transitions state and queues
//! pheromone deposits; motion + bounds reflection happen here.

pub mod brain;
pub mod sensors;

use crate::genome::Genome;
use crate::world::{Channel, World};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Module-level support for a motor decision. Positive and negative values are
/// allowed: action selection is an explicit competition, not call-order writes.
#[derive(Clone, Copy, Debug, Default)]
pub struct ActionEvidence {
    pub lh_turn: f32,
    pub mb_approach: f32,
    pub mb_avoidance: f32,
    pub cx_turn: f32,
    pub safety_turn: f32,
}

/// The sole command passed from brain-side computation to the body layer.
#[derive(Clone, Copy, Debug, Default)]
pub struct ActionCommand {
    pub turn_drive: f32,
    pub speed_drive: f32,
    pub deposit: [f32; 4],
    pub attack_drive: f32,
    pub task_switch_drive: f32,
    pub evidence: ActionEvidence,
}

/// Result of executing the previous command. It is body feedback rather than
/// a desired velocity, and can therefore be used safely by CX/learning later.
#[derive(Clone, Copy, Debug, Default)]
pub struct MotorFeedback {
    pub actual_turn: f32,
    pub actual_distance: f32,
    pub wall_contact: bool,
    pub energy_cost: f32,
    pub gait_phase: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Explore,
    FollowTrail,
    CarryReturn,
    Alarm,
    Defend,
    Nurse, // placeholder for later
}

#[derive(Clone, Debug)]
pub struct Ant {
    pub pos: crate::world::Vec2,
    pub heading: f32, // radians
    pub state: State,
    pub carrying: bool,
    pub energy: f32,
    /// health (T6.3 territorial war: reduced by enemy-colony ants)
    pub health: f32,
    /// path integration home vector (T3-gap3): accumulates displacement,
    /// points back to nest. Biologically: stride integration + sun compass.
    pub home_hx: f32,
    pub home_hy: f32,
    /// LIF membrane potentials (T3-gap9): hidden [10] + output [5], giving
    /// the network temporal memory (leaky integration) vs tanh's stateless
    /// forward pass.
    pub lif_v: [f32; 10], // hidden membrane potentials
    pub lif_v_out: [f32; 5], // output membrane potentials
    /// SNN-private learnable weight copy (T7.2 STDP), initialized from
    /// `genome.ann_weights` at birth and modified during life. The direct ANN
    /// intentionally reads the immutable genome weights and is an
    /// evolution-only baseline; the similarly shaped SNN reads this copy.
    pub learned_w: Vec<f32>,
    /// last tick each hidden/output neuron spiked (0 = never) for STDP.
    pub last_spike_h: [u32; 10],
    pub last_spike_o: [u32; 5],
    // T8 modular brain (AL→MB→Output) state
    pub mb_kc_v: Vec<f32>,       // KC membrane potentials (MB_KC=64)
    pub mb_out_v: [f32; 5],      // output membrane potentials
    pub mb_al_osc: f32,          // AL oscillation phase
    pub mb_kc_active: usize,     // T13 active KC count (grows with experience)
    pub learned_mb_w: Vec<f32>,  // STDP-modifiable MB weight copy
    pub last_kc_spike: Vec<u32>, // last spike tick per KC (for STDP)
    // T15: MB-private output-spike ticks (was reusing last_spike_o, shared with
    // the SNN STDP path — benign while one brain runs per ant, but the field
    // was not MB-specific; give MB its own to remove the cross-brain coupling).
    pub last_mb_out_spike: [u32; crate::genome::MB_OUT],
    // T16 dual-channel dopamine (PAM/PPL1 analog): reward (food→LTP) and
    // punish (damage→LTD). Both decay fast; gate MB + SNN STDP by sign.
    pub dopamine_reward: f32,
    pub dopamine_punish: f32,
    pub octopamine: f32, // foraging motivation (arousal)
    /// T22 ablation flags (per-ant, set by Simulator each tick for mechanisms
    /// reachable only from Ant::update / sensors). vision: skip visual sensing;
    /// stdp: skip lifetime synaptic plasticity.
    pub ablate_vision: bool,
    pub ablate_stdp: bool,
    /// Early-circuit / navigation ablations are propagated by Simulator.
    pub ablate_al_inhibition: bool,
    pub ablate_pn_multichannel: bool,
    pub ablate_lh_reflex: bool,
    pub ablate_orn: bool,
    pub ablate_compass: bool,
    pub ablate_cx_motor: bool,
    pub orn_adaptation: [f32; crate::ant::sensors::CHEM_CHANNELS],
    /// Social observations are populated only by the serial contact flush.
    pub contact_signal: f32,
    pub recruit_signal: f32,
    pub recruit_bearing: f32,
    /// RPE state is deliberately separate from dopamine pulses.
    pub value_estimate: f32,
    pub eligibility_trace: f32,
    pub last_rpe: f32,
    pub last_command: ActionCommand,
    pub motor_feedback: MotorFeedback,
    pub leg_phase: [f32; crate::genome::CPG_LEGS], // T14 CPG tripod gait phases
    // T9 central complex (CX): ring-attractor heading + neural path
    // integration. Active only in --brain cx mode (use_cx). The software
    // home_hx/home_hy vector is still maintained as ground-truth reference.
    pub use_cx: bool,
    pub cx_bump: [f32; crate::genome::CX_N], // ring-attractor activity (one bump)
    pub cx_hv_x: f32,                        // home-vector accumulator (CPU4 analog), x
    pub cx_hv_y: f32,                        // home-vector accumulator, y
    pub cx_prev_heading: f32,                // last tick's heading (for angular-velocity shift)
    pub age: u32,
    /// task-threshold heterogeneity seed (M4: foraging vs guarding lean)
    pub task_jitter: f32,
    pub genome: Genome,
    /// deposits queued during the (parallel) ant step; applied serially later.
    pub pending_deposits: Vec<(crate::world::Channel, i32, i32, f32)>,
    /// deliveries completed this tick; Simulator sums into `collected`.
    pub delivered: u32,
    /// per-ant RNG so the ant step can run in parallel (a shared &mut Rng
    /// can't cross threads). Seeded deterministically from index + world seed.
    pub rng: ChaCha8Rng,
    /// attacks queued during the parallel step (enemy idx, damage);
    /// applied serially by the Simulator, like pheromone deposits.
    pub pending_attacks: Vec<(usize, f32)>,
    /// brood tending queued this tick (Nurse state, T4.1)
    pub pending_brood: f32,
    /// colony id (T4.3 coevolution: two colonies share the world)
    pub colony_id: u8,
    /// food source picked up this tick (T4.3: depletes the source, enabling
    /// competition for limited food)
    pub pending_pickup: Option<usize>,
    /// which food source this ant is currently carrying from (for telemetry)
    pub carrying_from: Option<usize>,
    /// source index of the delivery completed this tick (telemetry)
    pub delivered_source: Option<usize>,
    /// cumulative deliveries over the ant's lifetime — individual-level
    /// fitness for multi-level selection (C5).
    pub total_delivered: u32,
    /// dead this tick (energy depleted) — removed by the Simulator flush.
    pub dead: bool,
    /// distance travelled since picking up food (CarryReturn). Used to fade
    /// the Trail deposit so it's densest near the food (giving outbound ants
    /// a gradient that points toward the food, not the nest).
    pub trip_dist: f32,
}

impl Ant {
    pub fn new(pos: crate::world::Vec2, heading: f32, genome: &Genome, rng_seed: u64) -> Self {
        Self {
            pos,
            heading,
            state: State::Explore,
            carrying: false,
            energy: 1.0,
            health: 1.0,
            home_hx: 0.0,
            home_hy: 0.0,
            lif_v: [0.0; 10],
            lif_v_out: [0.0; 5],
            learned_w: genome.ann_weights.clone(),
            last_spike_h: [0; 10],
            last_spike_o: [0; 5],
            mb_kc_v: vec![0.0; 64],
            mb_out_v: [0.0; 5],
            mb_al_osc: 0.0,
            mb_kc_active: crate::genome::MB_KC_INIT,
            learned_mb_w: genome.mb_weights.clone(),
            last_kc_spike: vec![0; 64],
            last_mb_out_spike: [0; crate::genome::MB_OUT],
            dopamine_reward: 0.0,
            dopamine_punish: 0.0,
            octopamine: 0.0,
            ablate_vision: false,
            ablate_stdp: false,
            ablate_al_inhibition: false,
            ablate_pn_multichannel: false,
            ablate_lh_reflex: false,
            ablate_orn: false,
            ablate_compass: false,
            ablate_cx_motor: false,
            orn_adaptation: [0.0; crate::ant::sensors::CHEM_CHANNELS],
            contact_signal: 0.0,
            recruit_signal: 0.0,
            recruit_bearing: 0.0,
            value_estimate: 0.0,
            eligibility_trace: 0.0,
            last_rpe: 0.0,
            last_command: ActionCommand::default(),
            motor_feedback: MotorFeedback::default(),
            leg_phase: [
                0.0,
                std::f32::consts::PI,
                0.0,
                std::f32::consts::PI,
                0.0,
                std::f32::consts::PI,
            ],
            use_cx: false,
            cx_bump: [1.0 / crate::genome::CX_N as f32; crate::genome::CX_N],
            cx_hv_x: 0.0,
            cx_hv_y: 0.0,
            cx_prev_heading: heading,
            age: 0,
            task_jitter: 0.0,
            genome: genome.clone(),
            pending_deposits: Vec::new(),
            pending_attacks: Vec::new(),
            pending_brood: 0.0,
            colony_id: 0,
            pending_pickup: None,
            carrying_from: None,
            delivered_source: None,
            total_delivered: 0,
            trip_dist: 0.0,
            delivered: 0,
            dead: false,
            rng: ChaCha8Rng::seed_from_u64(rng_seed),
        }
    }

    /// Home vector used for homing decisions. In CX mode (--brain cx) this
    /// is the neurally-integrated vector (ring attractor + accumulators);
    /// otherwise the software-accumulated ground truth (home_hx/home_hy).
    pub fn home_vector(&self) -> (f32, f32) {
        if self.use_cx {
            (self.cx_hv_x, self.cx_hv_y)
        } else {
            (self.home_hx, self.home_hy)
        }
    }

    /// T13: developmental neurogenesis — grow the MB by `kcs` KCs, capped at
    /// MB_KC. Called on reward events (delivery) so MB volume tracks foraging
    /// experience across the ant's lifetime (structural plasticity).
    pub fn grow_mb(&mut self, kcs: f32) {
        let max = crate::genome::MB_KC as f32;
        let grown = (self.mb_kc_active as f32 + kcs).clamp(crate::genome::MB_KC_INIT as f32, max);
        self.mb_kc_active = grown as usize;
    }

    /// T14: Central pattern generator for the stereotyped insect tripod gait.
    /// The phase RELATIONSHIP is hardwired (even legs 0,2,4 swing together,
    /// odd legs 1,3,5 anti-phase — the mutual-inhibition output of the CPG
    /// half-center circuit); the gait FREQUENCY `omega` is the dynamic,
    /// neuromodulated part (octopamine raises it → faster walking).
    /// Deterministic, guaranteed tripod (no spurious attractors).
    pub fn step_cpg(&mut self, omega: f32) {
        let tau = std::f32::consts::TAU;
        let master = (self.leg_phase[0] + omega).rem_euclid(tau);
        for i in 0..crate::genome::CPG_LEGS {
            let offset = if i % 2 == 0 { 0.0 } else { tau / 2.0 };
            self.leg_phase[i] = (master + offset).rem_euclid(tau);
        }
    }

    /// One tick: sense → decide (sets heading, transitions, deposits) → move.
    /// `brain_ann` selects the ANN decision layer (T2.2) over the FSM.
    pub fn update(
        &mut self,
        world: &World,
        brain_ann: bool,
        brain_snn: bool,
        brain_mb: bool,
        brain_cx: bool,
        brain_integrated: bool,
    ) {
        if self.dead {
            return;
        }
        self.age += 1;
        self.pending_deposits.clear();
        self.pending_attacks.clear();
        self.pending_brood = 0.0;
        self.pending_pickup = None;
        self.delivered_source = None;
        self.delivered = 0;
        // T11 neuromodulator dynamics (experience-driven, within-lifetime):
        // dopamine = reward signal (decays fast); octopamine = foraging arousal
        // (decays slow). Arousal rises during active search and falls while
        // returning with food or resting at nest — read by decide() to modulate
        // exploration vigor, trail responsiveness, and aggression.
        // T16: dual-channel dopamine (reward/punish) both decay fast.
        self.dopamine_reward *= 0.95;
        self.dopamine_punish *= 0.95;
        self.eligibility_trace = (self.eligibility_trace * self.genome.mb_eligibility_decay
            + 0.1
            + 0.2 * self.contact_signal)
            .clamp(0.0, 1.0);
        self.octopamine *= 0.99;
        let foraging = matches!(self.state, State::Explore | State::FollowTrail) && !self.carrying;
        if foraging {
            self.octopamine =
                (self.octopamine + crate::genome::OCT_FORAGE_GAIN).min(crate::genome::OCT_MAX);
        } else if self.carrying || self.state == State::Nurse {
            self.octopamine = (self.octopamine - crate::genome::OCT_REST_DECAY).max(0.0);
        }

        // CX is a reusable navigation submodule, not a controller-exclusive
        // storage replacement. Its action evidence is available to every brain
        // mode when the simulator enables it.
        self.use_cx = brain_cx || brain_integrated;
        let s = sensors::sense(self, world);
        let code = sensors::encode(self, &s);
        // Legacy controllers remain comparable, but their heading write is
        // captured and converted into an explicit descending action command.
        let decision_heading = self.heading;
        if brain_mb || brain_integrated {
            brain::mb_decide_with_code(self, &s, &code, world);
        } else if brain_snn {
            brain::snn_decide(self, &s, world);
        } else if brain_ann {
            brain::ann_decide(self, &s, world);
        } else {
            // T9: FSM framework with CX neural path integration — the FSM
            // reads its home vector through `home_vector()` (cx_hv when
            // use_cx), so only the dead-reckoning module is neuralized.
            brain::decide(self, &s, world);
        }
        let legacy_turn = brain::wrap_angle_public(self.heading - decision_heading);
        self.heading = decision_heading;
        // Adapt existing controller side effects into the one explicit command.
        // This preserves legacy behavior exactly while preventing the motor
        // layer from reading controller-private queues.
        let mut deposits = [0.0; 4];
        for (channel, _, _, amount) in self.pending_deposits.drain(..) {
            let slot = match channel {
                Channel::Trail => 0,
                Channel::Home => 1,
                Channel::Alarm => 2,
                Channel::Recruitment => 3,
            };
            deposits[slot] += amount;
        }
        let attack_drive = if self.pending_attacks.is_empty() {
            0.0
        } else {
            1.0
        };
        let attacks = std::mem::take(&mut self.pending_attacks);
        let mut command = ActionCommand {
            turn_drive: (legacy_turn / self.genome.turn_rate.max(1e-6)).clamp(-1.0, 1.0),
            speed_drive: 1.0,
            deposit: deposits,
            attack_drive,
            evidence: ActionEvidence {
                lh_turn: code.lh_turn,
                mb_approach: if brain_mb || brain_integrated {
                    self.mb_out_v[0]
                } else {
                    0.0
                },
                mb_avoidance: if brain_mb || brain_integrated {
                    self.mb_out_v[3]
                } else {
                    0.0
                },
                cx_turn: if self.use_cx {
                    command_cx_turn(self)
                } else {
                    0.0
                },
                ..Default::default()
            },
            ..Default::default()
        };
        // Preserve baseline behavior while exposing early-circuit evidence. The
        // CX-only controller already uses its decoded home vector in the FSM;
        // its independent readout is retained in telemetry for falsification.
        // The reusable CX output joins the descending competition explicitly.
        // It is gated on the same carry/home context as the existing homing
        // policy, preventing an empty-vector CX bump from steering explorers.
        if brain_integrated {
            // In the opt-in integrated mode, each source is an explicit term in
            // the descending competition. MB's legacy turn remains the learned
            // policy baseline; LH adds a fast prior and MBON outputs resolve
            // approach/avoidance conflict before the bounded motor command.
            command.turn_drive += 0.35 * command.evidence.lh_turn;
            command.turn_drive +=
                0.20 * (command.evidence.mb_approach - command.evidence.mb_avoidance).tanh();
        }
        if self.use_cx && self.carrying && !self.ablate_cx_motor {
            command.turn_drive += self.genome.cx_motor_gain * command.evidence.cx_turn;
        }
        command.turn_drive = command.turn_drive.clamp(-1.0, 1.0);
        self.heading += command.turn_drive * self.genome.turn_rate;
        let ix = self.pos.x as i32;
        let iy = self.pos.y as i32;
        for (slot, amount) in command.deposit.into_iter().enumerate() {
            if amount > 0.0 {
                let channel = match slot {
                    0 => Channel::Trail,
                    1 => Channel::Home,
                    2 => Channel::Alarm,
                    _ => Channel::Recruitment,
                };
                self.pending_deposits.push((channel, ix, iy, amount));
            }
        }
        if command.attack_drive > 0.0 {
            self.pending_attacks = attacks;
        }
        self.last_command = command;
        // accumulate individual fitness (multi-level selection hook)
        self.total_delivered += self.delivered;

        // energy economics (T2.1 genome-driven; T7.3 trophallaxis): drain by
        // activity, starve to death when depleted. Recharge is no longer free
        // — it happens in the Simulator flush via the shared colony_energy
        // pool (mouth-to-mouth food sharing). This means nest ants depend on
        // foragers bringing food back; without foraging, the colony starves.
        let g = &self.genome;
        let mut drain = g.energy_drain;
        if self.carrying {
            drain += g.carry_cost;
        }
        if matches!(self.state, State::Defend | State::Alarm) {
            drain += g.defend_cost;
        }
        self.energy -= drain;
        if self.energy <= 0.0 {
            self.energy = 0.0;
            self.dead = true;
            return;
        }

        // motion: Nurses stay at the nest tending brood (don't move).
        if self.state == State::Nurse {
            return;
        }
        // motion (single application per tick, regardless of state).
        // When the straight-ahead cell is a wall, probe candidate headings
        // to slide along the obstacle and find a gap. The sign is randomized
        // so blocked ants split up/down (or left/right) instead of always
        // trending one way — necessary for the two-bridge to get traffic on
        // both routes so pheromone can select the shorter.
        use rand::Rng;
        // T14: CPG gait — octopamine arousal raises gait frequency, which raises
        // walking speed (neuromodulator → locomotion, ties into T11). The leg
        // phases advance at `omega` (observable gait), speed scales with it.
        let omega =
            self.genome.cpg_freq * (1.0 + crate::genome::CPG_AROUSAL_GAIN * self.octopamine);
        let speed = self.genome.max_speed
            * (omega / self.genome.cpg_freq)
            * self.last_command.speed_drive.max(0.0)
            * self.genome.motor_speed_gain;
        self.step_cpg(omega);
        let old_x = self.pos.x;
        let old_y = self.pos.y;
        let old_heading = self.heading;
        let sign = if self.rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let mags = [0.5f32, 1.0, 1.5, 2.2];
        let mut deltas: Vec<f32> = vec![0.0];
        for &m in &mags {
            deltas.push(sign * m);
            deltas.push(-sign * m);
        }
        let mut moved = false;
        for &d in &deltas {
            let h = self.heading + d;
            let nx = self.pos.x + h.cos() * speed;
            let ny = self.pos.y + h.sin() * speed;
            if world.is_wall(nx, ny) {
                continue;
            }
            let (nx, ny, hd) = reflect_bounds(nx, ny, h, world.width, world.height);
            self.pos.x = nx;
            self.pos.y = ny;
            self.heading = hd;
            moved = true;
            break;
        }
        // path integration (T3-gap3): accumulate displacement into home vector.
        // At nest → reset (vector zeros). Biologically: stride integration
        // with sun-compass; has drift (no noise added here for simplicity).
        self.home_hx += self.pos.x - old_x;
        self.home_hy += self.pos.y - old_y;
        let dn = (world.nest.x - self.pos.x).hypot(world.nest.y - self.pos.y);
        if dn <= world.nest_radius {
            self.home_hx = 0.0;
            self.home_hy = 0.0;
        }
        // T9 CX path integration: shift the heading bump by the angular
        // velocity (post-motion heading includes decide-turns, wall slides
        // and bound reflections), then integrate the step into the neural
        // home vector. Runs after motion so the estimate uses the actual
        // travel direction of this tick.
        let actual_distance = (self.pos.x - old_x).hypot(self.pos.y - old_y);
        self.motor_feedback = MotorFeedback {
            actual_turn: brain::wrap_angle_public(self.heading - old_heading),
            actual_distance,
            wall_contact: !moved,
            energy_cost: drain,
            gait_phase: self.leg_phase[0],
        };
        if self.use_cx {
            brain::cx_integrate_observed(self, world, actual_distance, code.compass);
        }
        if !moved {
            // fully walled in: nudge heading so we don't deadlock
            self.heading += 0.7;
        }
    }
}

/// Read CX navigation state as a signed, bounded turning suggestion. This is
/// not applied implicitly: it is recorded inside `ActionCommand.evidence`.
fn command_cx_turn(ant: &Ant) -> f32 {
    let (hx, hy) = ant.home_vector();
    if hx.hypot(hy) < 1e-6 {
        return 0.0;
    }
    let target = (-hy).atan2(-hx);
    brain::wrap_angle_public(target - ant.heading) / std::f32::consts::PI
}

pub(crate) fn reflect_bounds(
    nx: f32,
    ny: f32,
    heading: f32,
    w: usize,
    h: usize,
) -> (f32, f32, f32) {
    let (mut nx, mut ny, mut hd) = (nx, ny, heading);
    if nx < 1.0 {
        nx = 1.0;
        hd = std::f32::consts::PI - hd;
    } else if nx >= (w - 1) as f32 {
        nx = (w - 2) as f32;
        hd = std::f32::consts::PI - hd;
    }
    if ny < 1.0 {
        ny = 1.0;
        hd = -hd;
    } else if ny >= (h - 1) as f32 {
        ny = (h - 2) as f32;
        hd = -hd;
    }
    (nx, ny, hd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Vec2;

    #[test]
    fn neurogenesis_grows_and_caps() {
        let g = Genome::default();
        let mut ant = Ant::new(Vec2::new(10.0, 10.0), 0.0, &g, 7);
        // immature MB at eclosion
        assert_eq!(ant.mb_kc_active, crate::genome::MB_KC_INIT);
        // growth below the ceiling increments by the requested amount
        ant.grow_mb(2.5);
        assert_eq!(ant.mb_kc_active, crate::genome::MB_KC_INIT + 2);
        // growth caps at MB_KC (never exceeds the full KC count)
        ant.grow_mb(1_000_000.0);
        assert_eq!(ant.mb_kc_active, crate::genome::MB_KC);
    }

    fn wrap_diff(a: f32, b: f32) -> f32 {
        let mut d = a - b;
        while d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        while d < -std::f32::consts::PI {
            d += std::f32::consts::TAU;
        }
        d
    }

    #[test]
    fn action_command_tracks_actual_motor_feedback() {
        let g = Genome::default();
        let world = World::new(64, 64, Vec2::new(32.0, 32.0), 4.0);
        let mut ant = Ant::new(Vec2::new(12.0, 12.0), 0.0, &g, 7);
        ant.update(&world, false, false, false, false, false);
        assert!(ant.last_command.turn_drive.is_finite());
        assert!(ant.motor_feedback.actual_distance.is_finite());
        assert!(ant.motor_feedback.actual_distance >= 0.0);
        assert!(ant.last_command.speed_drive >= 0.0);
    }

    #[test]
    fn cpg_produces_tripod_gait() {
        let g = Genome::default();
        let mut ant = Ant::new(Vec2::new(10.0, 10.0), 0.0, &g, 7);
        // start from arbitrary phases
        ant.leg_phase = [0.0, 0.3, 1.0, 2.0, 3.0, 4.0];
        for _ in 0..50 {
            ant.step_cpg(0.1);
        }
        // tripod: even legs (0,2,4) in phase; odd legs (1,3,5) in phase; the two
        // groups anti-phase (π apart).
        let d02 = wrap_diff(ant.leg_phase[0], ant.leg_phase[2]);
        let d04 = wrap_diff(ant.leg_phase[0], ant.leg_phase[4]);
        let d13 = wrap_diff(ant.leg_phase[1], ant.leg_phase[3]);
        let d01 = wrap_diff(ant.leg_phase[0], ant.leg_phase[1]);
        assert!(d02.abs() < 1e-4, "legs 0,2 not in phase: {d02}");
        assert!(d04.abs() < 1e-4, "legs 0,4 not in phase: {d04}");
        assert!(d13.abs() < 1e-4, "legs 1,3 not in phase: {d13}");
        assert!(
            (d01.abs() - std::f32::consts::PI).abs() < 1e-3,
            "tripod groups not anti-phase: {d01}"
        );
        // gait advances: master phase rotates as omega accumulates
        let p_before = ant.leg_phase[0];
        for _ in 0..10 {
            ant.step_cpg(0.1);
        }
        assert!(
            (wrap_diff(ant.leg_phase[0], p_before) - 1.0).abs() < 1e-3,
            "gait phase did not advance by omega·dt"
        );
    }
}
