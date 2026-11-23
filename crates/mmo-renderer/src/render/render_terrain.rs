use super::*;

impl MmoClientEngine {
    pub(crate) fn render_terrain(&mut self, cam: &Camera) {
        // Render sculpted terrain grid centered around camera focus
        let focus_x = if self.joined { self.local_x } else { 0.0 };
        let focus_z = if self.joined { self.local_z } else { -8.0 };
        let step = 7.5f32;
        let gx0 = floor_f32((focus_x - 75.0) / step);
        let gx1 = ceil_f32((focus_x + 75.0) / step);
        let gz0 = floor_f32((focus_z - 75.0) / step);
        let gz1 = ceil_f32((focus_z + 75.0) / step);

        for gz in gz0..gz1 {
            for gx in gx0..gx1 {
                let x0 = (gx as f32) * step;
                let z0 = (gz as f32) * step;
                let x1 = x0 + step;
                let z1 = z0 + step;

                let p00 = Vec3::new(x0, ground_height_at(x0, z0), z0);
                let p10 = Vec3::new(x1, ground_height_at(x1, z0), z0);
                let p11 = Vec3::new(x1, ground_height_at(x1, z1), z1);
                let p01 = Vec3::new(x0, ground_height_at(x0, z1), z1);

                let cx = (x0 + x1) * 0.5;
                let cz = (z0 + z1) * 0.5;
                let dist = sqrt_f32(cx * cx + cz * cz);
                let rgb = if dist < 16.0 {
                    0x536275 // Sanctuary flagstones
                } else if dist > 82.0 {
                    0x3E4C5E // Outer mountain cliff
                } else if cz < -32.0 && abs_f32(cx) < 32.0 {
                    0x2A3240 // Obsidian Citadel basalt
                } else if cx > 22.0 {
                    0x2F5254 // Crystal Veins slate
                } else if ((gx + gz) & 1) == 0 {
                    0x2F6B3E // Emerald meadow A
                } else {
                    0x285E36 // Emerald meadow B
                };

                self.fb.draw_quad(cam, p00, p10, p11, p01, rgb, false);
            }
        }
    }

    pub(crate) fn render_landmarks(&mut self, cam: &Camera) {
        // 6 Sanctuary Rune Pillars + Floating Cyan Crystals
        for i in 0..6 {
            let ang = (i as f32 / 6.0) * 6.283185 + 0.5236;
            let px = cos_approx(ang) * 14.2;
            let pz = sin_approx(ang) * 14.2;
            self.fb.draw_box(
                cam,
                Vec3::new(px, 2.0, pz),
                Vec3::new(0.45, 2.0, 0.45),
                ang,
                0x526072,
                false,
            );
            self.fb.draw_box(
                cam,
                Vec3::new(px, 4.15, pz),
                Vec3::new(0.6, 0.2, 0.6),
                ang,
                0xD97706,
                false,
            );
            let float_y = 4.9 + sin_approx(self.elapsed * 2.3 + i as f32) * 0.16;
            self.fb.draw_pyramid(
                cam,
                Vec3::new(px, float_y, pz),
                0.3,
                0.65,
                self.elapsed + i as f32,
                0x38BDF8,
                true,
            );
        }

        // Grand North Waystone Portal Arch (0, 0, -14.5)
        self.fb.draw_box(
            cam,
            Vec3::new(-3.2, 3.3, -14.5),
            Vec3::new(0.55, 3.3, 0.55),
            0.0,
            0x475569,
            false,
        );
        self.fb.draw_box(
            cam,
            Vec3::new(3.2, 3.3, -14.5),
            Vec3::new(0.55, 3.3, 0.55),
            0.0,
            0x475569,
            false,
        );
        self.fb.draw_box(
            cam,
            Vec3::new(0.0, 6.9, -14.5),
            Vec3::new(4.1, 0.55, 0.7),
            0.0,
            0x475569,
            false,
        );
        self.fb
            .draw_ring_3d(cam, Vec3::new(0.0, 0.12, 0.0), 14.8, 15.2, 0x38BDF8);

        // Northern Obsidian Citadel Monoliths (0, -58)
        let cy = ground_height_at(0.0, -58.0);
        self.fb.draw_box(
            cam,
            Vec3::new(0.0, cy + 0.3, -58.0),
            Vec3::new(6.8, 0.35, 6.8),
            0.39,
            0x1E2530,
            false,
        );
        let spires = [(7.5, 7.5), (-7.5, 7.5), (7.5, -7.5), (-7.5, -7.5)];
        for (idx, (sx, sz)) in spires.iter().enumerate() {
            let py = ground_height_at(*sx, -58.0 + *sz);
            self.fb.draw_pyramid(
                cam,
                Vec3::new(*sx, py, -58.0 + *sz),
                1.25,
                8.5,
                0.4,
                0x1A202C,
                false,
            );
            self.fb.draw_pyramid(
                cam,
                Vec3::new(*sx * 0.72, py + 1.1, -58.0 + *sz * 0.72),
                0.4,
                0.8,
                self.elapsed + idx as f32,
                0xF59E0B,
                true,
            );
        }
    }
}
