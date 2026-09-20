//! Brain — subsumption-style FSM. Outbound ants do **chemotaxis**: they steer
//! toward the bearing of maximum Trail concentration (laid by returners,
//! fading from food→nest so it's densest at the food → the gradient points
//! toward the food). At a fork this means the denser (shorter, faster) branch
//! recruits more — the ACO shortest-path mechanism.
//!
//! `decide` only sets heading, transitions state and queues deposits; motion
//! + bounds + wall-sliding happen in `Ant::update`.

use crate::ant::sensors::{Sensing, SensoryCode};
use crate::ant::{Ant, State};
use crate::genome::{ann_weight_count, ANN_HID, ANN_IN, ANN_OUT};
use crate::world::{Channel, World};
use rand::Rng;

// Brood stigmergy (T4.1): nurses add biomass, neglect starves it.
const BROOD_TEND: f32 = 0.04; // per nurse per tick
const BROOD_HEALTHY: f32 = 30.0; // nurses leave once brood is well-tended

// Age polyethism (T7.4): caste depends on age, not just birth jitter.
// Young → Nurse, middle → Forager, old → Guard. Individual variation via
// task_jitter (some mature earlier/later) prevents synchronized transition.
const NURSE_AGE_BASE: f32 = 200.0; // ticks
const GUARD_AGE_BASE: f32 = 600.0;

/// Steer `heading` toward `target` by at most `max_turn` radians.
fn steer_toward(heading: f32, target: f32, max_turn: f32) -> f32 {
    let mut diff = target - heading;
    while diff > std::f32::consts::PI {
        diff -= std::f32::consts::TAU;
    }
    while diff < -std::f32::consts::PI {
        diff += std::f32::consts::TAU;
    }
    heading + diff.clamp(-max_turn, max_turn)
}

pub fn decide(ant: &mut Ant, s: &Sensing, world: &World) {
    let g = &ant.genome;
    let ix = ant.pos.x as i32;
    let iy = ant.pos.y as i32;

    // T7.4 age polyethism: caste by age (young→nurse, mid→forager, old→guard)
    // with individual variation via task_jitter (±jitter*100 ticks on thresholds).
    let nurse_age = NURSE_AGE_BASE + ant.task_jitter * 100.0;
    let guard_age = GUARD_AGE_BASE + ant.task_jitter * 200.0;
    let is_guard = (ant.age as f32) >= guard_age;
    let is_young = (ant.age as f32) < nurse_age;
    let alarm_thresh = if is_guard {
        g.alarm_threshold * 0.4
    } else {
        g.alarm_threshold * 2.0
    };
    // T11 neuromodulator coupling: octopamine (foraging arousal) widens
    // exploration, lowers the trail-response threshold (aroused foragers are
    // more responsive to food cues), and boosts aggression (fight-or-flight).
    let arousal = ant.octopamine.clamp(0.0, crate::genome::OCT_MAX);
    let trail_thresh = g.trail_threshold * (1.0 - 0.5 * arousal);
    let explore = g.explore_rate * (1.0 + arousal);
    // `follow_strength` is an evolvable chemotaxis gain. Normalize it around
    // the historical default (3.0), preserving default behavior while letting
    // evolution and bridge sensitivity assays vary trail responsiveness.
    let trail_turn_scale = (0.2 + 0.1 * g.follow_strength).clamp(0.2, 1.0);
    let aggressive = g.aggression + g.aggression_baseline + 0.5 * arousal > 0.5;

    // ---- priority 1: direct enemy contact → alarm + engage ----
    if let Some(idx) = s.enemy_idx {
        let e = &world.enemies[idx];
        let target = (e.pos.y - ant.pos.y).atan2(e.pos.x - ant.pos.x);
        ant.heading = steer_toward(ant.heading, target, g.turn_rate);
        ant.pending_deposits
            .push((Channel::Alarm, ix, iy, g.alarm_release_rate));
        ant.state = State::Defend;
        if s.enemy_dist <= g.antenna_dist + 1.0 {
            ant.pending_attacks.push((idx, 1.0));
        }
        return;
    }

    // ---- priority 2: recruited by alarm pheromone → converge ----
    // No relay deposit: alarm is only laid by ants in direct enemy contact
    // (priority 1), so the cloud decays once enemies are killed — prevents a
    // self-sustaining defense lockup that would starve the colony.
    if !ant.carrying && s.alarm_val > alarm_thresh && aggressive {
        ant.heading = steer_toward(ant.heading, s.alarm_bearing, g.turn_rate);
        ant.state = State::Alarm;
        return;
    }

    match ant.state {
        State::Explore => {
            // wander with exploration noise.
            ant.heading += ant.rng.gen_range(-explore..=explore);
            // T23 fix: GRADED trail bias (soft steer, not a hard state-switch
            // climb). The FSM's dedicated FollowTrail hard-climb over-converged
            // all ants onto one source/path (bottleneck → trail net-harmful,
            // the +44% ablation artifact). The ANN — where trail is a graded
            // steering input blended with noise — gets trail BENEFICIAL. This
            // mirrors that: trail softly biases heading (half turn_rate) while
            // exploration noise persists, sustaining dispersal to multiple
            // sources. Real ants follow trails with error, not deterministically.
            if s.trail_val > trail_thresh {
                ant.heading =
                    steer_toward(ant.heading, s.trail_bearing, g.turn_rate * trail_turn_scale);
            }
            // outbound: mark the home path (denser near nest → gradient to nest)
            ant.pending_deposits
                .push((Channel::Home, ix, iy, g.home_release_rate * 0.05));
            // T7.6 vision: if food visible ahead, steer toward it
            if let Some(vi) = s.vision_food_idx {
                let fp = world.food[vi].pos;
                let target = (fp.y - ant.pos.y).atan2(fp.x - ant.pos.x);
                ant.heading = steer_toward(ant.heading, target, g.turn_rate);
            }
            // young ants (age < nurse_age) stay at nest to tend brood (T7.4)
            if s.at_nest && is_young {
                ant.state = State::Nurse;
            }
            if let Some(idx) = s.food_idx {
                if !ant.carrying {
                    ant.carrying = true;
                    ant.carrying_from = Some(idx);
                    ant.pending_pickup = Some(idx);
                    ant.pending_deposits.push((
                        Channel::Recruitment,
                        ix,
                        iy,
                        g.trail_release_rate * 0.5,
                    ));
                    ant.trip_dist = 0.0;
                    ant.state = State::CarryReturn;
                }
            }
        }

        State::FollowTrail => {
            // climb Trail gradient toward the food, with trail-following error
            // (real ants imperfectly follow trails). T23 fix: the small noise
            // + occasional drop-to-Explore sustains an exploring fraction so
            // multiple sources get exploited (avoids single-path bottleneck).
            if s.trail_val > g.trail_threshold * 0.3 && !ant.rng.gen_bool(0.03) {
                ant.heading =
                    steer_toward(ant.heading, s.trail_bearing, g.turn_rate * trail_turn_scale);
                ant.heading += ant.rng.gen_range(-explore * 0.3..=explore * 0.3);
            } else {
                ant.state = State::Explore;
            }
            ant.pending_deposits
                .push((Channel::Home, ix, iy, g.home_release_rate * 0.05));
            if let Some(idx) = s.food_idx {
                if !ant.carrying {
                    ant.carrying = true;
                    ant.carrying_from = Some(idx);
                    ant.pending_pickup = Some(idx);
                    ant.pending_deposits.push((
                        Channel::Recruitment,
                        ix,
                        iy,
                        g.trail_release_rate * 0.5,
                    ));
                    ant.trip_dist = 0.0;
                    ant.state = State::CarryReturn;
                }
            }
        }

        State::CarryReturn => {
            ant.trip_dist += g.max_speed;
            // path integration (T3-gap3): follow the home vector (accumulated
            // displacement, reset at nest), not a direct bearing to the nest.
            // Biologically plausible dead reckoning with drift.
            // T9: `home_vector()` returns the CX neurally-integrated vector
            // in --brain cx mode, the software ground truth otherwise.
            let (hx, hy) = ant.home_vector();
            let target = (-hy).atan2(-hx);
            ant.heading = steer_toward(ant.heading, target, g.turn_rate);
            // uniform deposit along the return path (classic ACO): the
            // shorter route gets laid more frequently → denser → recruits more.
            ant.pending_deposits
                .push((Channel::Trail, ix, iy, g.trail_release_rate));
            if s.at_nest {
                ant.carrying = false;
                ant.delivered_source = ant.carrying_from;
                ant.carrying_from = None;
                ant.trip_dist = 0.0;
                ant.delivered = ant.delivered.saturating_add(1);
                // face outward (away from nest) to re-enter the trail corridor
                ant.heading = (ant.pos.y - world.nest.y).atan2(ant.pos.x - world.nest.x);
                ant.state = State::FollowTrail;
            }
        }

        State::Alarm => {
            // climb the alarm gradient toward the threat; no relay (see above).
            if s.alarm_val > alarm_thresh {
                ant.heading = steer_toward(ant.heading, s.alarm_bearing, g.turn_rate);
            } else {
                ant.state = State::Explore;
            }
        }

        State::Defend => {
            if s.alarm_val > alarm_thresh {
                ant.heading = steer_toward(ant.heading, s.alarm_bearing, g.turn_rate);
            } else {
                ant.state = State::Explore;
            }
        }

        State::Nurse => {
            // T7.4: nurse while young; mature to forager when age > nurse_age.
            if !is_young {
                ant.state = State::Explore;
            } else {
                ant.pending_brood += BROOD_TEND;
            }
        }
    }
}

