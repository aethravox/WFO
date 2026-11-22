use super::*;

impl MmoClientEngine {
    pub(crate) fn update_joystick(&mut self, x: i32, y: i32) {
        let h = self.fb.height as i32;
        let dx = (x - 68) as f32;
        let dy = (y - (h - 208)) as f32;
        let max_r = 42.0f32;
        let dist = sqrt_f32(dx * dx + dy * dy);
        if dist > max_r {
            self.input.joy_x = dx / dist;
            self.input.joy_y = dy / dist;
        } else if dist > 2.0 {
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
            let ang = self.elapsed * 0.16;
            let cam_pos = Vec3::new(
                sin_approx(ang) * 24.0,
                10.5 + sin_approx(self.elapsed * 0.35) * 1.1,
                cos_approx(ang) * 24.0,
            );
            Camera::look_at(cam_pos, Vec3::new(0.0, 2.2, -14.0), self.fb.width)
        }
    }
}
