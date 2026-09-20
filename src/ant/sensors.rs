//! Sensory sampling and its explicit, low-dimensional neural encoding.
//!
//! `Sensing` is the physical observation used by legacy controllers. `SensoryCode`
//! is the auditable ORN→AL→PN/LH-like representation consumed by new modules.
//! Neither contains a world target coordinate; bearings are local observations.

use crate::ant::Ant;
use crate::world::{Channel, Vec2, World};
use rand::Rng;

pub const CHEM_CHANNELS: usize = 4;

/// A noisy, optionally unavailable compass observation. The true body heading
/// is used only while constructing this observation and never injected into CX.
#[derive(Clone, Copy, Debug, Default)]
pub struct CompassObservation {
    pub bearing: f32,
    pub confidence: f32,
    pub available: bool,
}

/// Early sensory representation. The dimensions are abstract model channels,
/// not a claim about receptor or glomerulus counts in a real insect.
#[derive(Clone, Debug)]
pub struct SensoryCode {
    pub raw: [f32; CHEM_CHANNELS],
    pub orn: [f32; CHEM_CHANNELS],
    pub al: [f32; CHEM_CHANNELS],
    /// Channel-specific PN projection.
    pub pn_single: [f32; CHEM_CHANNELS],
    /// Multi-channel PN projection: total evidence and two opponent contrasts.
    pub pn_mixed: [f32; 3],
    /// Fast innate approach (+) / avoidance (-) steering evidence.
    pub lh_turn: f32,
    pub compass: CompassObservation,
    pub vision_food: bool,
    pub contact_signal: f32,
}