/// Wrap an angle difference to (-π, π].
fn wrap_angle(mut d: f32) -> f32 {
    while d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    while d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    d
}

/// Public angle helper used by the body-side action executor.
pub fn wrap_angle_public(d: f32) -> f32 {
    wrap_angle(d)
}

/// T9 central complex (CX): ring-attractor heading update + neural path
/// integration. Called once per tick AFTER motion (from `Ant::update`) so
/// the angular velocity reflects the actual travel direction (decide-turns,
/// wall slides, bound reflections all included).
///
/// Ring attractor: CX_N heading units with preferred angles θᵢ = 2πi/N.
/// Each tick the activity bump is (a) shifted by the angular velocity
/// Δh·cx_shift_gain through a shifted cosine kernel (recurrent excitation),
/// (b) contrasted by global lateral inhibition (winner-take-all → one bump),
/// (c) weakly anchored to the true heading by a sun-compass injection
/// (polarized-light compass analog — bounds drift), then rectified and
/// divisively normalized. Pure f32, fully deterministic.
///
/// Path integration: the bump's decoded heading H_est replaces the true
/// heading for stride integration — the home vector accumulates
/// dist·(cos H_est, sin H_est) with a small leak, and zeros at the nest.
pub fn cx_integrate(ant: &mut Ant, world: &World, dist: f32) {
    // Compatibility numeric baseline used by legacy tests/benchmarks. Runtime
    // code calls `cx_integrate_observed` with an explicit compass observation.
    let compass = crate::ant::sensors::CompassObservation {
        bearing: ant.heading,
        confidence: 1.0,
        available: true,
    };
    cx_integrate_observed(ant, world, dist, compass);
}

/// CX ring update driven by a sensory compass observation plus actual executed
/// distance. The body heading is used only for angular self-motion shift; it is
/// never injected as compass evidence.
pub fn cx_integrate_observed(
    ant: &mut Ant,
    world: &World,
    dist: f32,
    compass: crate::ant::sensors::CompassObservation,
) {
    let g = &ant.genome;
    let n = crate::genome::CX_N;
    let tau = std::f32::consts::TAU;
    let dh = ant.motor_feedback.actual_turn;
    ant.cx_prev_heading = ant.heading;
    let shift = g.cx_shift_gain * dh;
    let compass_gain = if compass.available {
        g.cx_compass_gain * compass.confidence
    } else {
        0.0
    };
    let compass_bearing = compass.bearing;

    let mut newb = [0.0f32; crate::genome::CX_N];
    for i in 0..n {
        let th_i = tau * i as f32 / n as f32;
        let mut acc = 0.0f32;
        for j in 0..n {
            let th_j = tau * j as f32 / n as f32;
            let d = wrap_angle(th_i - th_j - shift);
            // cosine ring kernel: local excitation, global inhibition
            let k = g.cx_bump_gain * d.cos() - g.cx_inhibition;
            acc += k * ant.cx_bump[j];
        }
        // No true-heading leak: this injection is solely the noisy/occluded
        // CompassObservation made by the sensory layer.
        acc += compass_gain * (th_i - compass_bearing).cos().max(0.0);
        newb[i] = acc.max(0.0);
    }
    // divisive normalization (stable total activity → single bump)
    let sum: f32 = newb.iter().sum();
    if sum > 1e-9 {
        for v in newb.iter_mut() {
            *v /= sum;
        }
    } else {
        for v in newb.iter_mut() {
            *v = 1.0 / n as f32;
        }
    }
    ant.cx_bump = newb;

    // decode bump → heading estimate, integrate the stride
    let h_est = cx_heading(ant);
    let leak = g.cx_hv_leak;
    ant.cx_hv_x = (1.0 - leak) * ant.cx_hv_x + dist * h_est.cos();
    ant.cx_hv_y = (1.0 - leak) * ant.cx_hv_y + dist * h_est.sin();
    let dn = (world.nest.x - ant.pos.x).hypot(world.nest.y - ant.pos.y);
    if dn <= world.nest_radius {
        ant.cx_hv_x = 0.0;
        ant.cx_hv_y = 0.0;
    }
}

/// Decode the ring-attractor bump into a heading estimate (population
/// vector / bump centroid).
pub fn cx_heading(ant: &Ant) -> f32 {
    let n = crate::genome::CX_N;
    let tau = std::f32::consts::TAU;
    let mut sx = 0.0f32;
    let mut sy = 0.0f32;
    for i in 0..n {
        let th = tau * i as f32 / n as f32;
        sx += ant.cx_bump[i] * th.cos();
        sy += ant.cx_bump[i] * th.sin();
    }
    sy.atan2(sx)
}

/// Ground-truth comparison (debug/telemetry): angle and magnitude error of
/// the CX home vector vs the software-accumulated home vector.
/// Returns (angle_err_rad, distance_err). (0,0) when both are ~zero.
pub fn cx_home_error(ant: &Ant) -> (f32, f32) {
    let tmag = ant.home_hx.hypot(ant.home_hy);
    let cmag = ant.cx_hv_x.hypot(ant.cx_hv_y);
    let dist_err = (tmag - cmag).abs();
    if tmag < 1e-6 || cmag < 1e-6 {
        return (0.0, dist_err);
    }
    let t_ang = ant.home_hy.atan2(ant.home_hx);
    let c_ang = ant.cx_hv_y.atan2(ant.cx_hv_x);
    (wrap_angle(t_ang - c_ang).abs(), dist_err)
}

