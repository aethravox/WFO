use super::*;

impl MmoClientEngine {
    pub(crate) fn render_entities(&mut self, cam: &Camera) {
        if !self.joined {
            let walk = if self.local_moving {
                sin_approx(self.elapsed * 9.0) * 0.16
            } else {
                0.0
            };
            let px = self.local_x;
            let py = self.local_y;
            let pz = self.local_z;
            let yaw = self.local_yaw;
            let primary = self.lobby_class.primary_rgb();
            let accent = self.lobby_class.accent_rgb();
            self.fb
                .draw_ring_3d(cam, Vec3::new(px, py + 0.06, pz), 0.72, 0.88, accent);
            self.fb.draw_box(
                cam,
                Vec3::new(px - 0.18, py + 0.42, pz + walk),
                Vec3::new(0.12, 0.42, 0.13),
                yaw,
                0x334155,
                false,
            );
            self.fb.draw_box(
                cam,
                Vec3::new(px + 0.18, py + 0.42, pz - walk),
                Vec3::new(0.12, 0.42, 0.13),
                yaw,
                0x334155,
                false,
            );
            self.fb.draw_box(
                cam,
                Vec3::new(px, py + 1.2, pz),
                Vec3::new(0.32, 0.38, 0.22),
                yaw,
                primary,
                false,
            );
            self.fb.draw_box(
                cam,
                Vec3::new(px, py + 1.82, pz),
                Vec3::new(0.2, 0.2, 0.2),
                yaw,
                0xF4C7A1,
                false,
            );
            self.fb.draw_box(
                cam,
                Vec3::new(px + 0.42, py + 1.28, pz + 0.28),
                Vec3::new(0.07, 0.55, 0.07),
                yaw,
                accent,
                true,
            );
            if self.lobby_class == ClassId::Guardian {
                self.fb.draw_box(
                    cam,
                    Vec3::new(px - 0.42, py + 1.2, pz + 0.22),
                    Vec3::new(0.1, 0.62, 0.44),
                    yaw,
                    accent,
                    false,
                );
            }
        }
        // Players (Composite 3D Rigs)
        for (id, p) in &self.players {
            let is_self = self.self_id.as_ref() == Some(id);
            let (px, py, pz, yaw, moving) = if is_self && self.joined {
                (
                    self.local_x,
                    self.local_y,
                    self.local_z,
                    self.local_yaw,
                    self.local_moving,
                )
            } else {
                (p.x, p.y, p.z, p.yaw, p.moving)
            };

            let primary = p.class_id.primary_rgb();
            let accent = p.class_id.accent_rgb();
            let walk = if moving {
                sin_approx(self.elapsed * 10.0) * 0.22
            } else {
                0.0
            };

            // Selection ring on ground
            self.fb
                .draw_ring_3d(cam, Vec3::new(px, py + 0.06, pz), 0.72, 0.88, accent);

            // Legs
            let lx = Vec3::new(-0.18, 0.42, walk)
                .rotate_y(yaw)
                .add(Vec3::new(px, py, pz));
            let rx = Vec3::new(0.18, 0.42, -walk)
                .rotate_y(yaw)
                .add(Vec3::new(px, py, pz));
            self.fb
                .draw_box(cam, lx, Vec3::new(0.12, 0.42, 0.13), yaw, 0x334155, false);
            self.fb
                .draw_box(cam, rx, Vec3::new(0.12, 0.42, 0.13), yaw, 0x334155, false);

            // Torso & Pauldrons & Cape
            self.fb.draw_box(
                cam,
                Vec3::new(px, py + 1.2, pz),
                Vec3::new(0.32, 0.38, 0.22),
                yaw,
                primary,
                false,
            );
            let cape_pos = Vec3::new(0.0, 1.1, -0.26)
                .rotate_y(yaw)
                .add(Vec3::new(px, py, pz));
            self.fb.draw_box(
                cam,
                cape_pos,
                Vec3::new(0.28, 0.44, 0.04),
                yaw,
                0x1E293B,
                false,
            );

            // Head + Class Gear
            self.fb.draw_box(
                cam,
                Vec3::new(px, py + 1.82, pz),
                Vec3::new(0.2, 0.2, 0.2),
                yaw,
                0xE6C3A5,
                false,
            );
            if p.class_id == ClassId::Arcanist {
                self.fb.draw_pyramid(
                    cam,
                    Vec3::new(px, py + 1.98, pz),
                    0.28,
                    0.48,
                    yaw,
                    primary,
                    false,
                );
            } else {
                self.fb.draw_box(
                    cam,
                    Vec3::new(px, py + 1.95, pz),
                    Vec3::new(0.22, 0.1, 0.22),
                    yaw,
                    accent,
                    false,
                );
            }

            // Weapon in Right Hand
            let weap = Vec3::new(0.42, 1.28, 0.28)
                .rotate_y(yaw)
                .add(Vec3::new(px, py, pz));
            self.fb
                .draw_box(cam, weap, Vec3::new(0.07, 0.55, 0.07), yaw, accent, true);
            if p.class_id == ClassId::Guardian {
                let shield_pos = Vec3::new(-0.42, 1.2, 0.22)
                    .rotate_y(yaw)
                    .add(Vec3::new(px, py, pz));
                self.fb.draw_box(
                    cam,
                    shield_pos,
                    Vec3::new(0.1, 0.62, 0.44),
                    yaw,
                    accent,
                    false,
                );
            }
            if p.shield_hp > 0 {
                self.fb
                    .draw_ring_3d(cam, Vec3::new(px, py + 1.08, pz), 0.7, 0.84, 0x67E8F9);
            }
        }

        // Mobs (Composite 3D Rigs)
        for (id, m) in &self.mobs {
            if !m.alive {
                continue;
            }
            if self.target_mob_id.as_ref() == Some(id) {
                let r = if m.kind == MobKind::Colossus {
                    2.3
                } else {
                    1.1
                };
                self.fb
                    .draw_ring_3d(cam, Vec3::new(m.x, m.y + 0.07, m.z), r, r + 0.18, 0xEF4444);
            }
            match m.kind {
                MobKind::Stalker => {
                    self.fb.draw_box(
                        cam,
                        Vec3::new(m.x, m.y + 0.7, m.z),
                        Vec3::new(0.42, 0.28, 0.68),
                        m.yaw,
                        0x3B2154,
                        false,
                    );
                    self.fb.draw_pyramid(
                        cam,
                        Vec3::new(m.x, m.y + 0.98, m.z),
                        0.26,
                        0.45,
                        m.yaw,
                        0xC084FC,
                        true,
                    );
                }
                MobKind::Wisp => {
                    let bob = sin_approx(self.elapsed * 3.2) * 0.22;
                    self.fb.draw_pyramid(
                        cam,
                        Vec3::new(m.x, m.y + 1.2 + bob, m.z),
                        0.45,
                        0.9,
                        self.elapsed * 1.5,
                        0x38BDF8,
                        true,
                    );
                    self.fb.draw_box(
                        cam,
                        Vec3::new(
                            m.x + cos_approx(self.elapsed * 2.0) * 0.75,
                            m.y + 1.45,
                            m.z + sin_approx(self.elapsed * 2.0) * 0.75,
                        ),
                        Vec3::new(0.14, 0.24, 0.14),
                        self.elapsed,
                        0x1E293B,
                        false,
                    );
                }
                MobKind::Sentinel => {
                    self.fb.draw_box(
                        cam,
                        Vec3::new(m.x, m.y + 1.35, m.z),
                        Vec3::new(0.55, 0.75, 0.36),
                        m.yaw,
                        0x1E2430,
                        false,
                    );
                    self.fb.draw_box(
                        cam,
                        Vec3::new(m.x, m.y + 2.3, m.z),
                        Vec3::new(0.28, 0.28, 0.28),
                        m.yaw,
                        0x9A6B32,
                        false,
                    );
                    self.fb.draw_box(
                        cam,
                        Vec3::new(m.x, m.y + 1.5, m.z),
                        Vec3::new(0.18, 0.18, 0.42),
                        m.yaw,
                        0xF97316,
                        true,
                    );
                }
                MobKind::Colossus => {
                    self.fb.draw_box(
                        cam,
                        Vec3::new(m.x, m.y + 2.1, m.z),
                        Vec3::new(1.05, 1.35, 0.72),
                        m.yaw,
                        0x181E29,
                        false,
                    );
                    self.fb.draw_box(
                        cam,
                        Vec3::new(m.x, m.y + 3.8, m.z),
                        Vec3::new(0.45, 0.45, 0.45),
                        m.yaw,
                        0xD97706,
                        false,
                    );
                    self.fb.draw_pyramid(
                        cam,
                        Vec3::new(m.x, m.y + 4.35, m.z),
                        0.55,
                        0.85,
                        self.elapsed,
                        0xFBBF24,
                        true,
                    );
                    self.fb.draw_box(
                        cam,
                        Vec3::new(m.x, m.y + 2.3, m.z),
                        Vec3::new(0.38, 0.38, 0.78),
                        m.yaw,
                        0xF59E0B,
                        true,
                    );
                }
            }
        }
    }

    pub(crate) fn render_vfx(&mut self, cam: &Camera) {
        for r in &self.rings {
            let t = r.age / r.life;
            let rad = 0.5 + t * r.max_r;
            self.fb
                .draw_ring_3d(cam, Vec3::new(r.x, r.y, r.z), rad, rad + 0.25, r.color);
        }
    }
}