impl Default for SensoryCode {
    fn default() -> Self {
        Self {
            raw: [0.0; CHEM_CHANNELS],
            orn: [0.0; CHEM_CHANNELS],
            al: [0.0; CHEM_CHANNELS],
            pn_single: [0.0; CHEM_CHANNELS],
            pn_mixed: [0.0; 3],
            lh_turn: 0.0,
            compass: CompassObservation::default(),
            vision_food: false,
            contact_signal: 0.0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Sensing {
    pub trail_val: f32,
    pub trail_bearing: f32,
    pub home_val: f32,
    pub home_bearing: f32,
    pub alarm_val: f32,
    pub alarm_bearing: f32,
    pub nest_dist: f32,
    pub at_nest: bool,
    pub food_idx: Option<usize>,
    pub food_dist: f32,
    pub enemy_idx: Option<usize>,
    pub enemy_dist: f32,
    pub vision_food_idx: Option<usize>,
    pub vision_food_bearing: f32,
    pub vision_food_dist: f32,
}

fn grad(world: &World, pos: Vec2, ch: Channel, dist: f32) -> (f32, f32) {
    let mut best_v = -1.0f32;
    let mut best_a = 0.0f32;
    for k in 0..8u32 {
        let a = (k as f32) * std::f32::consts::TAU / 8.0;
        let v = world.sample(ch, pos.x + a.cos() * dist, pos.y + a.sin() * dist);
        if v > best_v {
            best_v = v;
            best_a = a;
        }
    }
    (best_v.max(0.0), best_a)
}

pub fn sense(ant: &Ant, world: &World) -> Sensing {
    let g = &ant.genome;
    let d = g.antenna_dist;
    let (trail_val, trail_bearing) = grad(world, ant.pos, Channel::Trail, d);
    let (home_val, home_bearing) = grad(world, ant.pos, Channel::Home, d);
    let (alarm_val, alarm_bearing) = grad(world, ant.pos, Channel::Alarm, d);
    let mut s = Sensing {
        trail_val,
        trail_bearing,
        home_val,
        home_bearing,
        alarm_val,
        alarm_bearing,
        ..Default::default()
    };
    let dx = world.nest.x - ant.pos.x;
    let dy = world.nest.y - ant.pos.y;
    s.nest_dist = dx.hypot(dy);
    s.at_nest = s.nest_dist <= world.nest_radius.max(g.nest_radius);
    if let Some((idx, dd)) = world.nearest_food(ant.pos, g.food_radius) {
        s.food_idx = Some(idx);
        s.food_dist = dd;
    }
    if let Some(idx) = world.nearest_enemy(ant.pos, g.food_radius + 4.0) {
        let e = &world.enemies[idx];
        s.enemy_idx = Some(idx);
        s.enemy_dist = (e.pos.x - ant.pos.x).hypot(e.pos.y - ant.pos.y);
    }
    if ant.ablate_vision {
        return s;
    }
    let vis_range = g.food_radius * 5.0;
    for (i, f) in world.food.iter().enumerate() {
        if f.amount <= 0.0 {
            continue;
        }
        let dx = f.pos.x - ant.pos.x;
        let dy = f.pos.y - ant.pos.y;
        let dist = dx.hypot(dy);
        if dist <= vis_range {
            let mut rel = dy.atan2(dx) - ant.heading;
            while rel > std::f32::consts::PI {
                rel -= std::f32::consts::TAU;
            }
            while rel < -std::f32::consts::PI {
                rel += std::f32::consts::TAU;
            }
            if rel.abs() < 1.0 {
                s.vision_food_idx = Some(i);
                s.vision_food_bearing = dy.atan2(dx);
                s.vision_food_dist = dist;
                break;
            }
        }
    }
    s
}

/// Encode a physical observation into saturating, adapting receptor responses,
/// laterally inhibited glomeruli and PN/LH-style projections. It is called once
/// per ant tick, hence local noise remains deterministic under rayon.
pub fn encode(ant: &mut Ant, s: &Sensing) -> SensoryCode {
    let raw = [
        s.trail_val,
        s.home_val,
        s.alarm_val,
        ant.recruit_signal.max(0.0),
    ];
    let g = &ant.genome;
    let mut orn = [0.0; CHEM_CHANNELS];
    for i in 0..CHEM_CHANNELS {
        ant.orn_adaptation[i] += g.sensory_adapt_rate * (raw[i] - ant.orn_adaptation[i]);
        let adapted = if ant.ablate_orn {
            0.0
        } else {
            (raw[i] - g.sensory_adapt_strength * ant.orn_adaptation[i]).max(0.0)
        };
        let noise = if g.sensory_noise > 0.0 {
            ant.rng.gen_range(-g.sensory_noise..=g.sensory_noise)
        } else {
            0.0
        };
        orn[i] = ((adapted + noise).max(0.0) / (1.0 + adapted.max(0.0))).clamp(0.0, 1.0);
    }
    let total: f32 = orn.iter().sum();
    let mut al = orn;
    if !ant.ablate_al_inhibition {
        for v in &mut al {
            *v = (*v - g.al_inhibition * (total - *v) / (CHEM_CHANNELS - 1) as f32).max(0.0);
        }
    }
    let pn_single = al;
    let pn_mixed = if ant.ablate_pn_multichannel {
        [0.0; 3]
    } else {
        [
            al.iter().sum::<f32>() / CHEM_CHANNELS as f32,
            al[0] - al[2],
            al[3] - al[1],
        ]
    };
    let steer = |bearing: f32| {
        let mut d = bearing - ant.heading;
        while d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        while d < -std::f32::consts::PI {
            d += std::f32::consts::TAU;
        }
        d / std::f32::consts::PI
    };
    let lh_turn = if ant.ablate_lh_reflex {
        0.0
    } else {
        (al[0] * steer(s.trail_bearing) + al[3] * ant.recruit_bearing
            - al[2] * steer(s.alarm_bearing))
        .tanh()
    };
    let compass = if ant.ablate_compass {
        CompassObservation::default()
    } else {
        CompassObservation {
            bearing: ant.heading
                + g.compass_bias
                + if g.compass_noise > 0.0 {
                    ant.rng.gen_range(-g.compass_noise..=g.compass_noise)
                } else {
                    0.0
                },
            confidence: (1.0 - g.compass_noise / std::f32::consts::PI).clamp(0.0, 1.0),
            available: true,
        }
    };
    SensoryCode {
        raw,
        orn,
        al,
        pn_single,
        pn_mixed,
        lh_turn,
        compass,
        vision_food: s.vision_food_idx.is_some(),
        contact_signal: ant.contact_signal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::Genome;
    use crate::world::World;

    #[test]
    fn orn_adapts_to_constant_stimulus() {
        let g = Genome::default();
        let mut ant = Ant::new(Vec2::new(20.0, 20.0), 0.0, &g, 7);
        let world = World::new(64, 64, Vec2::new(32.0, 32.0), 4.0);
        let s = Sensing {
            trail_val: 1.0,
            ..Default::default()
        };
        let first = encode(&mut ant, &s).orn[0];
        let mut late = first;
        for _ in 0..100 {
            late = encode(&mut ant, &s).orn[0];
        }
        assert!(
            late < first,
            "adapted ORN response must decay under constant input"
        );
        assert!(world.width > 0);
    }
}