/// ANN decision layer (T2.2). Fixed-topology MLP: inputs (sensors relative
/// to heading + state) → tanh hidden → linear outputs (turn, deposits,
/// attack). Weights live in `genome.ann_weights`; this direct ANN is an
/// evolution-only, lifetime-fixed baseline. The similarly shaped SNN is the
/// controller that reads the per-ant STDP weight copy. `--brain ann` replaces
/// the handwritten FSM.
pub fn ann_decide(ant: &mut Ant, s: &Sensing, _world: &World) {
    let g = &ant.genome;
    let w = &g.ann_weights;
    let n = ann_weight_count();
    if w.len() != n {
        return; // not configured — fall through to no-op (FSM path handles mode)
    }
    let ix = ant.pos.x as i32;
    let iy = ant.pos.y as i32;

    // Signed steer signal toward a bearing: wrapped angle diff / π ∈ [-1,1].
    // Nonzero for ALL relative angles (unlike sin, which is 0 when the target
    // is directly ahead OR behind — that left ants flying away from the nest).
    let steer = |b: f32| {
        let mut d = b - ant.heading;
        while d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        while d < -std::f32::consts::PI {
            d += std::f32::consts::TAU;
        }
        d / std::f32::consts::PI
    };
    let ts = steer(s.trail_bearing);
    let as_ = steer(s.alarm_bearing);
    // path integration (T3-gap3): use home vector bearing, not direct atan2.
    let nest_bearing = (-ant.home_hy).atan2(-ant.home_hx);
    let ns = steer(nest_bearing);
    let nc = (nest_bearing - ant.heading).cos();
    let c = if ant.carrying { 1.0 } else { 0.0 };
    let noise = ant.rng.gen_range(-1.0f32..1.0);
    // Pre-gated inputs (see foraging_ann_seed). input0 is scaled by trail_val
    // so that with no trail the outbound ant has zero trail-steer and only
    // wanders (via noise).
    let tv = s.trail_val * 20.0;
    let av = s.alarm_val * 10.0;
    let inputs = [
        (1.0 - c) * tv * ts, // outbound steer toward trail (gated by presence)
        c * ns,              // return steer toward nest
        (1.0 - c) * s.trail_val,
        c, // carrying gate
        ant.energy,
        s.alarm_val,
        av * as_, // alarm steer (gated by presence)
        nc,       // nest-ahead (cos)
        noise,    // exploration (bootstrap when no trail)
    ];

    // forward pass: hidden = tanh(W1·x + b1)
    // Layout: [W1(IN*HID) | b1(HID) | W2(HID*OUT) | b2(OUT)]
    let b1_off = ANN_IN * ANN_HID;
    let w2_off = b1_off + ANN_HID;
    let b2_off = w2_off + ANN_HID * ANN_OUT;
    let mut hidden = [0.0f32; ANN_HID];
    for h in 0..ANN_HID {
        let mut acc = w[b1_off + h];
        for i in 0..ANN_IN {
            acc += w[h * ANN_IN + i] * inputs[i];
        }
        hidden[h] = acc.tanh();
    }

    let mut out = [0.0f32; ANN_OUT];
    for o in 0..ANN_OUT {
        let mut acc = w[b2_off + o];
        for h in 0..ANN_HID {
            acc += w[w2_off + o * ANN_HID + h] * hidden[h];
        }
        out[o] = acc;
    }

    // apply outputs
    let turn = out[0].tanh() * g.turn_rate;
    ant.heading += turn;
    let dep_trail = out[1].max(0.0) * g.trail_release_rate;
    let dep_home = out[2].max(0.0) * g.home_release_rate * 0.1;
    let dep_alarm = out[3].max(0.0) * g.alarm_release_rate;
    if dep_trail > 0.0 {
        ant.pending_deposits
            .push((Channel::Trail, ix, iy, dep_trail));
    }
    if dep_home > 0.0 {
        ant.pending_deposits.push((Channel::Home, ix, iy, dep_home));
    }
    if dep_alarm > 0.0 {
        ant.pending_deposits
            .push((Channel::Alarm, ix, iy, dep_alarm));
    }
    if out[4] > 0.0 {
        if let Some(idx) = s.enemy_idx {
            if s.enemy_dist <= g.antenna_dist + 1.0 {
                ant.pending_attacks.push((idx, 1.0));
            }
        }
    }
    // Reflexive pickup / delivery (the network steers; these are mechanical,
    // mirroring the FSM — without them the colony never collects).
    if let Some(idx) = s.food_idx {
        if !ant.carrying {
            ant.carrying = true;
            ant.carrying_from = Some(idx);
            ant.pending_pickup = Some(idx);
            ant.pending_deposits
                .push((Channel::Recruitment, ix, iy, g.trail_release_rate * 0.5));
            ant.trip_dist = 0.0;
        }
    }
    if s.at_nest && ant.carrying {
        ant.carrying = false;
        ant.delivered_source = ant.carrying_from;
        ant.carrying_from = None;
        ant.trip_dist = 0.0;
        ant.delivered = ant.delivered.saturating_add(1);
    }
    // state label for telemetry (best-effort)
    if ant.carrying {
        ant.state = State::CarryReturn;
    } else if s.trail_val > g.trail_threshold {
        ant.state = State::FollowTrail;
    } else {
        ant.state = State::Explore;
    }
}

