use crate::font::TtfUiFont;
use alloc::vec;
use alloc::vec::Vec;
use mmo_protocol::{abs_f32, ceil_f32, cos_approx, floor_f32, sin_approx, sqrt_f32};

#[derive(Clone, Copy)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }

    #[inline]
    pub fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }

    #[inline]
    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    #[inline]
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    #[inline]
    pub fn normalize(self) -> Self {
        let len = sqrt_f32(self.dot(self));
        if len > 1e-5 {
            Self::new(self.x / len, self.y / len, self.z / len)
        } else {
            Self::new(0.0, 1.0, 0.0)
        }
    }

    #[inline]
    pub fn rotate_y(self, yaw: f32) -> Self {
        let s = sin_approx(yaw);
        let c = cos_approx(yaw);
        Self::new(self.x * c + self.z * s, self.y, -self.x * s + self.z * c)
    }
}

#[derive(Clone, Copy)]
pub struct Camera {
    pub pos: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub focal: f32,
}

impl Camera {
    pub fn look_at(pos: Vec3, target: Vec3, width: usize) -> Self {
        let forward = target.sub(pos).normalize();
        let world_up = Vec3::new(0.0, 1.0, 0.0);
        let right = forward.cross(world_up).normalize();
        let up = right.cross(forward).normalize();
        Self {
            pos,
            right,
            up,
            forward,
            focal: (width as f32) * 0.88,
        }
    }

    #[inline]
    pub fn world_to_cam(&self, p: Vec3) -> Vec3 {
        let rel = p.sub(self.pos);
        Vec3::new(rel.dot(self.right), rel.dot(self.up), rel.dot(self.forward))
    }

    #[inline]
    pub fn project_point(&self, p: Vec3, w: usize, h: usize) -> Option<(f32, f32, f32)> {
        let c = self.world_to_cam(p);
        if c.z <= 0.35 {
            return None;
        }
        let inv_z = self.focal / c.z;
        let sx = (w as f32) * 0.5 + c.x * inv_z;
        let sy = (h as f32) * 0.5 - c.y * inv_z;
        Some((sx, sy, c.z))
    }
}

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
    pub gpu_background_pixels: Vec<u32>,
    pub depth: Vec<f32>,
    pub scene_vertices: Vec<SceneVertex>,
    pub ui_pixels: Vec<u32>,
    gpu_scene_mode: bool,
    ui_capture: bool,
    ui_font: TtfUiFont,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SceneVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

