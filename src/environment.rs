//! Environment presets — different ecological pressures that select for
//! different genomes (the gene×environment theme). Each preset fixes a food
//! layout, enemy load and obstacle field around the nest; `EnvMetrics`
//! summarises it so we can correlate selection pressure with evolved traits.

use crate::world::{Enemy, FoodSource, Vec2, Wall};

#[derive(Clone, Debug, Default)]
pub struct EnvMetrics {
    pub food_count: usize,
    pub mean_food_dist: f32,
    pub enemy_count: usize,
    pub obstacle_cells: usize,
}

#[derive(Clone, Debug)]
pub struct Environment {
    pub name: String,
    pub foods: Vec<FoodSource>,
    pub enemies: Vec<Enemy>,
    pub walls: Vec<Wall>,
    pub metrics: EnvMetrics,
}

impl Environment {
    pub fn build(name: &str, nest: Vec2, w: f32, h: f32) -> Environment {
        let big = 1_000_000.0;
        let mk = |x: f32, y: f32| FoodSource {
            pos: Vec2::new(x, y),
            radius: 4.0,
            amount: big,
        };
        let e_at = |x: f32, y: f32| Enemy {
            pos: Vec2::new(x, y),
            radius: 2.0,
            health: 10.0,
        };

        let (foods, enemies, walls): (Vec<FoodSource>, Vec<Enemy>, Vec<Wall>) = match name {
            // Abundance + proximity: low foraging pressure.
            "rich_close" => (
                vec![
                    mk(nest.x + 40.0, nest.y),
                    mk(nest.x - 40.0, nest.y),
                    mk(nest.x, nest.y + 40.0),
                    mk(nest.x, nest.y - 40.0),
                ],
                vec![],
                vec![],
            ),
            // Scarcity + distance: high foraging pressure, favours strong trails.
            "scarce_far" => (vec![mk(w - 30.0, nest.y)], vec![], vec![]),
            // Predator pressure on foraging routes (not at the nest mouth):
            // 2 enemies ~28 cells out so alarm is localised — foraging can
            // continue while guards engage. Favours defence + enough foraging.
            "predator" => (
                vec![mk(nest.x + 60.0, nest.y), mk(nest.x - 60.0, nest.y)],
                vec![
                    e_at(nest.x + 28.0, nest.y + 6.0),
                    e_at(nest.x - 28.0, nest.y - 6.0),
                ],
                vec![],
            ),
            // Patchy clustered food: favours exploitation (trail-following).
            "patchy" => (
                vec![
                    mk(nest.x + 70.0, nest.y + 30.0),
                    mk(nest.x + 75.0, nest.y + 40.0),
                    mk(nest.x - 65.0, nest.y - 35.0),
                    mk(nest.x - 70.0, nest.y - 25.0),
                ],
                vec![],
                vec![],
            ),
            // Obstacles forcing detours: favours routing/turn behaviour.
            "maze" => (
                vec![mk(w - 30.0, nest.y)],
                vec![],
                vec![
                    Wall {
                        x0: w * 0.45,
                        y0: h * 0.20,
                        x1: w * 0.47,
                        y1: h * 0.50,
                    },
                    Wall {
                        x0: w * 0.55,
                        y0: h * 0.50,
                        x1: w * 0.57,
                        y1: h * 0.80,
                    },
                    Wall {
                        x0: w * 0.40,
                        y0: h * 0.62,
                        x1: w * 0.60,
                        y1: h * 0.64,
                    },
                ],
            ),
            _ => (vec![mk(nest.x + 50.0, nest.y)], vec![], vec![]),
        };

        let food_count = foods.len();
        let mean_food_dist = if foods.is_empty() {
            0.0
        } else {
            foods
                .iter()
                .map(|f| (f.pos.x - nest.x).hypot(f.pos.y - nest.y))
                .sum::<f32>()
                / food_count as f32
        };
        let obstacle_cells: usize = walls
            .iter()
            .map(|w| ((w.x1 - w.x0).max(1.0) * (w.y1 - w.y0).max(1.0)) as usize)
            .sum();

        Environment {
            name: name.to_string(),
            foods,
            enemies,
            walls,
            metrics: EnvMetrics {
                food_count,
                mean_food_dist,
                enemy_count: 0,
                obstacle_cells,
            },
        }
        .with_enemy_count()
    }

    fn with_enemy_count(mut self) -> Self {
        self.metrics.enemy_count = self.enemies.len();
        self
    }

    pub fn presets() -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "rich_close",
                "abundant food close to nest — low foraging pressure",
            ),
            ("scarce_far", "single distant food — high foraging pressure"),
            ("predator", "enemies near nest — defense pressure"),
            ("patchy", "clustered food patches — exploitation pressure"),
            ("maze", "walls forcing detours — routing pressure"),
        ]
    }
}
