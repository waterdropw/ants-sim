//! Spatial hash for ant-ant neighbor queries (collisions, antenna contact,
//! alarm recruitment). Rebuilt each tick from current positions.
//!
//! Not on the hot path in M3/M4 — recruitment there is routed through the
//! alarm-pheromone field rather than explicit neighbour queries — but kept
//! for later phases (collision avoidance, tactile antennation).

#![allow(dead_code)]

use crate::world::Vec2;

const CELL: usize = 4; // world cells per hash bucket

pub struct SpatialHash {
    cell: usize,
    bw: usize,
    bh: usize,
    buckets: Vec<Vec<usize>>,
}

impl SpatialHash {
    pub fn new(width: usize, height: usize) -> Self {
        let bw = (width + CELL - 1) / CELL;
        let bh = (height + CELL - 1) / CELL;
        Self {
            cell: CELL,
            bw,
            bh,
            buckets: vec![Vec::new(); bw * bh],
        }
    }

    fn bucket(&self, x: f32, y: f32) -> Option<usize> {
        let bx = (x as usize / self.cell) as isize;
        let by = (y as usize / self.cell) as isize;
        if bx < 0 || by < 0 || bx >= self.bw as isize || by >= self.bh as isize {
            None
        } else {
            Some((by * self.bw as isize + bx) as usize)
        }
    }

    pub fn rebuild(&mut self, positions: &[Vec2]) {
        for b in self.buckets.iter_mut() {
            b.clear();
        }
        for (i, p) in positions.iter().enumerate() {
            if let Some(b) = self.bucket(p.x, p.y) {
                self.buckets[b].push(i);
            }
        }
    }

    /// Visit all ant indices within `radius` of `pos`, calling `f(idx, dist_sq)`.
    pub fn query_near<F: FnMut(usize, f32)>(
        &self,
        positions: &[Vec2],
        pos: Vec2,
        radius: f32,
        mut f: F,
    ) {
        let r = ((radius / self.cell as f32).ceil() as isize).max(1);
        let bx = pos.x as usize / self.cell;
        let by = pos.y as usize / self.cell;
        let r2 = radius * radius;
        for dy in -r..=r {
            let yy = by as isize + dy;
            if yy < 0 || yy >= self.bh as isize {
                continue;
            }
            for dx in -r..=r {
                let xx = bx as isize + dx;
                if xx < 0 || xx >= self.bw as isize {
                    continue;
                }
                let bucket = (yy * self.bw as isize + xx) as usize;
                for &idx in &self.buckets[bucket] {
                    let p = positions[idx];
                    let ddx = p.x - pos.x;
                    let ddy = p.y - pos.y;
                    let d2 = ddx * ddx + ddy * ddy;
                    if d2 <= r2 {
                        f(idx, d2);
                    }
                }
            }
        }
    }
}