/// Spiking decision layer (T3-gap9): LIF (Leaky Integrate-and-Fire) neurons.
/// Unlike ann_decide's memoryless tanh forward pass, LIF neurons have membrane
/// potential state that persists across ticks (stored on Ant.lif_v/lif_v_out),
/// giving the network temporal memory — it can integrate, have delayed
/// responses, and exhibit dynamics that stateless tanh cannot produce.
/// Sub-threshold readout (tanh of potential) keeps outputs continuous for
/// steering/deposits while the integration adds the temporal dimension.
pub fn snn_decide(ant: &mut Ant, s: &Sensing, _world: &World) {
    let g = &ant.genome;
    // T7.2: use the learnable weight copy (STDP-modified) instead of the
    // innate genome weights. This separates "what evolution selects" (genome)
    // from "what the ant learns in its lifetime" (learned_w).
    let w = &ant.learned_w;
    let n = crate::genome::ann_weight_count();
    if w.len() != n {
        return;
    }

    // T19.2: age polyethism — young ants at the nest nurse brood (mirrors
    // FSM/MB). Without this the SNN colony can't grow via eclosion, leaving
    // it stranded at break-even with no margin → starvation (same gap that
    // pre-Tier-15 MB had). Gives SNN the same colony-growth engine.
    let nurse_age = NURSE_AGE_BASE + ant.task_jitter * 100.0;
    if (ant.age as f32) < nurse_age && s.at_nest && !ant.carrying {
        ant.state = State::Nurse;
        ant.pending_brood += BROOD_TEND;
        return;
    }

    let ix = ant.pos.x as i32;
    let iy = ant.pos.y as i32;

    // Same gated inputs as ann_decide (see foraging_ann_seed comments).
    // T19.2: input[0] (outbound steer) now includes a visual beeline term
    // (vs) so SNN ants can head toward visible food — matching the FSM/MB
    // visual foraging that lets the colony reach break-even before starving.
    let steer = |b: f32| {
        let mut d = b - ant.heading;
        while d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        while d < -std::f32::consts::PI {
            d += std::f32::consts::TAU;
        }
        d / std::f32::consts::PI
    };
    let ts = steer(s.trail_bearing);
    let as_ = steer(s.alarm_bearing);
    let nest_bearing = (-ant.home_hy).atan2(-ant.home_hx);
    let ns = steer(nest_bearing);
    let nc = (nest_bearing - ant.heading).cos();
    let c = if ant.carrying { 1.0 } else { 0.0 };
    let noise = ant.rng.gen_range(-1.0f32..1.0);
    let tv = s.trail_val * 20.0;
    let av = s.alarm_val * 10.0;
    let vs = if s.vision_food_idx.is_some() {
        steer(s.vision_food_bearing)
    } else {
        0.0
    };
    let inputs = [
        (1.0 - c) * (tv * ts + vs),
        c * ns,
        (1.0 - c) * s.trail_val,
        c,
        ant.energy,
        s.alarm_val,
        av * as_,
        nc,
        noise,
    ];

    // LIF forward pass with persistent membrane potential state.
    // LIF_LEAK=1.0 makes steady-state gain = 1.0 (matching tanh), while
    // the integration gives temporal memory (v approaches input with time
    // constant LIF_TAU). Lower leak would amplify gain (v=input/leak>input).
    const LIF_LEAK: f32 = 1.0;
    const LIF_TAU: f32 = 5.0;
    let dt = 1.0 / LIF_TAU;
    let b1_off = ANN_IN * ANN_HID;
    let w2_off = b1_off + ANN_HID;
    let b2_off = w2_off + ANN_HID * ANN_OUT;

    // hidden: v_h[h] += (sum_i W1[h,i]*x[i] + b1[h] - LIF_LEAK*v_h[h]) * dt
    let mut hidden = [0.0f32; ANN_HID];
    let mut h_spike = [false; ANN_HID];
    for h in 0..ANN_HID {
        let mut inp = w[b1_off + h]; // bias
        for i in 0..ANN_IN {
            inp += w[h * ANN_IN + i] * inputs[i];
        }
        ant.lif_v[h] += (inp - LIF_LEAK * ant.lif_v[h]) * dt;
        hidden[h] = ant.lif_v[h].tanh(); // sub-threshold readout
        if ant.lif_v[h] >= 1.0 {
            h_spike[h] = true;
        }
    }
    // output: v_o[o] += (sum_h W2[o,h]*hidden[h] + b2[o] - LIF_LEAK*v_o[o]) * dt
    let mut out = [0.0f32; ANN_OUT];
    let mut o_spike = [false; ANN_OUT];
    for o in 0..ANN_OUT {
        let mut inp = w[b2_off + o];
        for h in 0..ANN_HID {
            inp += w[w2_off + o * ANN_HID + h] * hidden[h];
        }
        ant.lif_v_out[o] += (inp - LIF_LEAK * ant.lif_v_out[o]) * dt;
        out[o] = ant.lif_v_out[o].tanh();
        if ant.lif_v_out[o] >= 1.0 {
            o_spike[o] = true;
        }
    }

    // T22 ablation: skip SNN lifetime plasticity when ablate_stdp is set.
    if !ant.ablate_stdp {
        // STDP (T7.2, T16): spike-timing-dependent plasticity on W2 (hidden→output),
        // now dual-dopamine-modulated (parallel to MB). Pre-before-post → LTP,
        // reward-gated (PAM analog); post-before-pre → LTD, baseline forgetting +
        // punish-amplified (PPL1 analog). The innate foraging_ann_seed is unaffected
        // (it lives in the genome copy; only the learned copy is refined here).
        const STDP_WINDOW: u32 = 10;
        const STDP_LR: f32 = 0.005;
        let ltd_rate = STDP_LR
            + if ant.dopamine_punish > crate::genome::DOPAMINE_THRESH {
                STDP_LR
            } else {
                0.0
            };
        let ltp_gated = ant.dopamine_reward > crate::genome::DOPAMINE_THRESH;
        // On hidden spike: LTD for outputs that spiked before this hidden
        for h in 0..ANN_HID {
            if h_spike[h] {
                ant.last_spike_h[h] = ant.age;
                for o in 0..ANN_OUT {
                    let ts_o = ant.last_spike_o[o];
                    if ts_o > 0 && ant.age > ts_o && ant.age - ts_o < STDP_WINDOW {
                        // post (output) fired before pre (hidden) → LTD
                        let idx = w2_off + o * ANN_HID + h;
                        let v = &mut ant.learned_w[idx];
                        *v = (*v - ltd_rate).max(crate::genome::ANN_W_MIN);
                    }
                }
            }
        }
        // On output spike: LTP for hiddens that spiked before this output (reward-gated)
        for o in 0..ANN_OUT {
            if o_spike[o] {
                ant.last_spike_o[o] = ant.age;
                if ltp_gated {
                    for h in 0..ANN_HID {
                        let ts_h = ant.last_spike_h[h];
                        if ts_h > 0 && ant.age > ts_h && ant.age - ts_h < STDP_WINDOW {
                            // pre (hidden) fired before post (output) → LTP
                            let idx = w2_off + o * ANN_HID + h;
                            let v = &mut ant.learned_w[idx];
                            *v = (*v + STDP_LR).min(crate::genome::ANN_W_MAX);
                        }
                    }
                }
            }
        }
    } // T22 ablate_stdp guard

    // apply outputs (same mapping as ann_decide)
    let turn = out[0].tanh() * g.turn_rate;
    ant.heading += turn;
    let dep_trail = out[1].max(0.0) * g.trail_release_rate;
    let dep_home = out[2].max(0.0) * g.home_release_rate * 0.1;
    let dep_alarm = out[3].max(0.0) * g.alarm_release_rate;
    if dep_trail > 0.0 {
        ant.pending_deposits
            .push((Channel::Trail, ix, iy, dep_trail));
    }
    if dep_home > 0.0 {
        ant.pending_deposits.push((Channel::Home, ix, iy, dep_home));
    }
    if dep_alarm > 0.0 {
        ant.pending_deposits
            .push((Channel::Alarm, ix, iy, dep_alarm));
    }
    if out[4] > 0.0 {
        if let Some(idx) = s.enemy_idx {
            if s.enemy_dist <= g.antenna_dist + 1.0 {
                ant.pending_attacks.push((idx, 1.0));
            }
        }
    }
    // reflexive pickup/delivery (same as ann_decide)
    if let Some(idx) = s.food_idx {
        if !ant.carrying {
            ant.carrying = true;
            ant.carrying_from = Some(idx);
            ant.trip_dist = 0.0;
        }
    }
    if s.at_nest && ant.carrying {
        ant.carrying = false;
        ant.delivered_source = ant.carrying_from;
        ant.carrying_from = None;
        ant.trip_dist = 0.0;
        ant.delivered = ant.delivered.saturating_add(1);
    }
    if ant.carrying {
        ant.state = State::CarryReturn;
    } else if s.trail_val > g.trail_threshold {
        ant.state = State::FollowTrail;
    } else {
        ant.state = State::Explore;
    }
}

