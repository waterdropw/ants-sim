//! Pheromone field — multi-channel, double-buffered diffusion + evaporation.
//!
//! Channel physics per tick:
//!     C_next = (1 - decay) * (C + D * laplacian(C)) + deposits
//! Diffusion uses a 5-point (von Neumann) stencil with reflective borders.

use rayon::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Channel {
    Trail,
    Home,
    Alarm,
    Recruitment,
}

pub const CHANNELS: [Channel; 4] = [
    Channel::Trail,
    Channel::Home,
    Channel::Alarm,
    Channel::Recruitment,
];

pub struct PheromoneField {
    pub width: usize,
    pub height: usize,
    /// one buffer per channel, double-buffered
    buf: [Vec<f32>; 4],
    next: [Vec<f32>; 4],
}

impl PheromoneField {
    pub fn new(width: usize, height: usize) -> Self {
        let z = || vec![0.0; width * height];
        Self {
            width,
            height,
            buf: [z(), z(), z(), z()],
            next: [z(), z(), z(), z()],
        }
    }

    fn idx(ch: Channel) -> usize {
        match ch {
            Channel::Trail => 0,
            Channel::Home => 1,
            Channel::Alarm => 2,
            Channel::Recruitment => 3,
        }
    }

    /// Read concentration at integer cell (out of range => 0).
    pub fn at(&self, ch: Channel, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            0.0
        } else {
            self.buf[Self::idx(ch)][y as usize * self.width + x as usize]
        }
    }

    /// Bilinear sample at floating point world coordinates.
    pub fn sample(&self, ch: Channel, fx: f32, fy: f32) -> f32 {
        let x = fx.clamp(0.0, (self.width - 1) as f32);
        let y = fy.clamp(0.0, (self.height - 1) as f32);
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let x1 = (x0 + 1).min(self.width as i32 - 1);
        let y1 = (y0 + 1).min(self.height as i32 - 1);
        let tx = x - x0 as f32;
        let ty = y - y0 as f32;
        let c00 = self.at(ch, x0, y0);
        let c10 = self.at(ch, x1, y0);
        let c01 = self.at(ch, x0, y1);
        let c11 = self.at(ch, x1, y1);
        let top = c00 * (1.0 - tx) + c10 * tx;
        let bot = c01 * (1.0 - tx) + c11 * tx;
        top * (1.0 - ty) + bot * ty
    }

    /// Deposit concentration at a cell (additive, used during the ant step
    /// before `step` is called — writes go to the live buffer).
    pub fn deposit(&mut self, ch: Channel, x: i32, y: i32, amount: f32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let i = y as usize * self.width + x as usize;
        self.buf[Self::idx(ch)][i] += amount;
    }

    /// Sample the max channel for visualization scaling.
    pub fn max_val(&self, ch: Channel) -> f32 {
        let b = &self.buf[Self::idx(ch)];
        b.par_iter().cloned().reduce(|| 0.0f32, f32::max)
    }

    /// Raw buffer slice for a channel (for rendering).
    pub fn channel_slice(&self, ch: Channel) -> &[f32] {
        &self.buf[Self::idx(ch)]
    }

    /// Advance chemistry: diffuse + evaporate, in parallel over channels.
    /// `diffusivity[ci]` in cells^2/tick; `decay[ci]` per tick (fraction lost).
    pub fn step(&mut self, diffusivity: &[f32; 4], decay: &[f32; 4]) {
        let w = self.width;
        let h = self.height;
        for ci in 0..4 {
            let src = &self.buf[ci];
            let dst = &mut self.next[ci];
            let d = diffusivity[ci];
            let k = decay[ci];
            dst.par_iter_mut().enumerate().for_each(|(i, out)| {
                let x = i % w;
                let y = i / w;
                let c = src[i];
                let mut n = 0.0f32;
                let mut count = 0.0f32;
                if x > 0 {
                    n += src[i - 1];
                    count += 1.0;
                }
                if x + 1 < w {
                    n += src[i + 1];
                    count += 1.0;
                }
                if y > 0 {
                    n += src[i - w];
                    count += 1.0;
                }
                if y + 1 < h {
                    n += src[i + w];
                    count += 1.0;
                }
                let lap = n - c * count;
                let diffused = c + d * lap;
                *out = (diffused * (1.0 - k)).max(0.0);
            });
            std::mem::swap(&mut self.buf[ci], &mut self.next[ci]);
        }
    }

    /// Mean concentration in an axis-aligned box around (cx, cy) of half-size r.
    /// Telemetry: compare corridor density near one food source vs another.
    pub fn mean_in_rect(&self, ch: Channel, cx: f32, cy: f32, r: f32) -> f32 {
        let x0 = ((cx - r).max(0.0)) as i32;
        let y0 = ((cy - r).max(0.0)) as i32;
        let x1 = ((cx + r).min(self.width as f32 - 1.0)) as i32;
        let y1 = ((cy + r).min(self.height as f32 - 1.0)) as i32;
        let mut sum = 0.0f32;
        let mut n = 0.0f32;
        let mut y = y0;
        while y <= y1 {
            let mut x = x0;
            while x <= x1 {
                sum += self.at(ch, x, y);
                n += 1.0;
                x += 1;
            }
            y += 1;
        }
        if n > 0.0 {
            sum / n
        } else {
            0.0
        }
    }

    /// Max concentration in an axis-aligned box around (cx, cy) of half-size r.
    pub fn max_in_rect(&self, ch: Channel, cx: f32, cy: f32, r: f32) -> f32 {
        let x0 = ((cx - r).max(0.0)) as i32;
        let y0 = ((cy - r).max(0.0)) as i32;
        let x1 = ((cx + r).min(self.width as f32 - 1.0)) as i32;
        let y1 = ((cy + r).min(self.height as f32 - 1.0)) as i32;
        let mut m = 0.0f32;
        let mut y = y0;
        while y <= y1 {
            let mut x = x0;
            while x <= x1 {
                m = m.max(self.at(ch, x, y));
                x += 1;
            }
            y += 1;
        }
        m
    }

    pub fn clear(&mut self) {
        for b in self.buf.iter_mut() {
            for v in b.iter_mut() {
                *v = 0.0;
            }
        }
    }
}
