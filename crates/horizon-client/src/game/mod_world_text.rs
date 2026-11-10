//! Capability `render`'s world text (`render.set-world-text`, docs/mods.md):
//! a mod's lines drawn in the world, billboarded to the camera like name
//! tags. Colours, gradients, rainbow, glow and sparkle are drawn as font
//! colours and quads; a mod never supplies shader code.
#[cfg(feature = "mods")]
use super::{Game, view_vector};
#[cfg(feature = "mods")]
use horizon_client::{
    block_entity_render::vertex_light,
    gfx::TEXT_POLYGON_OFFSET_OVERLAY,
    hud::{HudAtlas, HudBatch},
    mesher::Vertex,
    text::{Span, Style, Text},
};
#[cfg(feature = "mods")]
use std::time::{SystemTime, UNIX_EPOCH};

/// One GUI pixel of text in the world: `Matrix4f.scale(0.025F, -0.025F, 0.025F)`.
#[cfg(feature = "mods")]
const WORLD_TEXT_UNIT: f32 = 0.025;
/// A line is never wider than this in the world, in blocks: a longer line
/// shrinks its type to fit, so the slots stay readable and apart.
#[cfg(feature = "mods")]
const MAX_LINE_WIDTH: f32 = 4.0;
/// The mod's texts are near the player; beyond this they are dropped.
#[cfg(feature = "mods")]
const MAX_DISTANCE_SQUARED: f64 = 128.0 * 128.0;
/// The halo copies around a glowing line, in GUI pixels.
const GLOW_OFFSETS: [(f32, f32); 8] = [
    (-1.0, 0.0),
    (1.0, 0.0),
    (0.0, -1.0),
    (0.0, 1.0),
    (-0.7, -0.7),
    (-0.7, 0.7),
    (0.7, -0.7),
    (0.7, 0.7),
];

#[cfg(feature = "mods")]
fn split_argb(color: u32) -> (u8, u8, u8) {
    ((color >> 16) as u8, (color >> 8) as u8, color as u8)
}

#[cfg(feature = "mods")]
fn pack_rgb(r: u8, g: u8, b: u8) -> u32 {
    (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

#[cfg(feature = "mods")]
fn lerp_rgb(a: u32, b: u32, t: f32) -> u32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t.clamp(0.0, 1.0)) as u8;
    pack_rgb(
        mix((a >> 16) as u8, (b >> 16) as u8),
        mix((a >> 8) as u8, (b >> 8) as u8),
        mix(a as u8, b as u8),
    )
}

/// Full-saturation HSV at `hue` (0..1) as 0xRRGGBB.
#[cfg(feature = "mods")]
fn rainbow(hue: f32) -> u32 {
    let h = hue.rem_euclid(1.0) * 6.0;
    let i = h.floor() as u32;
    let f = h - i as f32;
    let one = f;
    let two = 1.0 - f;
    let (r, g, b) = match i {
        0 => (1.0, one, 0.0),
        1 => (two, 1.0, 0.0),
        2 => (0.0, 1.0, one),
        3 => (0.0, two, 1.0),
        4 => (one, 0.0, 1.0),
        _ => (1.0, 0.0, two),
    };
    pack_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Seconds since the epoch, for rainbow and twinkle phases.
#[cfg(feature = "mods")]
fn time_now() -> f32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f32())
        .unwrap_or(0.0)
}

/// The line's text with per-glyph colours (gradient and rainbow are per
/// glyph), as a `Text` the HUD font can draw.
#[cfg(feature = "mods")]
fn gradient_text(atlas: &HudAtlas, shape: &horizon_mods::WorldTextShape, now: f32) -> Text {
    let base = split_argb(shape.color);
    let ts: Vec<f32> = {
        let bold = shape.bold;
        let widths: Vec<f32> = shape
            .text
            .chars()
            .map(|ch| {
                let glyph = atlas.font.glyph(ch);
                glyph.advance + if bold { glyph.offset } else { 0.0 }
            })
            .collect();
        let total: f32 = widths.iter().sum();
        let mut out = Vec::with_capacity(widths.len());
        let mut x = 0.0;
        for w in &widths {
            let t = if total > 0.0 {
                (x + w * 0.5) / total
            } else {
                0.0
            };
            out.push(t);
            x += w;
        }
        out
    };
    let mut text = Text::default();
    for (i, ch) in shape.text.chars().enumerate() {
        let t = ts[i];
        let color = if shape.rainbow {
            rainbow(now * 0.33 + t * 1.5)
        } else if let Some(other) = shape.gradient {
            lerp_rgb(
                pack_rgb(base.0, base.1, base.2),
                pack_rgb((other >> 16) as u8, (other >> 8) as u8, other as u8),
                t,
            )
        } else {
            pack_rgb(base.0, base.1, base.2)
        };
        text.spans.push(Span {
            text: ch.to_string(),
            style: Style {
                color: Some(color),
                bold: shape.bold.then_some(true),
                ..Style::default()
            },
        });
    }
    text
}