/// T8 modular brain: AL → MB → Output with dopamine-gated STDP.
/// Antennal lobe: 16 inputs → 8 glomeruli (convergence + lateral
/// inhibition + oscillation). Mushroom body: 8 → 64 sparse KC (random
/// projection + high threshold). Output: 64 → 5 (tanh readout).
///
/// Chemical amplitudes are read from the auditable PN projections in `code`;
/// local spatial bearings remain in `s` because they are already observations.
pub fn mb_decide_with_code(ant: &mut Ant, s: &Sensing, code: &SensoryCode, world: &World) {
    let g = &ant.genome;
    let w = &ant.learned_mb_w;
    let n = crate::genome::mb_weight_count();
    if w.len() != n {
        return;
    }

    // T15: age polyethism — young ants at the nest nurse brood (mirrors FSM).
    // Without nursing the MB colony can't grow via eclosion, leaving it stranded
    // at break-even with no margin → starvation spiral once trails fluctuate.
    // This gives MB the same colony-growth engine FSM uses to pull ahead.
    let nurse_age = NURSE_AGE_BASE + ant.task_jitter * 100.0;
    if (ant.age as f32) < nurse_age && s.at_nest && !ant.carrying {
        ant.state = State::Nurse;
        ant.pending_brood += BROOD_TEND;
        return;
    }

    let ix = ant.pos.x as i32;
    let iy = ant.pos.y as i32;

    // --- input assembly (20 elements) ---
    let steer = |b: f32| {
        let mut d = b - ant.heading;
        while d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        while d < -std::f32::consts::PI {
            d += std::f32::consts::TAU;
        }
        d / std::f32::consts::PI
    };
    let ts = steer(s.trail_bearing);
    let ns = steer((-ant.home_hy).atan2(-ant.home_hx));
    let as_ = steer(s.alarm_bearing);
    let nc = ((-ant.home_hy).atan2(-ant.home_hx) - ant.heading).cos();
    let c = if ant.carrying { 1.0 } else { 0.0 };
    let noise = ant.rng.gen_range(-1.0f32..1.0);
    // T15 foraging prior: carrying-gated steer (mirrors foraging_ann_seed). Binary
    // KCs can't encode a signed graded turn, so the *seed* (mb_seed) sign-splits
    // these into L/R detector KCs — outbound ants steer toward Trail (or visible
    // food), inbound ants steer toward the nest. The trail-presence gate keeps
    // outbound ants from chasing noise when no trail exists yet; vision (when
    // food is in the forward cone) beelines directly, matching the FSM's visual
    // foraging — this is what lets the colony reach break-even before starving.
    let tv = (code.pn_single[0] * 20.0).tanh(); // PN trail-presence gate
    let vs = if s.vision_food_idx.is_some() {
        steer(s.vision_food_bearing)
    } else {
        0.0
    };
    let out_steer = (1.0 - c) * (ts * tv + vs); // input3:  outbound trail+vision steer
    let in_steer = c * ns; // input15: inbound nest steer
                           // T10 multi-modal: vision (food seen at a bearing) + proximity, gated to zero
                           // when nothing is in the visual cone so cos doesn't falsely read "ahead".
    let (v_sin, v_cos, v_prox) = if s.vision_food_idx.is_some() {
        let rel = s.vision_food_bearing - ant.heading;
        (rel.sin(), rel.cos(), 1.0 / (1.0 + s.vision_food_dist))
    } else {
        (0.0, 0.0, 0.0)
    };
    let nest_prox = 1.0 / (1.0 + s.nest_dist);
    // 20 inputs: 4 PN chemical channels, 4 steer sins, 4 steer coss, 4 state,
    // and 4 multimodal terms. Slots 3 and 15 carry carrying-gated steer signals.
    let inputs: [f32; crate::genome::MB_AL_INPUTS] = [
        code.pn_single[0],
        code.pn_single[1],
        code.pn_single[2],
        out_steer, // PN channels plus outbound steer
        ts,
        ns,
        as_,
        noise, // 4 steer signals
        (s.trail_bearing - ant.heading).cos(),
        ((-ant.home_hy).atan2(-ant.home_hx) - ant.heading).cos(),
        (s.alarm_bearing - ant.heading).cos(),
        nc, // 4 cos signals
        c,
        ant.energy,
        code.pn_mixed[1] - code.pn_mixed[2],
        in_steer, // state plus mixed PN opponent evidence
        v_sin,
        v_cos,
        v_prox,
        nest_prox, // 4 multi-modal (T10 vision+prox)
    ];

    // --- weight layout offsets ---
    let al_in = crate::genome::MB_AL_INPUTS;
    let al_g = crate::genome::MB_AL_GLOM;
    let kc_n = crate::genome::MB_KC;
    let out_n = crate::genome::MB_OUT;
    let off_b_al = al_in * al_g;
    let off_w_lat = off_b_al + al_g;
    let off_w_kc = off_w_lat + al_g * al_g;
    let off_b_kc = off_w_kc + al_g * kc_n;
    let off_w_out = off_b_kc + kc_n;
    let off_b_out = off_w_out + kc_n * out_n;

    // --- AL: convergence (W_al · input + b_al) ---
    let mut al_raw = [0.0f32; crate::genome::MB_AL_GLOM];
    for gl in 0..al_g {
        let mut acc = w[off_b_al + gl];
        for i in 0..al_in {
            acc += w[gl * al_in + i] * inputs[i];
        }
        al_raw[gl] = acc.tanh();
    }
    // lateral inhibition (W_lat: winner-take-all contrast enhancement)
    let mut al_out = [0.0f32; crate::genome::MB_AL_GLOM];
    for gl in 0..al_g {
        let mut inhib = 0.0;
        for gl2 in 0..al_g {
            if gl != gl2 {
                inhib += w[off_w_lat + gl * al_g + gl2] * al_raw[gl2];
            }
        }
        al_out[gl] = (al_raw[gl] - inhib * 0.3).tanh();
    }
    // oscillation modulation (global phase → temporal coding). T15: depth reduced
    // from 0.5±0.5 to 0.8±0.2 so al_out never drops below 60% — the foraging-prior
    // detector KCs (mb_seed) need reliable spiking, and full-depth modulation
    // quenched them each cycle (~0% duty at cos=-1) so the seed couldn't steer.
    // Temporal coding is preserved (still oscillates), just without full blackout.
    ant.mb_al_osc += 0.15; // oscillation frequency
    let osc_mod = 0.8 + 0.2 * ant.mb_al_osc.cos();
    for gl in 0..al_g {
        al_out[gl] *= osc_mod;
    }

    // --- MB: sparse KC (W_kc · al + b_kc, threshold → binary) ---
    // T13: only the ant's currently-grown KCs (mb_kc_active) participate —
    // dormant KCs are "not yet developed" and never spike. The weight layout
    // offsets below still use the full kc_n so dormant-KC weights stay valid.
    let kc_act = ant.mb_kc_active.min(kc_n);
    let mut kc_spikes = vec![false; kc_n];
    for k in 0..kc_act {
        let mut acc = w[off_b_kc + k];
        for gl in 0..al_g {
            acc += w[off_w_kc + k * al_g + gl] * al_out[gl];
        }
        ant.mb_kc_v[k] = acc; // store raw potential
        if acc >= crate::genome::KC_THRESH {
            kc_spikes[k] = true;
            ant.last_kc_spike[k] = ant.age;
        }
    }

    // --- Output: tanh(W_out · kc_sparse + b_out) ---
    let mut out = [0.0f32; 5];
    for o in 0..out_n {
        let mut acc = w[off_b_out + o];
        for k in 0..kc_act {
            let kc_val = if kc_spikes[k] { 1.0 } else { 0.0 };
            acc += w[off_w_out + o * kc_n + k] * kc_val;
        }
        ant.mb_out_v[o] = acc;
        out[o] = acc.tanh();
    }

    // T22 ablation: skip all lifetime synaptic plasticity (W_out STDP + W_kc
    // Hebbian) when ablate_stdp is set — readout is weight-drift (learning).
    if !ant.ablate_stdp {
        // --- STDP on W_out (KC→output), compartmentalized dual-dopamine (T16/T18) ---
        // T18 per-output valence: approach outputs (turn/trail/home, o<MB_APPROACH_OUT)
        // use reward→LTP, punish→LTD (PAM compartment); avoidance outputs (alarm/attack)
        // are OPPONENT — punish→LTP, reward→LTD (PPL1 compartment). This lets the MB
        // learn approach AND avoidance in parallel: foraging reward strengthens food-
        // seeking while war punish strengthens defense. Baseline forgetting LTD on all.
        let thr = crate::genome::DOPAMINE_THRESH;
        let valence = |o: usize| -> (f32, f32) {
            if o < crate::genome::MB_APPROACH_OUT {
                (ant.dopamine_reward, ant.dopamine_punish)
            } else {
                (ant.dopamine_punish, ant.dopamine_reward) // opponent
            }
        };
        // LTD: baseline forgetting + ltd_dop-amplified
        for k in 0..kc_act {
            if kc_spikes[k] {
                for o in 0..out_n {
                    let ts_o = ant.last_mb_out_spike[o]; // T15: MB-private
                    if ts_o > 0 && ant.age > ts_o && ant.age - ts_o < 10 {
                        let (_, ltd_dop) = valence(o);
                        let rate = 0.005 + if ltd_dop > thr { 0.005 } else { 0.0 };
                        let idx = off_w_out + o * kc_n + k;
                        let v = &mut ant.learned_mb_w[idx];
                        *v = (*v - rate).max(crate::genome::ANN_W_MIN);
                    }
                }
            }
        }
        // LTP: ltp_dop-gated (reward for approach, punish for avoidance)
        for o in 0..out_n {
            if ant.mb_out_v[o] >= 1.0 {
                ant.last_mb_out_spike[o] = ant.age;
                let (ltp_dop, _) = valence(o);
                if ltp_dop > thr {
                    for k in 0..kc_act {
                        let ts_k = ant.last_kc_spike[k];
                        if ts_k > 0 && ant.age > ts_k && ant.age - ts_k < 10 {
                            let idx = off_w_out + o * kc_n + k;
                            let v = &mut ant.learned_mb_w[idx];
                            *v = (*v + 0.01).min(crate::genome::ANN_W_MAX);
                        }
                    }
                }
            }
        }

        // --- T17: AL→KC (W_kc) Hebbian plasticity, dopamine-gated, on learnable
        // substrate KCs only (detectors 0..MB_DETECTOR_KCS are protected so the
        // foraging-prior thresholds stay stable). KC k spiked + glom g active →
        // strengthen (reward, PAM) / weaken (punish, PPL1) that AL→KC input.
        // Small rate + W bounds → broadens the learnable surface without runaway.
        const WKC_LR: f32 = 0.002;
        let rew_w = ant.dopamine_reward > crate::genome::DOPAMINE_THRESH;
        let pun_w = ant.dopamine_punish > crate::genome::DOPAMINE_THRESH;
        let det = crate::genome::MB_DETECTOR_KCS;
        if (rew_w || pun_w) && kc_act > det {
            for k in det..kc_act {
                if kc_spikes[k] {
                    for gl in 0..al_g {
                        if al_out[gl].abs() > 0.1 {
                            let idx = off_w_kc + k * al_g + gl;
                            let v = &mut ant.learned_mb_w[idx];
                            if rew_w {
                                *v = (*v + WKC_LR).min(crate::genome::ANN_W_MAX);
                            }
                            if pun_w {
                                *v = (*v - WKC_LR).max(crate::genome::ANN_W_MIN);
                            }
                        }
                    }
                }
            }
        }
    } // T22 ablate_stdp guard

    // --- apply outputs ---
    // T15: turn is a graded innate AL→turn reflex (lateral-horn analog), NOT the
    // binary KC readout. Binary KCs can't encode a signed graded turn, so the
    // bang-bang out0 dead-boxed the colony into starvation. The signed steer
    // gloms (0 = outbound trail+vision, 1 = inbound nest) are carrying-gated in
    // the inputs, so their sum is the active proportional steer signal — no
    // dead zone, smooth tracking like the FSM's steer_toward. The noise glom
    // (7) adds a graded symmetric random walk for exploration. KC→output
    // (deposits/attack) stays binary + STDP-learnable; steering is innate
    // (evolution tunes W_al + the gain below). out0/mb_out_v[0] are still
    // computed above as the STDP substrate.
    let steer_g = al_raw[0] + al_raw[1]; // signed, carrying-gated (only one nonzero)
    let noise_g = al_raw[7]; // signed → symmetric exploration
    let turn = (2.5 * steer_g + 0.5 * noise_g).tanh() * g.turn_rate;
    ant.heading += turn;
    let dep_trail = out[1].max(0.0) * g.trail_release_rate;
    // home deposit scaled to match the FSM's faint outbound Home marking
    // (FSM lays Home at home_release_rate*0.05; here out[2]≤1 so *0.1 ≈ same order)
    let dep_home = out[2].max(0.0) * g.home_release_rate * 0.1;
    let dep_alarm = out[3].max(0.0) * g.alarm_release_rate;
    if dep_trail > 0.0 {
        ant.pending_deposits
            .push((Channel::Trail, ix, iy, dep_trail));
    }
    if dep_home > 0.0 {
        ant.pending_deposits.push((Channel::Home, ix, iy, dep_home));
    }
    if dep_alarm > 0.0 {
        ant.pending_deposits
            .push((Channel::Alarm, ix, iy, dep_alarm));
    }
    if out[4] > 0.0 {
        if let Some(idx) = s.enemy_idx {
            if s.enemy_dist <= g.antenna_dist + 1.0 {
                ant.pending_attacks.push((idx, 1.0));
            }
        }
    }
    // reflexive pickup/delivery
    if let Some(idx) = s.food_idx {
        if !ant.carrying {
            ant.carrying = true;
            ant.carrying_from = Some(idx);
            ant.trip_dist = 0.0;
            ant.pending_pickup = Some(idx);
            ant.pending_deposits
                .push((Channel::Recruitment, ix, iy, g.trail_release_rate * 0.5));
        }
    }
    if s.at_nest && ant.carrying {
        ant.carrying = false;
        ant.delivered_source = ant.carrying_from;
        ant.carrying_from = None;
        ant.trip_dist = 0.0;
        ant.delivered = ant.delivered.saturating_add(1);
        ant.heading = (ant.pos.y - world.nest.y).atan2(ant.pos.x - world.nest.x);
    }
    // (T11: octopamine/dopamine now updated experience-driven in Ant::update,
    // shared across all brain paths — not per-brain here.)
    if ant.carrying {
        ant.state = State::CarryReturn;
    } else if s.trail_val > g.trail_threshold {
        ant.state = State::FollowTrail;
    } else {
        ant.state = State::Explore;
    }
}

