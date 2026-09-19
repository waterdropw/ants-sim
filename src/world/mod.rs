//! World — grid environment: pheromone field, nest, food sources, enemies.
//! Ants read the world (sensing) and write pheromone deposits back through it.

pub mod pheromone;
pub mod spatial_hash;

pub use pheromone::{Channel, PheromoneField};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn len(self) -> f32 {
        self.x.hypot(self.y)
    }
    pub fn normalize(self) -> Self {
        let l = self.len();
        if l < 1e-9 {
            Self::new(1.0, 0.0)
        } else {
            Self::new(self.x / l, self.y / l)
        }
    }
    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FoodSource {
    pub pos: Vec2,
    pub radius: f32,
    pub amount: f32, // remaining food
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Enemy {
    pub pos: Vec2,
    pub radius: f32,
    pub health: f32,
}

/// Axis-aligned wall rectangle. Ants cannot enter; they slide along it.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Wall {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

pub struct World {
    pub width: usize,
    pub height: usize,
    pub field: PheromoneField,
    pub nest: Vec2,
    pub nest_radius: f32,
    pub food: Vec<FoodSource>,
    pub enemies: Vec<Enemy>,
    pub walls: Vec<Wall>,
    /// brood biomass in the nest (T4.1): nurses tend it (growth), neglect
    /// starves it (decay).
    pub brood: f32,
    /// per-channel chemistry
    pub diffusivity: [f32; 4],
    pub decay: [f32; 4],
}

impl World {
    pub fn new(width: usize, height: usize, nest: Vec2, nest_radius: f32) -> Self {
        let mut decay = [0.015; 4];
        decay[Self::ch_idx(Channel::Alarm)] = 0.025;
        decay[Self::ch_idx(Channel::Home)] = 0.006;
        // Low diffusivity for Trail/Home so corridors *accumulate* instead of
        // leaking to neighbours as fast as ants lay them (4-neighbour stencil
        // bleeds 4*d per tick, so d must stay small).
        let diffusivity = [0.01, 0.015, 0.10, 0.05];
        Self {
            width,
            height,
            field: PheromoneField::new(width, height),
            nest,
            nest_radius,
            food: Vec::new(),
            enemies: Vec::new(),
            walls: Vec::new(),
            brood: 20.0,
            diffusivity,
            decay,
        }
    }

    pub fn ch_idx(ch: Channel) -> usize {
        match ch {
            Channel::Trail => 0,
            Channel::Home => 1,
            Channel::Alarm => 2,
            Channel::Recruitment => 3,
        }
    }

    pub fn sample(&self, ch: Channel, x: f32, y: f32) -> f32 {
        self.field.sample(ch, x, y)
    }

    pub fn deposit(&mut self, ch: Channel, x: i32, y: i32, amount: f32) {
        self.field.deposit(ch, x, y, amount);
    }

    /// Is a continuous-space point inside any wall?
    pub fn is_wall(&self, fx: f32, fy: f32) -> bool {
        for w in &self.walls {
            if fx >= w.x0 && fx <= w.x1 && fy >= w.y0 && fy <= w.y1 {
                return true;
            }
        }
        false
    }

    pub fn step_chemistry(&mut self) {
        let d = self.diffusivity;
        let k = self.decay;
        self.field.step(&d, &k);
    }

    /// Nearest food source within sensing radius. Returns index + distance.
    pub fn nearest_food(&self, pos: Vec2, radius: f32) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (i, f) in self.food.iter().enumerate() {
            if f.amount <= 0.0 {
                continue;
            }
            let d = (f.pos.x - pos.x).hypot(f.pos.y - pos.y);
            if d <= f.radius + radius {
                match best {
                    Some((_, bd)) if bd <= d => {}
                    _ => best = Some((i, d)),
                }
            }
        }
        best
    }

    pub fn at_nest(&self, pos: Vec2) -> bool {
        (self.nest.x - pos.x).hypot(self.nest.y - pos.y) <= self.nest_radius
    }

    pub fn nearest_enemy(&self, pos: Vec2, radius: f32) -> Option<usize> {
        self.enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| (e.pos.x - pos.x).hypot(e.pos.y - pos.y) <= e.radius + radius)
            .map(|(i, e)| (i, (e.pos.x - pos.x).hypot(e.pos.y - pos.y)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(i, _)| i)
    }
}
