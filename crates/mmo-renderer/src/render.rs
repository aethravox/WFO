use super::*;

impl MmoClientEngine {
    pub(crate) fn render_lobby_room(&mut self, cam: &Camera) {
        // A roughly 500 m² anime-style private lodge: 25 m × 20 m.
        let floor = [
            Vec3::new(-12.5, 0.0, -10.0),
            Vec3::new(12.5, 0.0, -10.0),
            Vec3::new(12.5, 0.0, 10.0),
            Vec3::new(-12.5, 0.0, 10.0),
        ];
        self.fb.draw_quad(cam, floor[0], floor[1], floor[2], floor[3], 0x4C596B, false);

        for x in (-12..=12).step_by(3) {
            let px = x as f32;
            self.fb.draw_box(cam, Vec3::new(px, 0.08, 0.0), Vec3::new(0.025, 0.025, 9.8), 0.0, 0x718096, false);
        }
        for z in (-9..=9).step_by(3) {
            let pz = z as f32;
            self.fb.draw_box(cam, Vec3::new(0.0, 0.08, pz), Vec3::new(12.2, 0.025, 0.025), 0.0, 0x718096, false);
        }

        // Back and side walls.
        self.fb.draw_box(cam, Vec3::new(0.0, 3.2, -10.0), Vec3::new(12.7, 3.2, 0.25), 0.0, 0x273449, false);
        self.fb.draw_box(cam, Vec3::new(-12.5, 3.2, 0.0), Vec3::new(0.25, 3.2, 10.1), 0.0, 0x273449, false);
        self.fb.draw_box(cam, Vec3::new(12.5, 3.2, 0.0), Vec3::new(0.25, 3.2, 10.1), 0.0, 0x273449, false);

        // World portal at the back.
        self.fb.draw_box(cam, Vec3::new(-3.8, 3.0, -9.45), Vec3::new(0.45, 3.0, 0.45), 0.0, 0xE8B4D9, false);
        self.fb.draw_box(cam, Vec3::new(3.8, 3.0, -9.45), Vec3::new(0.45, 3.0, 0.45), 0.0, 0xE8B4D9, false);
        self.fb.draw_box(cam, Vec3::new(0.0, 5.9, -9.45), Vec3::new(4.2, 0.45, 0.45), 0.0, 0xE8B4D9, false);
        self.fb.draw_ring_3d(cam, Vec3::new(0.0, 0.16, -9.35), 2.7, 2.95, 0x38BDF8);
        self.fb.draw_pyramid(cam, Vec3::new(0.0, 3.2, -9.3), 2.1, 4.8, self.elapsed * 0.35, 0x8B5CF6, true);

        // Random-class NPC on the left.
        self.fb.draw_box(cam, Vec3::new(-9.0, 1.15, 4.0), Vec3::new(0.5, 1.15, 0.5), 0.0, 0xC27824, false);
        self.fb.draw_box(cam, Vec3::new(-9.0, 2.25, 4.0), Vec3::new(0.35, 0.35, 0.35), 0.0, 0xF4C7A1, false);
        self.fb.draw_pyramid(cam, Vec3::new(-9.0, 2.55, 4.0), 0.5, 0.7, self.elapsed, 0xF59E0B, true);

        // Storage, chess, and duel arena.
        self.fb.draw_box(cam, Vec3::new(9.0, 1.1, 4.0), Vec3::new(1.7, 1.1, 0.55), 0.0, 0x6556A0, false);
        self.fb.draw_box(cam, Vec3::new(-7.0, 0.65, -5.0), Vec3::new(2.0, 0.65, 1.2), 0.0, 0x263348, false);
        for i in 0..8 {
            let px = -7.0 - 1.75 + (i as f32 % 2.0) * 3.5;
            let pz = -5.0 - 1.05 + (i / 2) as f32 * 0.7;
            self.fb.draw_box(cam, Vec3::new(px, 1.34, pz), Vec3::new(0.28, 0.28, 0.28), 0.0, if i % 2 == 0 { 0xF8FAFC } else { 0x111827 }, false);
        }
        self.fb.draw_ring_3d(cam, Vec3::new(8.0, 0.13, -5.0), 3.2, 3.5, 0xEF4444);
        self.fb.draw_box(cam, Vec3::new(8.0, 1.0, -5.0), Vec3::new(0.35, 1.0, 0.35), 0.0, 0xF97316, true);

        // Friendly display pedestals.
        for i in 0..4 {
            let px = -10.5 + i as f32 * 2.2;
            self.fb.draw_box(cam, Vec3::new(px, 0.55, 8.5), Vec3::new(0.55, 0.55, 0.55), 0.0, 0x334155, false);
        }
    }
    pub fn render(&mut self) {
        self.fb.clear_sky();
        let cam = self.current_camera();

        if self.joined {
            self.render_terrain(&cam);
            self.render_landmarks(&cam);
            self.render_forest_and_crystals(&cam);
        } else {
            self.render_lobby_room(&cam);
        }
        self.render_entities(&cam);
        self.render_vfx(&cam);
        self.render_hud(&cam);
    }

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
            self.fb.draw_box(cam, Vec3::new(px, 2.0, pz), Vec3::new(0.45, 2.0, 0.45), ang, 0x526072, false);
            self.fb.draw_box(cam, Vec3::new(px, 4.15, pz), Vec3::new(0.6, 0.2, 0.6), ang, 0xD97706, false);
            let float_y = 4.9 + sin_approx(self.elapsed * 2.3 + i as f32) * 0.16;
            self.fb.draw_pyramid(cam, Vec3::new(px, float_y, pz), 0.3, 0.65, self.elapsed + i as f32, 0x38BDF8, true);
        }

        // Grand North Waystone Portal Arch (0, 0, -14.5)
        self.fb.draw_box(cam, Vec3::new(-3.2, 3.3, -14.5), Vec3::new(0.55, 3.3, 0.55), 0.0, 0x475569, false);
        self.fb.draw_box(cam, Vec3::new(3.2, 3.3, -14.5), Vec3::new(0.55, 3.3, 0.55), 0.0, 0x475569, false);
        self.fb.draw_box(cam, Vec3::new(0.0, 6.9, -14.5), Vec3::new(4.1, 0.55, 0.7), 0.0, 0x475569, false);
        self.fb.draw_ring_3d(cam, Vec3::new(0.0, 0.12, 0.0), 14.8, 15.2, 0x38BDF8);

        // Northern Obsidian Citadel Monoliths (0, -58)
        let cy = ground_height_at(0.0, -58.0);
        self.fb.draw_box(cam, Vec3::new(0.0, cy + 0.3, -58.0), Vec3::new(6.8, 0.35, 6.8), 0.39, 0x1E2530, false);
        let spires = [(7.5, 7.5), (-7.5, 7.5), (7.5, -7.5), (-7.5, -7.5)];
        for (idx, (sx, sz)) in spires.iter().enumerate() {
            let py = ground_height_at(*sx, -58.0 + *sz);
            self.fb.draw_pyramid(cam, Vec3::new(*sx, py, -58.0 + *sz), 1.25, 8.5, 0.4, 0x1A202C, false);
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

    pub(crate) fn render_forest_and_crystals(&mut self, cam: &Camera) {
        // Deterministic Pine Trees
        for i in 0..56 {
            let ang = (i as f32) * 2.39996;
            let rad = 22.0 + ((i * 73) % 58) as f32;
            let tx = cos_approx(ang) * rad;
            let tz = sin_approx(ang) * rad;
            if sqrt_f32(tx * tx + (tz + 58.0) * (tz + 58.0)) < 14.0 {
                continue;
            }
            if abs_f32(tx) < 5.5 && tz < -10.0 && tz > -55.0 {
                continue;
            }
            let ty = ground_height_at(tx, tz);
            self.fb.draw_box(cam, Vec3::new(tx, ty + 1.0, tz), Vec3::new(0.24, 1.0, 0.24), 0.0, 0x5C4028, false);
            self.fb.draw_pyramid(cam, Vec3::new(tx, ty + 1.7, tz), 1.55, 3.0, ang, 0x1E5631, false);
            self.fb.draw_pyramid(cam, Vec3::new(tx, ty + 3.4, tz), 1.1, 2.4, ang + 0.5, 0x266B3E, false);
        }

        // Harvestable Aether Crystals
        for c in self.crystals.values() {
            self.fb.draw_box(cam, Vec3::new(c.x, c.y + 0.2, c.z), Vec3::new(0.75, 0.2, 0.75), 0.3, 0x334155, false);
            if c.available {
                self.fb.draw_pyramid(cam, Vec3::new(c.x, c.y + 0.35, c.z), 0.36, 1.5, self.elapsed * 0.4, 0x22D3EE, true);
                self.fb.draw_pyramid(cam, Vec3::new(c.x + 0.35, c.y + 0.3, c.z + 0.2), 0.22, 0.95, 0.3, 0x06B6D4, true);
                self.fb.draw_pyramid(cam, Vec3::new(c.x - 0.32, c.y + 0.3, c.z - 0.22), 0.22, 0.9, -0.4, 0x06B6D4, true);
            }
        }
    }

    pub(crate) fn render_entities(&mut self, cam: &Camera) {
        if !self.joined {
            let walk = if self.local_moving { sin_approx(self.elapsed * 9.0) * 0.16 } else { 0.0 };
            let px = self.local_x;
            let py = self.local_y;
            let pz = self.local_z;
            let yaw = self.local_yaw;
            let primary = self.lobby_class.primary_rgb();
            let accent = self.lobby_class.accent_rgb();
            self.fb.draw_ring_3d(cam, Vec3::new(px, py + 0.06, pz), 0.72, 0.88, accent);
            self.fb.draw_box(cam, Vec3::new(px - 0.18, py + 0.42, pz + walk), Vec3::new(0.12, 0.42, 0.13), yaw, 0x334155, false);
            self.fb.draw_box(cam, Vec3::new(px + 0.18, py + 0.42, pz - walk), Vec3::new(0.12, 0.42, 0.13), yaw, 0x334155, false);
            self.fb.draw_box(cam, Vec3::new(px, py + 1.2, pz), Vec3::new(0.32, 0.38, 0.22), yaw, primary, false);
            self.fb.draw_box(cam, Vec3::new(px, py + 1.82, pz), Vec3::new(0.2, 0.2, 0.2), yaw, 0xF4C7A1, false);
            self.fb.draw_box(cam, Vec3::new(px + 0.42, py + 1.28, pz + 0.28), Vec3::new(0.07, 0.55, 0.07), yaw, accent, true);
        }
        // Players (Composite 3D Rigs)
        for (id, p) in &self.players {
            let is_self = self.self_id.as_ref() == Some(id);
            let (px, py, pz, yaw, moving) = if is_self && self.joined {
                (self.local_x, self.local_y, self.local_z, self.local_yaw, self.local_moving)
            } else {
                (p.x, p.y, p.z, p.yaw, p.moving)
            };

            let primary = p.class_id.primary_rgb();
            let accent = p.class_id.accent_rgb();
            let walk = if moving { sin_approx(self.elapsed * 10.0) * 0.22 } else { 0.0 };

            // Selection ring on ground
            self.fb.draw_ring_3d(cam, Vec3::new(px, py + 0.06, pz), 0.72, 0.88, accent);

            // Legs
            let lx = Vec3::new(-0.18, 0.42, walk).rotate_y(yaw).add(Vec3::new(px, py, pz));
            let rx = Vec3::new(0.18, 0.42, -walk).rotate_y(yaw).add(Vec3::new(px, py, pz));
            self.fb.draw_box(cam, lx, Vec3::new(0.12, 0.42, 0.13), yaw, 0x334155, false);
            self.fb.draw_box(cam, rx, Vec3::new(0.12, 0.42, 0.13), yaw, 0x334155, false);

            // Torso & Pauldrons & Cape
            self.fb.draw_box(cam, Vec3::new(px, py + 1.2, pz), Vec3::new(0.32, 0.38, 0.22), yaw, primary, false);
            let cape_pos = Vec3::new(0.0, 1.1, -0.26).rotate_y(yaw).add(Vec3::new(px, py, pz));
            self.fb.draw_box(cam, cape_pos, Vec3::new(0.28, 0.44, 0.04), yaw, 0x1E293B, false);

            // Head + Class Gear
            self.fb.draw_box(cam, Vec3::new(px, py + 1.82, pz), Vec3::new(0.2, 0.2, 0.2), yaw, 0xE6C3A5, false);
            if p.class_id == ClassId::Arcanist {
                self.fb.draw_pyramid(cam, Vec3::new(px, py + 1.98, pz), 0.28, 0.48, yaw, primary, false);
            } else {
                self.fb.draw_box(cam, Vec3::new(px, py + 1.95, pz), Vec3::new(0.22, 0.1, 0.22), yaw, accent, false);
            }

            // Weapon in Right Hand
            let weap = Vec3::new(0.42, 1.28, 0.28).rotate_y(yaw).add(Vec3::new(px, py, pz));
            self.fb.draw_box(cam, weap, Vec3::new(0.07, 0.55, 0.07), yaw, accent, true);
        }

        // Mobs (Composite 3D Rigs)
        for (id, m) in &self.mobs {
            if !m.alive {
                continue;
            }
            if self.target_mob_id.as_ref() == Some(id) {
                let r = if m.kind == MobKind::Colossus { 2.3 } else { 1.1 };
                self.fb.draw_ring_3d(cam, Vec3::new(m.x, m.y + 0.07, m.z), r, r + 0.18, 0xEF4444);
            }
            match m.kind {
                MobKind::Stalker => {
                    self.fb.draw_box(cam, Vec3::new(m.x, m.y + 0.7, m.z), Vec3::new(0.42, 0.28, 0.68), m.yaw, 0x3B2154, false);
                    self.fb.draw_pyramid(cam, Vec3::new(m.x, m.y + 0.98, m.z), 0.26, 0.45, m.yaw, 0xC084FC, true);
                }
                MobKind::Wisp => {
                    let bob = sin_approx(self.elapsed * 3.2) * 0.22;
                    self.fb.draw_pyramid(cam, Vec3::new(m.x, m.y + 1.2 + bob, m.z), 0.45, 0.9, self.elapsed * 1.5, 0x38BDF8, true);
                    self.fb.draw_box(cam, Vec3::new(m.x + cos_approx(self.elapsed * 2.0) * 0.75, m.y + 1.45, m.z + sin_approx(self.elapsed * 2.0) * 0.75), Vec3::new(0.14, 0.24, 0.14), self.elapsed, 0x1E293B, false);
                }
                MobKind::Sentinel => {
                    self.fb.draw_box(cam, Vec3::new(m.x, m.y + 1.35, m.z), Vec3::new(0.55, 0.75, 0.36), m.yaw, 0x1E2430, false);
                    self.fb.draw_box(cam, Vec3::new(m.x, m.y + 2.3, m.z), Vec3::new(0.28, 0.28, 0.28), m.yaw, 0x9A6B32, false);
                    self.fb.draw_box(cam, Vec3::new(m.x, m.y + 1.5, m.z), Vec3::new(0.18, 0.18, 0.42), m.yaw, 0xF97316, true);
                }
                MobKind::Colossus => {
                    self.fb.draw_box(cam, Vec3::new(m.x, m.y + 2.1, m.z), Vec3::new(1.05, 1.35, 0.72), m.yaw, 0x181E29, false);
                    self.fb.draw_box(cam, Vec3::new(m.x, m.y + 3.8, m.z), Vec3::new(0.45, 0.45, 0.45), m.yaw, 0xD97706, false);
                    self.fb.draw_pyramid(cam, Vec3::new(m.x, m.y + 4.35, m.z), 0.55, 0.85, self.elapsed, 0xFBBF24, true);
                    self.fb.draw_box(cam, Vec3::new(m.x, m.y + 2.3, m.z), Vec3::new(0.38, 0.38, 0.78), m.yaw, 0xF59E0B, true);
                }
            }
        }
    }

    pub(crate) fn render_vfx(&mut self, cam: &Camera) {
        for r in &self.rings {
            let t = r.age / r.life;
            let rad = 0.5 + t * r.max_r;
            self.fb.draw_ring_3d(cam, Vec3::new(r.x, r.y, r.z), rad, rad + 0.25, r.color);
        }
    }
}