/// Compatibility entry point for focused MB tests and callers without a
/// precomputed sensory code. The simulator uses `mb_decide_with_code`.
pub fn mb_decide(ant: &mut Ant, s: &Sensing, world: &World) {
    let code = crate::ant::sensors::encode(ant, s);
    mb_decide_with_code(ant, s, &code, world);
}

#[cfg(test)]
mod cx_tests {
    use super::*;
    use crate::ant::Ant;
    use crate::genome::Genome;
    use crate::world::{Vec2, World};

    fn setup() -> (Ant, World) {
        let g = Genome::default();
        let world = World::new(64, 64, Vec2::new(32.0, 32.0), 4.0);
        // start the ant well away from the nest so the integrator never
        // triggers the nest-radius reset during these micro-tests
        let ant = Ant::new(Vec2::new(50.0, 50.0), 0.0, &g, 7);
        (ant, world)
    }

    /// Bump stability: with a constant heading the ring attractor should
    /// settle into a single, localized bump whose decoded heading matches
    /// the true heading (within one ring cell).
    #[test]
    fn cx_observation_occlusion_removes_compass_anchor() {
        let (mut ant, world) = setup();
        ant.motor_feedback.actual_turn = 0.0;
        for _ in 0..20 {
            cx_integrate_observed(
                &mut ant,
                &world,
                0.5,
                crate::ant::sensors::CompassObservation {
                    bearing: 0.0,
                    confidence: 1.0,
                    available: true,
                },
            );
        }
        let anchored = cx_heading(&ant);
        for _ in 0..10 {
            cx_integrate_observed(
                &mut ant,
                &world,
                0.5,
                crate::ant::sensors::CompassObservation::default(),
            );
        }
        assert!(wrap_angle(cx_heading(&ant) - anchored).abs() < 0.5);
    }

    #[test]
    fn cx_bump_stable_and_tracks_heading() {
        let (mut ant, world) = setup();
        let h = 0.7_f32; // constant heading
        ant.heading = h;
        ant.cx_prev_heading = h;
        for _ in 0..40 {
            ant.heading = h;
            cx_integrate(&mut ant, &world, 0.5);
        }
        // single bump: exactly one dominant cell
        let max = ant.cx_bump.iter().cloned().fold(0.0f32, f32::max);
        let total: f32 = ant.cx_bump.iter().sum();
        assert!(max > 0.0, "bump collapsed");
        // one cell clearly dominates (localization). A cosine-kernel bump is
        // broad, so we require it to be strongly peaked above uniform (1/N),
        // and rely on the heading-decode check below as the strong guarantee.
        let uniform = 1.0 / crate::genome::CX_N as f32;
        assert!(
            max / total > 2.0 * uniform,
            "bump not peaked: max/total={}",
            max / total
        );
        // decoded heading near true heading (within one cell of the ring)
        let cell = std::f32::consts::TAU / crate::genome::CX_N as f32;
        let err = wrap_angle(cx_heading(&ant) - h).abs();
        assert!(err <= cell, "decoded heading err {err} > cell {cell}");
    }

