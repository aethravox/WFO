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
        self.fb
            .draw_quad(cam, floor[0], floor[1], floor[2], floor[3], 0x4C596B, false);

        for x in (-12..=12).step_by(3) {
            let px = x as f32;
            self.fb.draw_box(
                cam,
                Vec3::new(px, 0.08, 0.0),
                Vec3::new(0.025, 0.025, 9.8),
                0.0,
                0x718096,
                false,
            );
        }
        for z in (-9..=9).step_by(3) {
            let pz = z as f32;
            self.fb.draw_box(
                cam,
                Vec3::new(0.0, 0.08, pz),
                Vec3::new(12.2, 0.025, 0.025),
                0.0,
                0x718096,
                false,
            );
        }

        // Back and side walls.
        self.fb.draw_box(
            cam,
            Vec3::new(0.0, 3.2, -10.0),
            Vec3::new(12.7, 3.2, 0.25),
            0.0,
            0x273449,
            false,
        );
        self.fb.draw_box(
            cam,
            Vec3::new(-12.5, 3.2, 0.0),
            Vec3::new(0.25, 3.2, 10.1),
            0.0,
            0x273449,
            false,
        );
        self.fb.draw_box(
            cam,
            Vec3::new(12.5, 3.2, 0.0),
            Vec3::new(0.25, 3.2, 10.1),
            0.0,
            0x273449,
            false,
        );

        // World portal at the back.
        self.fb.draw_box(
            cam,
            Vec3::new(-3.8, 3.0, -9.45),
            Vec3::new(0.45, 3.0, 0.45),
            0.0,
            0xE8B4D9,
            false,
        );
        self.fb.draw_box(
            cam,
            Vec3::new(3.8, 3.0, -9.45),
            Vec3::new(0.45, 3.0, 0.45),
            0.0,
            0xE8B4D9,
            false,
        );
        self.fb.draw_box(
            cam,
            Vec3::new(0.0, 5.9, -9.45),
            Vec3::new(4.2, 0.45, 0.45),
            0.0,
            0xE8B4D9,
            false,
        );
        self.fb
            .draw_ring_3d(cam, Vec3::new(0.0, 0.16, -9.35), 2.7, 2.95, 0x38BDF8);
        self.fb.draw_pyramid(
            cam,
            Vec3::new(0.0, 3.2, -9.3),
            2.1,
            4.8,
            self.elapsed * 0.35,
            0x8B5CF6,
            true,
        );

        // Random-class NPC on the left.
        self.fb.draw_box(
            cam,
            Vec3::new(-9.0, 1.15, 4.0),
            Vec3::new(0.5, 1.15, 0.5),
            0.0,
            0xC27824,
            false,
        );
        self.fb.draw_box(
            cam,
            Vec3::new(-9.0, 2.25, 4.0),
            Vec3::new(0.35, 0.35, 0.35),
            0.0,
            0xF4C7A1,
            false,
        );
        self.fb.draw_pyramid(
            cam,
            Vec3::new(-9.0, 2.55, 4.0),
            0.5,
            0.7,
            self.elapsed,
            0xF59E0B,
            true,
        );

        // Storage, chess, and duel arena.
        self.fb.draw_box(
            cam,
            Vec3::new(9.0, 1.1, 4.0),
            Vec3::new(1.7, 1.1, 0.55),
            0.0,
            0x6556A0,
            false,
        );
        self.fb.draw_box(
            cam,
            Vec3::new(-7.0, 0.65, -5.0),
            Vec3::new(2.0, 0.65, 1.2),
            0.0,
            0x263348,
            false,
        );
        for i in 0..8 {
            let px = -7.0 - 1.75 + (i as f32 % 2.0) * 3.5;
            let pz = -5.0 - 1.05 + (i / 2) as f32 * 0.7;
            self.fb.draw_box(
                cam,
                Vec3::new(px, 1.34, pz),
                Vec3::new(0.28, 0.28, 0.28),
                0.0,
                if i % 2 == 0 { 0xF8FAFC } else { 0x111827 },
                false,
            );
        }
        self.fb
            .draw_ring_3d(cam, Vec3::new(8.0, 0.13, -5.0), 3.2, 3.5, 0xEF4444);
        self.fb.draw_box(
            cam,
            Vec3::new(8.0, 1.0, -5.0),
            Vec3::new(0.35, 1.0, 0.35),
            0.0,
            0xF97316,
            true,
        );

        // Friendly display pedestals.
        for i in 0..4 {
            let px = -10.5 + i as f32 * 2.2;
            self.fb.draw_box(
                cam,
                Vec3::new(px, 0.55, 8.5),
                Vec3::new(0.55, 0.55, 0.55),
                0.0,
                0x334155,
                false,
            );
        }
    }
}
