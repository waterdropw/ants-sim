//! Sensors. The headline one is **chemotaxis**: sample a pheromone channel
//! in 8 directions around the ant and report the bearing of the maximum.
//! That gives outbound ants a real "climb the gradient toward the food"
//! direction (the deposit fades along the return trip, so Trail is densest
//! near the food and the gradient points there) — and it is what lets them
//! *choose* at a fork: the denser (shorter, better-reinforced) branch wins.

use crate::ant::Ant;
use crate::world::{Channel, Vec2, World};

#[derive(Clone, Debug, Default)]
pub struct Sensing {
    /// max Trail concentration in the 8-direction sample, and the bearing
    /// (world-frame radians) at which it was found.
    pub trail_val: f32,
    pub trail_bearing: f32,
    pub home_val: f32,
    pub home_bearing: f32,
    pub alarm_val: f32,
    pub alarm_bearing: f32,
    // proximity
    pub nest_dist: f32,
    pub at_nest: bool,
    pub food_idx: Option<usize>,
    pub food_dist: f32,
    pub enemy_idx: Option<usize>,
    pub enemy_dist: f32,
    /// T7.6 simple vision: food seen in a forward cone (longer range than
    /// contact food_radius). Ants can head toward visible food.
    pub vision_food_idx: Option<usize>,
    /// T10 multi-modal: world-frame bearing + distance to the seen food, so the
    /// MB can integrate vision with olfaction (cross-modal association).
    pub vision_food_bearing: f32,
    pub vision_food_dist: f32,
}

/// Sample `ch` in 8 directions at `dist`, return (max value, world-frame bearing).
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

    // nest
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
    // T7.6 simple vision: food in a forward cone at 5× contact range.
    // Biologically: compound eyes detect visual landmarks/food at distance.
    // T22 ablation: skip visual sensing (blinding analog).
    if ant.ablate_vision {
        return s;
    }
    let vis_range = g.food_radius * 5.0;
    for (i, f) in world.food.iter().enumerate() {
        if f.amount <= 0.0 { continue; }
        let dx = f.pos.x - ant.pos.x;
        let dy = f.pos.y - ant.pos.y;
        let dist = dx.hypot(dy);
        if dist <= vis_range {
            let mut rel = dy.atan2(dx) - ant.heading;
            while rel > std::f32::consts::PI { rel -= std::f32::consts::TAU; }
            while rel < -std::f32::consts::PI { rel += std::f32::consts::TAU; }
            if rel.abs() < 1.0 { // ~57° forward cone
                s.vision_food_idx = Some(i);
                s.vision_food_bearing = dy.atan2(dx); // world-frame
                s.vision_food_dist = dist;
                break;
            }
        }
    }
    s
}