    /// Shift correctness: rotating the ant by `dh` should move the decoded
    /// heading estimate by ~`dh` (the bump translates around the ring).
    #[test]
    fn cx_bump_shifts_with_turn() {
        let (mut ant, world) = setup();
        ant.heading = 0.0;
        ant.cx_prev_heading = 0.0;
        for _ in 0..30 {
            cx_integrate(&mut ant, &world, 0.0); // settle, no translation
        }
        let before = cx_heading(&ant);
        // turn the ant in a few increments, integrating each step
        let dh_step = 0.1_f32;
        let steps = 5;
        for _ in 0..steps {
            ant.heading = wrap_angle(ant.heading + dh_step);
            cx_integrate(&mut ant, &world, 0.0);
        }
        let after = cx_heading(&ant);
        let moved = wrap_angle(after - before);
        let want = dh_step * steps as f32;
        // allow one ring cell of tolerance either way
        let cell = std::f32::consts::TAU / crate::genome::CX_N as f32;
        assert!(
            (moved - want).abs() <= cell,
            "bump moved {moved}, expected ~{want} (±{cell})"
        );
    }

    /// Integration error bounded: a deterministic square walk should yield a
    /// CX home vector whose magnitude is within a generous tolerance of the
    /// exact software integral (dist·cos/sin of the *true* heading).
    #[test]
    fn cx_integration_bounded_error() {
        let (mut ant, world) = setup();
        ant.cx_prev_heading = ant.heading;
        // ground-truth software integral of the same walk
        let mut tx = 0.0f32;
        let mut ty = 0.0f32;
        let stride = 0.5f32;
        // 4 legs of a square, 20 strides each, heading 0, 90, 180, 270 deg
        let legs = [
            0.0f32,
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
            1.5 * std::f32::consts::PI,
        ];
        for &lh in &legs {
            ant.heading = lh;
            for _ in 0..20 {
                cx_integrate(&mut ant, &world, stride);
                tx += stride * lh.cos();
                ty += stride * lh.sin();
            }
        }
        // The square is closed → true net vector ≈ 0, so the CX estimate
        // should also be small relative to the total distance walked.
        let walked = stride * 20.0 * 4.0;
        let cmag = ant.cx_hv_x.hypot(ant.cx_hv_y);
        assert!(
            cmag < 0.25 * walked,
            "cx home vector magnitude {cmag} too large after closed walk of {walked}"
        );
        // and the true integral is ~0 as expected (sanity)
        assert!(tx.hypot(ty) < 1.0);
    }

    /// home_vector() dispatches to the neural integral only in CX mode.
    #[test]
    fn cx_home_vector_dispatch() {
        let (mut ant, _world) = setup();
        ant.home_hx = 3.0;
        ant.home_hy = 4.0;
        ant.cx_hv_x = -1.0;
        ant.cx_hv_y = 2.0;
        ant.use_cx = false;
        assert_eq!(ant.home_vector(), (3.0, 4.0));
        ant.use_cx = true;
        assert_eq!(ant.home_vector(), (-1.0, 2.0));
    }
}

#[cfg(test)]
mod fsm_tests {
    use super::*;
    use crate::ant::Ant;
    use crate::genome::Genome;
    use crate::world::{Vec2, World};

    fn ant_with_follow_strength(follow_strength: f32) -> (Ant, World) {
        let g = Genome {
            follow_strength,
            ..Genome::default()
        };
        let world = World::new(64, 64, Vec2::new(32.0, 32.0), 4.0);
        let mut ant = Ant::new(Vec2::new(50.0, 50.0), 0.0, &g, 7);
        ant.age = 1_000;
        (ant, world)
    }

    #[test]
    fn fsm_follow_strength_scales_trail_steering() {
        let s = Sensing {
            trail_val: 1.0,
            trail_bearing: std::f32::consts::FRAC_PI_2,
            ..Default::default()
        };
        let (mut weak, world) = ant_with_follow_strength(0.5);
        let (mut strong, _) = ant_with_follow_strength(6.0);
        decide(&mut weak, &s, &world);
        decide(&mut strong, &s, &world);
        assert!(
            strong.heading > weak.heading,
            "stronger chemotaxis should yield a larger turn: {} <= {}",
            strong.heading,
            weak.heading
        );
    }
}

#[cfg(test)]
mod mb_tests {
    use super::*;
    use crate::ant::sensors::Sensing;
    use crate::ant::Ant;
    use crate::genome::Genome;
    use crate::world::{Vec2, World};

    fn mb_ant(heading: f32) -> (Ant, World) {
        let g = Genome::default();
        let world = World::new(64, 64, Vec2::new(32.0, 32.0), 4.0);
        let mut ant = Ant::new(Vec2::new(50.0, 50.0), heading, &g, 7);
        ant.age = 1000; // past nurse_age → foraging logic, not nursing
        (ant, world)
    }

    /// index into learned_mb_w for W_out at (output o, KC k), mirroring mb_seed.
    fn wout_idx(o: usize, k: usize) -> usize {
        use crate::genome::{MB_AL_GLOM as AG, MB_AL_INPUTS as AI, MB_KC as KN};
        let off_w_out = AI * AG + AG + AG * AG + AG * KN + KN;
        off_w_out + o * KN + k
    }

    #[test]
    fn mb_punishment_learning_decreases_active_approach_weight() {
        let (mut ant, world) = mb_ant(0.0);
        let s = Sensing {
            trail_val: 0.8,
            trail_bearing: std::f32::consts::FRAC_PI_2,
            ..Default::default()
        };
        let k = 0;
        let idx = wout_idx(0, k);
        let before = ant.learned_mb_w[idx];
        ant.dopamine_punish = crate::genome::DOPAMINE_MAX;
        for _ in 0..4 {
            ant.age += 1;
            mb_decide(&mut ant, &s, &world);
        }
        assert!(
            ant.learned_mb_w[idx] < before,
            "punished cue should weaken its active approach pathway"
        );
    }

    /// Outbound (not carrying) with Trail to the left → ant turns left
    /// (heading increases). The graded AL→turn reflex must sign-match the
    /// trail bearing — the fix for the binary-KC bang-bang limitation that
    /// dead-boxed the colony into starvation.
    #[test]
    fn mb_seed_outbound_steers_toward_trail_left() {
        let (mut ant, world) = mb_ant(0.0);
        let s = Sensing {
            trail_val: 0.5,
            trail_bearing: std::f32::consts::FRAC_PI_2, // trail 90° to the left
            ..Default::default()
        };
        let h0 = ant.heading;
        mb_decide(&mut ant, &s, &world);
        assert!(
            ant.heading > h0,
            "outbound ant should turn left toward trail, heading {} -> {}",
            h0,
            ant.heading
        );
    }

    /// Outbound with Trail to the right → ant turns right (heading decreases).
    #[test]
    fn mb_seed_outbound_steers_toward_trail_right() {
        let (mut ant, world) = mb_ant(0.0);
        let s = Sensing {
            trail_val: 0.5,
            trail_bearing: -std::f32::consts::FRAC_PI_2, // trail 90° to the right
            ..Default::default()
        };
        let h0 = ant.heading;
        mb_decide(&mut ant, &s, &world);
        assert!(
            ant.heading < h0,
            "outbound ant should turn right toward trail, heading {} -> {}",
            h0,
            ant.heading
        );
    }