const SUN_DIR: Vec3 = Vec3 {
    x: 0.48,
    y: 0.78,
    z: 0.40,
};

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        let len = width * height;
        Self {
            width,
            height,
            pixels: vec![0; len],
            gpu_background_pixels: vec![0; len],
            depth: vec![1e9; len],
            scene_vertices: Vec::new(),
            ui_pixels: vec![0; len],
            gpu_scene_mode: false,
            ui_capture: false,
            ui_font: TtfUiFont::new(),
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        let len = width * height;
        self.pixels.resize(len, 0);
        self.gpu_background_pixels.resize(len, 0);
        self.depth.resize(len, 1e9);
        self.ui_pixels.resize(len, 0);
    }

    pub fn clear_sky(&mut self) {
        self.depth.fill(1e9);
        self.scene_vertices.clear();
        self.ui_pixels.fill(0);
        self.ui_capture = false;
        let h = self.height.max(1);
        for y in 0..self.height {
            let t = y as f32 / h as f32;
            // Atmospheric horizon gradient with warmer daylight near the horizon.
            let r = (55.0 + t * 65.0) as u32;
            let g = (95.0 + t * 78.0) as u32;
            let b = (142.0 + t * 66.0) as u32;
            let col = (r << 16) | (g << 8) | b;
            let row_start = y * self.width;
            let row_end = row_start + self.width;
            self.pixels[row_start..row_end].fill(col);
        }

        // Soft sun disc and a few thin clouds give the world a more readable sky.
        let sun_x = (self.width as i32 * 78) / 100;
        let sun_y = (self.height as i32 * 18) / 100;
        for dy in -14..=14 {
            for dx in -14..=14 {
                let d2 = dx * dx + dy * dy;
                if d2 <= 196 {
                    let x = sun_x + dx;
                    let y = sun_y + dy;
                    if x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32 {
                        let glow = if d2 <= 90 { 0xFFF4C4 } else { 0xF6D58A };
                        self.pixels[(y as usize) * self.width + x as usize] = glow;
                    }
                }
            }
        }
        let cloud_rows = [
            ((self.height as i32 * 12) / 100).max(18),
            ((self.height as i32 * 27) / 100).max(48),
        ];
        for (idx, cy) in cloud_rows.iter().enumerate() {
            let cx = if idx == 0 {
                (self.width as i32 * 18) / 100
            } else {
                (self.width as i32 * 56) / 100
            };
            for oy in -2..=2 {
                for ox in -34..=34 {
                    let x = cx + ox;
                    let y = cy + oy;
                    let edge = (ox * ox) / 1152 + (oy * oy) / 5;
                    if edge < 1
                        && x >= 0
                        && y >= 0
                        && x < self.width as i32
                        && y < self.height as i32
                    {
                        self.pixels[(y as usize) * self.width + x as usize] = 0xEAF2F7;
                    }
                }
            }
            self.gpu_background_pixels.copy_from_slice(&self.pixels);
        }
    }

    #[inline]
    pub fn shade_color(base_rgb: u32, normal: Vec3, depth_z: f32, emissive: bool) -> u32 {
        let br = ((base_rgb >> 16) & 0xFF) as f32;
        let bg = ((base_rgb >> 8) & 0xFF) as f32;
        let bb = (base_rgb & 0xFF) as f32;

        let light = if emissive {
            1.15
        } else {
            let ndotl = normal.dot(SUN_DIR).max(0.0);
            let hemi = 0.42 + (normal.y * 0.5 + 0.5) * 0.24;
            (hemi + ndotl * 0.72).min(1.18)
        };

        let mut r = (br * light).min(255.0);
        let mut g = (bg * light).min(255.0);
        let mut b = (bb * light).min(255.0);

        // Exponential horizon fog matching sky color (112, 155, 198)
        if depth_z > 28.0 {
            let fog = ((depth_z - 28.0) / 115.0).clamp(0.0, 0.82);
            r = r + (112.0 - r) * fog;
            g = g + (155.0 - g) * fog;
            b = b + (198.0 - b) * fog;
        }

        (((r as u32) & 0xFF) << 16) | (((g as u32) & 0xFF) << 8) | ((b as u32) & 0xFF)
    }

    pub fn draw_triangle(
        &mut self,
        cam: &Camera,
        p0: Vec3,
        p1: Vec3,
        p2: Vec3,
        base_rgb: u32,
        emissive: bool,
    ) {
        let c0 = cam.world_to_cam(p0);
        let c1 = cam.world_to_cam(p1);
        let c2 = cam.world_to_cam(p2);

        if c0.z <= 0.45 || c1.z <= 0.45 || c2.z <= 0.45 {
            return;
        }
        if c0.z > 145.0 && c1.z > 145.0 && c2.z > 145.0 {
            return;
        }

        // Face normal in world space
        let e1 = p1.sub(p0);
        let e2 = p2.sub(p0);
        let normal = e1.cross(e2).normalize();

        let w = self.width as f32;
        let h = self.height as f32;

        let x0 = w * 0.5 + c0.x * cam.focal / c0.z;
        let y0 = h * 0.5 - c0.y * cam.focal / c0.z;
        let x1 = w * 0.5 + c1.x * cam.focal / c1.z;
        let y1 = h * 0.5 - c1.y * cam.focal / c1.z;
        let x2 = w * 0.5 + c2.x * cam.focal / c2.z;
        let y2 = h * 0.5 - c2.y * cam.focal / c2.z;

        let area = (x1 - x0) * (y2 - y0) - (y1 - y0) * (x2 - x0);
        if abs_f32(area) < 0.25 {
            return;
        }

        let min_x = floor_f32(x0.min(x1).min(x2)).max(0);
        let max_x = ceil_f32(x0.max(x1).max(x2)).min(self.width as i32 - 1);
        let min_y = floor_f32(y0.min(y1).min(y2)).max(0);
        let max_y = ceil_f32(y0.max(y1).max(y2)).min(self.height as i32 - 1);

        if min_x > max_x || min_y > max_y {
            return;
        }

        let avg_z = (c0.z + c1.z + c2.z) * 0.333333;
        let col = Self::shade_color(base_rgb, normal, avg_z, emissive);

        if self.gpu_scene_mode {
            let color = [
                ((col >> 16) & 0xFF) as f32 / 255.0,
                ((col >> 8) & 0xFF) as f32 / 255.0,
                (col & 0xFF) as f32 / 255.0,
            ];
            let projected = [(x0, y0, c0.z), (x1, y1, c1.z), (x2, y2, c2.z)];
            for (x, y, depth) in projected {
                self.scene_vertices.push(SceneVertex {
                    position: [
                        (x / w) * 2.0 - 1.0,
                        1.0 - (y / h) * 2.0,
                        (145.0 / 144.65 * (1.0 - 0.35 / depth)).clamp(0.0, 1.0),
                    ],
                    color,
                });
            }
            return;
        }

        let inv_area = 1.0 / area;
        let inv_z0 = 1.0 / c0.z;
        let inv_z1 = 1.0 / c1.z;
        let inv_z2 = 1.0 / c2.z;

        for py in min_y..=max_y {
            let fy = py as f32 + 0.5;
            let row = (py as usize) * self.width;
            for px in min_x..=max_x {
                let fx = px as f32 + 0.5;
                let w0 = ((x1 - fx) * (y2 - fy) - (y1 - fy) * (x2 - fx)) * inv_area;
                let w1 = ((x2 - fx) * (y0 - fy) - (y2 - fy) * (x0 - fx)) * inv_area;
                let w2 = 1.0 - w0 - w1;
                if w0 >= -0.001 && w1 >= -0.001 && w2 >= -0.001 {
                    let iz = w0 * inv_z0 + w1 * inv_z1 + w2 * inv_z2;
                    let z = 1.0 / iz;
                    let idx = row + (px as usize);
                    if z < self.depth[idx] {
                        self.depth[idx] = z;
                        self.pixels[idx] = col;
                    }
                }
            }
        }
    }

    pub fn draw_quad(
        &mut self,
        cam: &Camera,
        a: Vec3,
        b: Vec3,
        c: Vec3,
        d: Vec3,
        rgb: u32,
        emissive: bool,
    ) {
        self.draw_triangle(cam, a, b, c, rgb, emissive);
        self.draw_triangle(cam, a, c, d, rgb, emissive);
    }

    pub fn draw_box(
        &mut self,
        cam: &Camera,
        center: Vec3,
        half: Vec3,
        yaw: f32,
        rgb: u32,
        emissive: bool,
    ) {
        let corners = [
            Vec3::new(-half.x, -half.y, -half.z)
                .rotate_y(yaw)
                .add(center),
            Vec3::new(half.x, -half.y, -half.z)
                .rotate_y(yaw)
                .add(center),
            Vec3::new(half.x, half.y, -half.z).rotate_y(yaw).add(center),
            Vec3::new(-half.x, half.y, -half.z)
                .rotate_y(yaw)
                .add(center),
            Vec3::new(-half.x, -half.y, half.z)
                .rotate_y(yaw)
                .add(center),
            Vec3::new(half.x, -half.y, half.z).rotate_y(yaw).add(center),
            Vec3::new(half.x, half.y, half.z).rotate_y(yaw).add(center),
            Vec3::new(-half.x, half.y, half.z).rotate_y(yaw).add(center),
        ];
        // Front, Back, Top, Bottom, Right, Left
        self.draw_quad(
            cam, corners[4], corners[5], corners[6], corners[7], rgb, emissive,
        );
        self.draw_quad(
            cam, corners[1], corners[0], corners[3], corners[2], rgb, emissive,
        );
        self.draw_quad(
            cam, corners[3], corners[2], corners[6], corners[7], rgb, emissive,
        );
        self.draw_quad(
            cam, corners[0], corners[1], corners[5], corners[4], rgb, emissive,
        );
        self.draw_quad(
            cam, corners[5], corners[1], corners[2], corners[6], rgb, emissive,
        );
        self.draw_quad(
            cam, corners[0], corners[4], corners[7], corners[3], rgb, emissive,
        );
    }

    pub fn draw_pyramid(
        &mut self,
        cam: &Camera,
        base_center: Vec3,
        radius: f32,
        height: f32,
        yaw: f32,
        rgb: u32,
        emissive: bool,
    ) {
        let apex = base_center.add(Vec3::new(0.0, height, 0.0));
        let p0 = Vec3::new(-radius, 0.0, -radius)
            .rotate_y(yaw)
            .add(base_center);
        let p1 = Vec3::new(radius, 0.0, -radius)
            .rotate_y(yaw)
            .add(base_center);
        let p2 = Vec3::new(radius, 0.0, radius)
            .rotate_y(yaw)
            .add(base_center);
        let p3 = Vec3::new(-radius, 0.0, radius)
            .rotate_y(yaw)
            .add(base_center);
        self.draw_triangle(cam, p0, p1, apex, rgb, emissive);
        self.draw_triangle(cam, p1, p2, apex, rgb, emissive);
        self.draw_triangle(cam, p2, p3, apex, rgb, emissive);
        self.draw_triangle(cam, p3, p0, apex, rgb, emissive);
    }

    pub fn draw_ring_3d(&mut self, cam: &Camera, center: Vec3, r_in: f32, r_out: f32, rgb: u32) {
        let segs = 14;
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * 6.283185;
            let a1 = ((i + 1) as f32 / segs as f32) * 6.283185;
            let p0 = center.add(Vec3::new(cos_approx(a0) * r_in, 0.0, sin_approx(a0) * r_in));
            let p1 = center.add(Vec3::new(
                cos_approx(a0) * r_out,
                0.0,
                sin_approx(a0) * r_out,
            ));
            let p2 = center.add(Vec3::new(
                cos_approx(a1) * r_out,
                0.0,
                sin_approx(a1) * r_out,
            ));
            let p3 = center.add(Vec3::new(cos_approx(a1) * r_in, 0.0, sin_approx(a1) * r_in));
            self.draw_quad(cam, p0, p1, p2, p3, rgb, true);
        }
    }

    // 2D HUD Drawing Helpers
    pub fn set_gpu_scene_mode(&mut self, enabled: bool) {
        self.gpu_scene_mode = enabled;
    }

    pub fn begin_ui_capture(&mut self) {
        self.ui_capture = true;
        self.ui_pixels.fill(0);
    }

    pub fn end_ui_capture(&mut self) {
        self.ui_capture = false;
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, rgb: u32) {
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(self.width as i32);
        let y1 = (y + h).min(self.height as i32);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for py in y0..y1 {
            let row = (py as usize) * self.width;
            for px in x0..x1 {
                let idx = row + (px as usize);
                self.pixels[idx] = rgb;
                if self.ui_capture {
                    self.ui_pixels[idx] = 0xFF00_0000 | (rgb & 0x00FF_FFFF);
                }
            }
        }
    }

    pub fn fill_rect_alpha(&mut self, x: i32, y: i32, w: i32, h: i32, rgb: u32, alpha_u8: u32) {
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(self.width as i32);
        let y1 = (y + h).min(self.height as i32);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let sr = (rgb >> 16) & 0xFF;
        let sg = (rgb >> 8) & 0xFF;
        let sb = rgb & 0xFF;
        let a = alpha_u8;
        let inv = 255 - a;
        for py in y0..y1 {
            let row = (py as usize) * self.width;
            for px in x0..x1 {
                let dst = self.pixels[row + (px as usize)];
                let dr = ((dst >> 16) & 0xFF) * inv + sr * a;
                let dg = ((dst >> 8) & 0xFF) * inv + sg * a;
                let db = (dst & 0xFF) * inv + sb * a;
                let idx = row + (px as usize);
                self.pixels[idx] = ((dr / 255) << 16) | ((dg / 255) << 8) | (db / 255);
                if self.ui_capture {
                    let old = self.ui_pixels[idx];
                    let old_a = (old >> 24) & 0xFF;
                    let out_a = a + (old_a * inv) / 255;
                    let blend = |src: u32, dst: u32| {
                        if out_a == 0 {
                            0
                        } else {
                            (src * a + dst * old_a * inv / 255) / out_a
                        }
                    };
                    let r = blend(sr, (old >> 16) & 0xFF);
                    let g = blend(sg, (old >> 8) & 0xFF);
                    let b = blend(sb, old & 0xFF);
                    self.ui_pixels[idx] = (out_a << 24) | (r << 16) | (g << 8) | b;
                }
            }
        }
    }

    pub fn stroke_rect(&mut self, x: i32, y: i32, w: i32, h: i32, rgb: u32) {
        self.fill_rect(x, y, w, 1, rgb);
        self.fill_rect(x, y + h - 1, w, 1, rgb);
        self.fill_rect(x, y, 1, h, rgb);
        self.fill_rect(x + w - 1, y, 1, h, rgb);
    }

    pub fn rounded_panel(&mut self, x: i32, y: i32, w: i32, h: i32, rgb: u32, alpha: u32) {
        let radius = 8.min(w / 4).min(h / 4);
        self.fill_rect_alpha(x + radius, y, w - radius * 2, h, rgb, alpha);
        self.fill_rect_alpha(x, y + radius, w, h - radius * 2, rgb, alpha);
        self.fill_rect_alpha(
            x + radius,
            y + radius,
            w - radius * 2,
            h - radius * 2,
            rgb,
            alpha,
        );
    }

    pub fn panel_border(&mut self, x: i32, y: i32, w: i32, h: i32, rgb: u32) {
        self.stroke_rect(x, y, w, h, rgb);
        self.fill_rect(x + 2, y + 2, w - 4, 1, rgb);
    }

    pub fn draw_text_shadow(&mut self, x: i32, y: i32, text: &str, scale: i32, rgb: u32) {
        self.draw_text(x + 1, y + 1, text, scale, 0x020617);
        self.draw_text(x, y, text, scale, rgb);
    }

    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, scale: i32, rgb: u32) {
        let s = scale.max(1);
        let width = self.width;
        let fb_w = self.width as i32;
        let fb_h = self.height as i32;
        let ui_capture = self.ui_capture;
        let pixels = &mut self.pixels;
        let ui_pixels = &mut self.ui_pixels;
        let ui_font = &mut self.ui_font;

        let ascent = ui_font.ascent_px(s);
        let baseline_y = y + ascent + 1;
        let sr = (rgb >> 16) & 0xFF;
        let sg = (rgb >> 8) & 0xFF;
        let sb = rgb & 0xFF;

        let mut cursor_x = x as f32;
        for ch in text.chars() {
            let g = ui_font.glyph(ch, s);
            let gw = g.metrics.width as i32;
            let gh = g.metrics.height as i32;
            let xmin = g.metrics.xmin;
            let ymin = g.metrics.ymin;
            let advance = g.metrics.advance_width;

            if gw > 0 && gh > 0 && g.bitmap.len() >= (gw * gh) as usize {
                let gx0 = floor_f32(cursor_x + 0.5) as i32 + xmin;
                let gy0 = baseline_y - gh - ymin;

                for row in 0..gh {
                    let py = gy0 + row;
                    if py < 0 || py >= fb_h {
                        continue;
                    }
                    let row_off = (py as usize) * width;
                    let bmp_row = (row * gw) as usize;
                    for col in 0..gw {
                        let px = gx0 + col;
                        if px < 0 || px >= fb_w {
                            continue;
                        }
                        let cov = g.bitmap[bmp_row + col as usize] as u32;
                        if cov == 0 {
                            continue;
                        }
                        let idx = row_off + (px as usize);
                        if cov == 255 {
                            pixels[idx] = rgb;
                            if ui_capture {
                                ui_pixels[idx] = 0xFF00_0000 | (rgb & 0x00FF_FFFF);
                            }
                        } else {
                            let inv = 255 - cov;
                            let dst = pixels[idx];
                            let dr = (((dst >> 16) & 0xFF) * inv + sr * cov) / 255;
                            let dg = (((dst >> 8) & 0xFF) * inv + sg * cov) / 255;
                            let db = ((dst & 0xFF) * inv + sb * cov) / 255;
                            pixels[idx] = (dr << 16) | (dg << 8) | db;
                            if ui_capture {
                                let old = ui_pixels[idx];
                                let old_a = (old >> 24) & 0xFF;
                                let out_a = cov + (old_a * inv) / 255;
                                let blend = |s_c: u32, d_c: u32| {
                                    if out_a == 0 {
                                        0
                                    } else {
                                        (s_c * cov + d_c * old_a * inv / 255) / out_a
                                    }
                                };
                                let r = blend(sr, (old >> 16) & 0xFF);
                                let g_c = blend(sg, (old >> 8) & 0xFF);
                                let b = blend(sb, old & 0xFF);
                                ui_pixels[idx] = (out_a << 24) | (r << 16) | (g_c << 8) | b;
                            }
                        }
                    }
                }
            }
            cursor_x += advance;
        }
    }

    pub fn measure_text(&mut self, text: &str, scale: i32) -> i32 {
        self.ui_font.measure_text(text, scale)
    }
}
