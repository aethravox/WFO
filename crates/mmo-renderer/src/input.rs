use super::*;

impl MmoClientEngine {
    pub(crate) fn update_joystick(&mut self, x: i32, y: i32) {
        let h = self.fb.height as i32;
        let ox = if self.input.joy_origin_x > 0 {
            self.input.joy_origin_x
        } else {
            74
        };
        let oy = if self.input.joy_origin_y > 0 {
            self.input.joy_origin_y
        } else {
            (h - 180).max(120)
        };
        let dx = (x - ox) as f32;
        let dy = (y - oy) as f32;
        let max_r = 44.0f32;
        let dist = sqrt_f32(dx * dx + dy * dy);
        if dist > max_r {
            self.input.joy_x = dx / dist;
            self.input.joy_y = dy / dist;
        } else if dist > 4.0 {
            self.input.joy_x = dx / max_r;
            self.input.joy_y = dy / max_r;
        } else {
            self.input.joy_x = 0.0;
            self.input.joy_y = 0.0;
        }
    }

    pub(crate) fn current_camera(&self) -> Camera {
        if self.joined {
            let target = Vec3::new(self.local_x, self.local_y + 1.5, self.local_z);
            let horiz = cos_approx(self.cam_pitch) * self.cam_dist;
            let vert = sin_approx(self.cam_pitch) * self.cam_dist;
            let cam_pos = Vec3::new(
                self.local_x + sin_approx(self.cam_yaw) * horiz,
                self.local_y + 1.5 + vert,
                self.local_z + cos_approx(self.cam_yaw) * horiz,
            );
            Camera::look_at(cam_pos, target, self.fb.width)
        } else {
            let horiz = cos_approx(self.cam_pitch) * self.cam_dist;
            let vert = sin_approx(self.cam_pitch) * self.cam_dist;
            let mut cx = self.local_x + sin_approx(self.cam_yaw) * horiz;
            let mut cz = self.local_z + cos_approx(self.cam_yaw) * horiz;
            cx = cx.clamp(-11.8, 11.8);
            cz = cz.clamp(-9.3, 9.3);
            let focus_y = self.local_y + 1.5;
            let ideal_y = focus_y + vert;
            let min_cam_y = 0.55;
            let (cam_y, target_y) = if ideal_y < min_cam_y {
                (min_cam_y, min_cam_y - vert)
            } else {
                (ideal_y, focus_y)
            };
            let cam_pos = Vec3::new(cx, cam_y, cz);
            let target = Vec3::new(self.local_x, target_y, self.local_z);
            Camera::look_at(cam_pos, target, self.fb.width)
        }
    }
}