    /// Inbound (carrying) → ant steers toward the nest along the home vector.
    #[test]
    fn mb_seed_inbound_steers_toward_nest() {
        let (mut ant, world) = mb_ant(0.0);
        ant.carrying = true;
        // home vector points nest→ant; ant is +y of nest → nest bearing = -π/2.
        ant.home_hx = 0.0;
        ant.home_hy = 10.0;
        let s = Sensing {
            ..Default::default()
        };
        let h0 = ant.heading;
        mb_decide(&mut ant, &s, &world);
        assert!(
            ant.heading < h0,
            "carrying ant should turn toward nest (-π/2), heading {} -> {}",
            h0,
            ant.heading
        );
    }

    /// T16: punish dopamine (PPL1 analog) amplifies LTD. Carrying → KC5 spikes
    /// + out1 spikes. Call 1 records the output spike; call 2 (KC5 spikes again,
    /// output had spiked before) fires LTD. With punish dopamine the LTD rate
    /// is higher, so the KC5→out1 weight drops more than without it.
    #[test]
    fn mb_punish_dopamine_amplifies_ltd() {
        let mk = |punish: bool| -> f32 {
            let (mut ant, world) = mb_ant(0.0);
            ant.carrying = true;
            let s = Sensing {
                ..Default::default()
            };
            mb_decide(&mut ant, &s, &world); // call 1: record out1 spike
            ant.age = 1001; // advance so age > last_mb_out_spike
            if punish {
                ant.dopamine_punish = 1.0;
            }
            let idx = wout_idx(1, 5);
            let before = ant.learned_mb_w[idx];
            mb_decide(&mut ant, &s, &world); // call 2: LTD fires
            before - ant.learned_mb_w[idx]
        };
        let d_punish = mk(true);
        let d_base = mk(false);
        assert!(d_punish > 0.0, "punish dopamine should drive LTD");
        assert!(
            d_punish > d_base,
            "punish should amplify LTD over baseline: punish {} <= baseline {}",
            d_punish,
            d_base
        );
    }

    /// T18: compartmentalized DAN — punish drives LTP on the AVOIDANCE output
    /// (alarm, out3), not LTD. KC6 (alarm-left detector) fires call 1 (alarm to
    /// the left) setting its eligibility trace; call 2 alarm flips to the right
    /// so KC7 fires (KC6 silent, trace intact) while out3 still spikes → punish
    /// (opponent valence for avoidance) drives LTP on KC6→out3.
    #[test]
    fn mb_compartmentalized_avoidance_ltp() {
        let (mut ant, world) = mb_ant(0.0);
        ant.dopamine_punish = 1.0;
        let s_left = Sensing {
            alarm_val: 0.5,
            alarm_bearing: std::f32::consts::FRAC_PI_2,
            ..Default::default()
        };
        mb_decide(&mut ant, &s_left, &world); // KC6 fires, out3 spikes, trace set
        ant.age = 1001;
        ant.dopamine_punish = 1.0; // re-arm (decay only happens in update)
        let s_right = Sensing {
            alarm_val: 0.5,
            alarm_bearing: -std::f32::consts::FRAC_PI_2,
            ..Default::default()
        };
        let idx = wout_idx(3, 6);
        let before = ant.learned_mb_w[idx];
        mb_decide(&mut ant, &s_right, &world); // KC7 fires (KC6 silent), out3 spikes
        assert!(
            ant.learned_mb_w[idx] > before,
            "punish should drive LTP on avoidance output (alarm KC6->out3): {} -> {}",
            before,
            ant.learned_mb_w[idx]
        );
    }
}

#[cfg(test)]
mod snn_tests {
    use super::*;
    use crate::ant::sensors::Sensing;
    use crate::ant::Ant;
    use crate::genome::Genome;
    use crate::world::{Vec2, World};

    /// The direct ANN is intentionally evolution-only: its decision path must
    /// never read or alter the SNN-private lifetime plasticity copy.
    #[test]
    fn ann_decision_does_not_modify_snn_learned_weights() {
        let g = Genome::default();
        let world = World::new(64, 64, Vec2::new(32.0, 32.0), 6.0);
        let mut ant = Ant::new(Vec2::new(50.0, 50.0), 0.0, &g, 7);
        ant.age = 1_000;
        ant.learned_w.iter_mut().for_each(|weight| *weight = -3.0);
        let before = ant.learned_w.clone();
        let s = Sensing {
            trail_val: 0.5,
            trail_bearing: std::f32::consts::FRAC_PI_2,
            ..Default::default()
        };

        ann_decide(&mut ant, &s, &world);

        assert_eq!(ant.learned_w, before);
    }

    /// In contrast to the direct ANN, the SNN uses the private weight copy:
    /// a recent hidden spike plus a dopamine-gated output spike potentiates W2.
    #[test]
    fn snn_reward_gated_stdp_modifies_learned_w2() {
        let g = Genome::default();
        let world = World::new(64, 64, Vec2::new(32.0, 32.0), 6.0);
        let mut ant = Ant::new(Vec2::new(50.0, 50.0), 0.0, &g, 7);
        ant.age = 1_000;
        ant.learned_w.fill(0.0);
        ant.lif_v = [0.99; ANN_HID]; // decays below threshold this tick
        ant.lif_v_out = [1.3; ANN_OUT]; // remains above threshold this tick
        ant.last_spike_h = [999; ANN_HID]; // valid pre-before-post trace
        ant.dopamine_reward = crate::genome::DOPAMINE_THRESH + 0.1;
        let w2_off = ANN_IN * ANN_HID + ANN_HID;
        let before = ant.learned_w[w2_off];

        snn_decide(&mut ant, &Sensing::default(), &world);

        assert!(
            ant.learned_w[w2_off] > before,
            "reward-gated SNN STDP must potentiate the learned W2 copy"
        );
    }

    /// T19.2: a young SNN ant at the nest nurses brood (age polyethism),
    /// mirroring FSM/MB — gives SNN the eclosion colony-growth engine.
    #[test]
    fn snn_young_at_nest_nurses() {
        let g = Genome::default();
        let world = World::new(64, 64, Vec2::new(32.0, 32.0), 6.0);
        let mut ant = Ant::new(Vec2::new(32.0, 32.0), 0.0, &g, 7); // at nest
        ant.age = 50; // < nurse_age (200) → young
        let s = Sensing {
            at_nest: true,
            ..Default::default()
        };
        snn_decide(&mut ant, &s, &world);
        assert!(
            matches!(ant.state, crate::ant::State::Nurse),
            "young SNN ant at nest should nurse"
        );
        assert!(ant.pending_brood > 0.0, "nurse should tend brood");
    }

    /// T19.2: SNN outbound with vision food to the right steers toward it
    /// (the visual beeline term in input[0] — mirror of MB visual foraging).
    #[test]
    fn snn_outbound_steers_toward_visible_food() {
        let g = Genome::default();
        let world = World::new(64, 64, Vec2::new(32.0, 32.0), 6.0);
        let mut ant = Ant::new(Vec2::new(50.0, 50.0), 0.0, &g, 7); // away from nest
        ant.age = 1000; // past nurse_age → foraging
        let s = Sensing {
            vision_food_idx: Some(0),
            vision_food_bearing: -std::f32::consts::FRAC_PI_2, // food 90° right
            ..Default::default()
        };
        let h0 = ant.heading;
        // run several ticks so the LIF membrane integrates the visual steer
        for _ in 0..20 {
            snn_decide(&mut ant, &s, &world);
        }
        assert!(
            ant.heading < h0,
            "outbound SNN ant should turn toward visible food (right), heading {} -> {}",
            h0,
            ant.heading
        );
    }
}