#[cfg(feature = "mods")]
impl Game<'_> {
    /// `render.set-world-text`: the mods' texts, billboarded to the camera
    /// into the translucent entity pass (`see_through`: over everything).
    pub(super) fn mod_world_text_geometry(
        &mut self,
        camera: glam::DVec3,
        out: &mut Vec<Vertex>,
        see_through: &mut Vec<Vertex>,
    ) {
        let texts: Vec<horizon_mods::WorldTextShape> =
            self.mods.mods.world_texts().cloned().collect();
        if texts.is_empty() {
            return;
        }
        self.mark_translucent_entity(out.len(), "minecraft:text_display");
        let now = time_now();
        let (view_yaw, view_pitch) =
            if self.camera_type == super::perspective::CameraType::ThirdPersonFront {
                (self.yaw + 180.0, -self.pitch)
            } else {
                (self.yaw, self.pitch)
            };
        let forward = glam::DVec3::from_array(view_vector(view_yaw, view_pitch));
        let right = forward.cross(glam::DVec3::Y).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        let away = forward.as_vec3() * 0.002;
        for shape in &texts {
            let o = glam::DVec3::new(
                f64::from(shape.at[0]),
                f64::from(shape.at[1]),
                f64::from(shape.at[2]),
            ) - camera;
            if o.length_squared() > MAX_DISTANCE_SQUARED {
                continue;
            }
            let light = self.world.light(
                shape.at[0].floor() as i32,
                shape.at[1].floor() as i32,
                shape.at[2].floor() as i32,
            );
            let lit = (light >> 4).max(2) << 4 | (light & 15).max(2);
            let light = if shape.see_through {
                0
            } else {
                vertex_light(lit)
            };
            let layer = if shape.see_through {
                2
            } else {
                TEXT_POLYGON_OFFSET_OVERLAY
            };
            let atlas = &self.gui.atlas;
            let alpha = (shape.color >> 24).max(1);
            let argb = |alpha: u32, rgb: u32| (alpha << 24) | rgb;
            let text = gradient_text(atlas, shape, now);
            let width = atlas.font.width_text(&text);
            let x = -width / 2.0;
            // Long lines shrink their type to fit the world width cap, so
            // the five slots stay readable and apart.
            let raw_unit = WORLD_TEXT_UNIT * shape.scale;
            let fitted = if width * raw_unit > MAX_LINE_WIDTH {
                MAX_LINE_WIDTH / width
            } else {
                raw_unit
            };
            let o = o.as_vec3();
            let (r, u) = (right.as_vec3() * fitted, up.as_vec3() * fitted);
            let at = |x: f32, y: f32| o + r * x - u * y;
            let mut batch = HudBatch::new([1, 1]);
            // The coloured halo behind the glyphs.
            if shape.glow {
                let glow = split_argb(shape.color);
                let halo = Text::styled(
                    shape.text.as_str(),
                    Style::color(pack_rgb(glow.0, glow.1, glow.2)),
                );
                let halo_alpha = (alpha / 2).max(1);
                for (dx, dy) in GLOW_OFFSETS {
                    batch.text_styled(atlas, &halo, x + dx, dy, argb(halo_alpha, 0xFF_FFFF), false);
                }
            }
            batch.text_styled(atlas, &text, x, 0.0, argb(alpha, 0xFF_FFFF), shape.shadow);
            // Twinkles drifting over the line.
            if shape.sparkle {
                let base = split_argb(shape.color);
                for k in 0..6 {
                    let phase = now * 1.7 + k as f32 * 1.31;
                    let twinkle = 0.3 + 0.7 * (0.5 + 0.5 * (now * 3.1 + k as f32 * 1.73).sin());
                    let size = 1.0 + 1.1 * (now * 2.3 + k as f32 * 2.17).sin().abs();
                    let dx = x + ((k as f32 + 0.5) / 6.0) * width + phase.sin() * 3.0;
                    let dy = 5.5 + phase.cos() * 7.0;
                    let a = (alpha as f32 * twinkle) as u32;
                    let star = (0.65 + 0.35 * twinkle).min(1.0);
                    let r = (base.0 as f32 * (1.0 - star) + 255.0 * star) as u8;
                    let g = (base.1 as f32 * (1.0 - star) + 255.0 * star) as u8;
                    let b = (base.2 as f32 * (1.0 - star) + 255.0 * star) as u8;
                    let c = argb(a, pack_rgb(r, g, b));
                    let (cx, cy) = (dx, dy);
                    let corners = [
                        [cx, cy - size],
                        [cx + size * 0.35, cy - size * 0.35],
                        [cx + size, cy],
                        [cx + size * 0.35, cy + size * 0.35],
                        [cx, cy + size],
                        [cx - size * 0.35, cy + size * 0.35],
                        [cx - size, cy],
                        [cx - size * 0.35, cy - size * 0.35],
                    ];
                    batch.fill_quad(atlas, [corners[0], corners[2], corners[4], corners[6]], c);
                    batch.fill_quad(atlas, [corners[1], corners[3], corners[5], corners[7]], c);
                }
            }
            let target: &mut Vec<Vertex> = if shape.see_through {
                &mut *see_through
            } else {
                &mut *out
            };
            for quad in batch.vertices.as_chunks::<6>().0 {
                for v in [quad[5], quad[2], quad[1], quad[0]] {
                    target.push(Vertex {
                        normal: [0; 4],
                        pos: (at(v.pos[0], v.pos[1]) + away).to_array(),
                        uv: v.uv,
                        layer,
                        color: v.color,
                        light,
                        glint_uv: [0.0, 0.0],
                    });
                }
            }
            self.map_patches.extend(batch.patches);
        }
        self.mark_translucent_entity(out.len(), "");
    }
}
