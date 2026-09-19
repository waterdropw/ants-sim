//! Field -> ColorImage conversion for egui textures.

use crate::world::{Channel, World};
use egui::ColorImage;

/// Build an RGBA image of one channel, scaled to a visible range.
/// `auto_gain`: normalize so the field's current max maps to full brightness —
/// keeps faint trails visible and, crucially, preserves *relative* differences
/// so a denser trail reads brighter than a sparser one instead of both clipping
/// to the same saturated yellow.
pub fn render_channel(world: &World, ch: Channel, scale: f32, auto_gain: bool) -> ColorImage {
    let w = world.width;
    let h = world.height;
    let src = world.field.channel_slice(ch);
    let eff_scale = if auto_gain {
        let m = world.field.max_val(ch);
        if m > 1e-6 {
            1.0 / m
        } else {
            scale
        }
    } else {
        scale
    };
    let mut pixels = Vec::with_capacity(w * h);
    for &v in src {
        let n = (v * eff_scale).clamp(0.0, 1.0);
        pixels.push(viridis(n));
    }
    ColorImage::new([w, h], pixels)
}

fn viridis(t: f32) -> egui::Color32 {
    // cheap viridis-ish ramp: dark blue -> teal -> green -> yellow
    let t = t.clamp(0.0, 1.0);
    let r = (255.0 * (0.267 + 1.0 * t)) as u8;
    let g = (255.0 * (0.0 + 1.0 * t).min(1.0)) as u8;
    let b = (255.0 * (0.33 - 0.33 * t).max(0.0)) as u8;
    // brighten at high values
    egui::Color32::from_rgb(r, g, b)
}
