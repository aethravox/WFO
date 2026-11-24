use super::*;

impl MmoClientEngine {
    pub fn on_touch_start(&mut self, id: u64, x: i32, y: i32) {
        self.touch_mode = true;
        let w = self.fb.width as i32;
        let h = self.fb.height as i32;

        if !self.joined {
            let panel_active = self.auth_panel_open && !self.account_authenticated;
            if !panel_active {
                if self.input.joy_pointer.is_none() && x <= w / 2 && y >= 40 && y <= h - 40 {
                    self.input.joy_pointer = Some(id);
                    self.input.joy_origin_x = x.clamp(58, (w / 2 - 24).max(58));
                    self.input.joy_origin_y = y.clamp(80, (h - 80).max(80));
                    self.update_joystick(x, y);
                    return;
                }
                if x >= w - 92 && x <= w - 16 && y >= h - 210 && y <= h - 164 {
                    self.trigger_jump();
                    return;
                }
                if x >= w - 174 && x <= w - 102 && y >= h - 156 && y <= h - 110 {
                    self.trigger_dodge();
                    return;
                }
                if x >= w - 92 && x <= w - 16 && y >= h - 156 && y <= h - 110 {
                    self.trigger_gather();
                    return;
                }
            } else {
                // If auth panel is open, let touches on left side interact or not capture camera drag inadvertently
                if x <= w / 2 {
                    return;
                }
            }
        } else {
            let compact_actions = w < 680;
            let bar_x = (w - if compact_actions { 208 } else { 280 }) / 2;
            let bar_y = h - 68;
            let bar_w = if compact_actions { 208 } else { 280 };

            // Utility buttons
            let jbx = w - 88;
            if x >= jbx && x <= jbx + 72 && y >= h - 296 && y <= h - 250 {
                self.toggle_personal_marker();
                return;
            }
            if x >= jbx && x <= jbx + 72 && y >= h - 242 && y <= h - 196 {
                self.toggle_bestiary();
                return;
            }
            if x >= jbx && x <= jbx + 72 && y >= h - 350 && y <= h - 304 {
                self.touch_emote_menu = !self.touch_emote_menu;
                return;
            }
            if self.touch_emote_menu {
                let emote_w = 58;
                let emote_x = w - 88 - 4 * emote_w - 12;
                let emote_y = h - 350;
                if x >= emote_x && x <= emote_x + 4 * emote_w && y >= emote_y && y <= emote_y + 46 {
                    let slot = ((x - emote_x) / emote_w).clamp(0, 3) as usize;
                    let emotes = [
                        EmoteId::Wave,
                        EmoteId::Cheer,
                        EmoteId::Laugh,
                        EmoteId::Dance,
                    ];
                    self.trigger_emote(emotes[slot]);
                    self.touch_emote_menu = false;
                    return;
                }
            }

            // Virtual joystick
            let joy_zone_max_x = (bar_x - 10).max(120);
            let joy_zone_min_y = (h / 2 - 36).max(114);
            let joy_zone_max_y = (h - 74).max(joy_zone_min_y + 40);
            if self.input.joy_pointer.is_none()
                && x <= joy_zone_max_x
                && y >= joy_zone_min_y
                && y <= joy_zone_max_y
            {
                let min_oy = joy_zone_min_y + 24;
                let max_oy = (joy_zone_max_y - 30).max(min_oy);
                self.input.joy_pointer = Some(id);
                self.input.joy_origin_x = x.clamp(58, (joy_zone_max_x - 24).max(58));
                self.input.joy_origin_y = y.clamp(min_oy, max_oy);
                self.update_joystick(x, y);
                return;
            }

            // Compact chat button
            if x >= w - 88 && x <= w - 16 && y >= h - 188 && y <= h - 142 {
                self.chat_focused = !self.chat_focused;
                return;
            }

            // Jump button
            if x >= w - 88 && x <= w - 16 && y >= h - 134 && y <= h - 88 {
                self.trigger_jump();
                return;
            }

            // Dodge button
            if x >= w - 160 && x <= w - 96 && y >= h - 134 && y <= h - 88 {
                self.trigger_dodge();
                return;
            }

            // Action bar
            if y >= bar_y && y <= bar_y + 52 && x >= bar_x && x <= bar_x + bar_w {
                let slot = if compact_actions {
                    (x - bar_x - 6) / 50
                } else {
                    (x - bar_x) / 70
                };
                match slot {
                    0 => self.trigger_skill(0),
                    1 => self.trigger_skill(1),
                    2 => self.trigger_skill(2),
                    _ => self.trigger_gather(),
                }
                return;
            }
        }

        if self.input.cam_pointer.is_none() {
            self.input.cam_pointer = Some(id);
            self.input.cam_last_x = x;
            self.input.cam_last_y = y;
            self.input.cam_drag_dist = 0;
        } else if self.input.pinch_pointer.is_none() {
            self.input.pinch_pointer = Some(id);
            self.input.pinch_last_x = x;
            self.input.pinch_last_y = y;
            let dx = x - self.input.cam_last_x;
            let dy = y - self.input.cam_last_y;
            self.input.pinch_last_distance = sqrt_f32((dx * dx + dy * dy) as f32);
        }
    }

    pub fn on_touch_move(&mut self, id: u64, x: i32, y: i32) {
        if self.input.joy_pointer == Some(id) {
            self.update_joystick(x, y);
            return;
        }
        if self.input.pinch_pointer == Some(id) {
            self.input.pinch_last_x = x;
            self.input.pinch_last_y = y;
            let dx = x - self.input.cam_last_x;
            let dy = y - self.input.cam_last_y;
            let distance = sqrt_f32((dx * dx + dy * dy) as f32);
            let delta = distance - self.input.pinch_last_distance;
            if delta.abs() > 1.0 {
                self.cam_dist = clamp_f32(self.cam_dist - delta * 0.018, 6.5, 22.0);
                self.input.pinch_last_distance = distance;
            }
            return;
        }
        if self.input.cam_pointer == Some(id) {
            let dx = x - self.input.cam_last_x;
            let dy = y - self.input.cam_last_y;
            self.input.cam_drag_dist += dx.abs() + dy.abs();
            self.input.cam_last_x = x;
            self.input.cam_last_y = y;
            self.cam_yaw -= (dx as f32) * 0.007;
            self.cam_pitch = clamp_f32(self.cam_pitch + (dy as f32) * 0.0055, -0.58, 1.18);
        }
    }

    pub fn on_touch_end(&mut self, id: u64, x: i32, y: i32) {
        if self.input.joy_pointer == Some(id) {
            self.input.joy_pointer = None;
            self.input.joy_x = 0.0;
            self.input.joy_y = 0.0;
            self.input.joy_origin_x = 0;
            self.input.joy_origin_y = 0;
            return;
        }
        if self.input.pinch_pointer == Some(id) {
            self.input.pinch_pointer = None;
            self.input.pinch_last_distance = 0.0;
            return;
        }
        if self.input.cam_pointer == Some(id) {
            self.input.cam_pointer = None;
            if self.input.cam_drag_dist < 22 {
                self.input.dragging = true;
                self.input.drag_dist = 0;
                self.on_pointer_up(x, y);
            }
        }
    }
}
