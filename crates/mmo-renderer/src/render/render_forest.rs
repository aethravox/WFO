use super::*;

impl MmoClientEngine {
    pub(crate) fn render_forest_and_crystals(&mut self, cam: &Camera) {
        // Deterministic Pine Trees
        for i in 0..56 {
            let ang = (i as f32) * 2.39996;
            let Some((tx, tz)) = mmo_protocol::forest_tree_position(i) else {
                continue;
            };
            let ty = ground_height_at(tx, tz);
            self.fb.draw_box(
                cam,
                Vec3::new(tx, ty + 1.0, tz),
                Vec3::new(0.24, 1.0, 0.24),
                0.0,
                0x5C4028,
                false,
            );
            self.fb.draw_pyramid(
                cam,
                Vec3::new(tx, ty + 1.7, tz),
                1.55,
                3.0,
                ang,
                0x1E5631,
                false,
            );
            self.fb.draw_pyramid(
                cam,
                Vec3::new(tx, ty + 3.4, tz),
                1.1,
                2.4,
                ang + 0.5,
                0x266B3E,
                false,
            );
        }

        // Harvestable Aether Crystals
        for c in self.crystals.values() {
            self.fb.draw_box(
                cam,
                Vec3::new(c.x, c.y + 0.2, c.z),
                Vec3::new(0.75, 0.2, 0.75),
                0.3,
                0x334155,
                false,
            );
            if c.available {
                self.fb.draw_pyramid(
                    cam,
                    Vec3::new(c.x, c.y + 0.35, c.z),
                    0.36,
                    1.5,
                    self.elapsed * 0.4,
                    0x22D3EE,
                    true,
                );
                self.fb.draw_pyramid(
                    cam,
                    Vec3::new(c.x + 0.35, c.y + 0.3, c.z + 0.2),
                    0.22,
                    0.95,
                    0.3,
                    0x06B6D4,
                    true,
                );
                self.fb.draw_pyramid(
                    cam,
                    Vec3::new(c.x - 0.32, c.y + 0.3, c.z - 0.22),
                    0.22,
                    0.9,
                    -0.4,
                    0x06B6D4,
                    true,
                );
            }
        }
    }
}
