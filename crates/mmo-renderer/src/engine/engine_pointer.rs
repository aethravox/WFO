use super::*;

impl MmoClientEngine {
    pub fn on_pointer_down(&mut self, x: i32, y: i32) {
        self.input.dragging = true;
        self.input.last_mouse_x = x;
        self.input.last_mouse_y = y;
        self.input.drag_dist = 0;
    }

    pub fn on_pointer_move(&mut self, x: i32, y: i32) {
        if !self.input.dragging || !self.joined {
            return;
        }
        let dx = x - self.input.last_mouse_x;
        let dy = y - self.input.last_mouse_y;
        self.input.drag_dist += dx.abs() + dy.abs();
        self.input.last_mouse_x = x;
        self.input.last_mouse_y = y;
        self.cam_yaw -= (dx as f32) * 0.007;
        self.cam_pitch = clamp_f32(self.cam_pitch + (dy as f32) * 0.0055, 0.14, 1.08);
    }

    pub fn on_pointer_up(&mut self, x: i32, y: i32) {
        let was_click = self.input.dragging && self.input.drag_dist < 8;
        self.input.dragging = false;
        if !was_click {
            return;
        }

        // 1. Check LangButton in ready_slot (powered by aethravox/Lang!)
        if self.lang.handle_tap(x, y) {
            return;
        }

        let w = self.fb.width as i32;
        let h = self.fb.height as i32;

        // Touch / Pad HUD toggle button (left of LangButton)
        if x >= w - 152 && x <= w - 90 && y >= 12 && y <= 42 {
            self.touch_mode = !self.touch_mode;
            return;
        }
        if x >= w - 220 && x <= w - 158 && y >= 12 && y <= 42 {
            self.toggle_lan_mode();
            return;
        }
        if self.lan_mode {
            let panel_w = if w < 680 { w - 32 } else { 220 };
            let panel_x = if w < 680 { 16 } else { w - 236 };
            let panel_y = 52;
            if x >= panel_x && x <= panel_x + panel_w && y >= panel_y + 24 && y <= panel_y + 96 {
                let idx = ((y - (panel_y + 30)) / 14).clamp(0, 3) as usize;
                if let Some(peer) = self.lan_peers.get(idx) {
                    self.challenge_lan_peer(&peer.id.clone());
                }
                return;
            }
        }

        if !self.joined && !self.account_authenticated {
            if self.auth_panel_open {
                let (panel_x, panel_y, panel_w) = self.auth_panel_layout();
                if x >= panel_x + 8
                    && x <= panel_x + panel_w - 8
                    && y >= panel_y + 34
                    && y <= panel_y + 62
                {
                    self.auth_password_focus = false;
                } else if x >= panel_x + 8
                    && x <= panel_x + panel_w - 8
                    && y >= panel_y + 66
                    && y <= panel_y + 94
                {
                    self.auth_password_focus = true;
                } else if y >= panel_y + 102 && y <= panel_y + 136 {
                    let column = ((x - panel_x) * 3 / panel_w).clamp(0, 2);
                    match column {
                        0 => self.submit_auth(false),
                        1 => self.submit_auth(true),
                        _ => self.enter_offline_mode(),
                    }
                }
                return;
            }
            let offline_btn_y = if w < 540 { 126 } else { 96 };
            if x >= 16 && x <= 198 && y >= offline_btn_y && y <= offline_btn_y + 30 {
                self.auth_panel_open = true;
                self.auth_password_focus = false;
                return;
            }
        }

        // Virtual Jump button when touch_mode is active in-game
        if self.joined
            && self.touch_mode
            && x >= w - 88
            && x <= w - 16
            && y >= h - 134
            && y <= h - 88
        {
            self.trigger_jump();
            return;
        }

        if self.joined
            && self.touch_mode
            && x >= w - 160
            && x <= w - 96
            && y >= h - 134
            && y <= h - 88
        {
            self.trigger_dodge();
            return;
        }

        if !self.joined {
            if !self.account_authenticated && self.auth_panel_open {
                return;
            }
            let invite_w = if w < 680 { w - 32 } else { 270 };
            let invite_x = if w < 680 { 16 } else { w - 286 };
            let invite_y = if w < 680 { h - 98 } else { 128 };
            let input_w = invite_w - 74;
            if x >= invite_x && x <= invite_x + input_w && y >= invite_y && y <= invite_y + 38 {
                self.lobby_invite_focus = true;
                return;
            }
            if x >= invite_x + input_w
                && x <= invite_x + invite_w
                && y >= invite_y
                && y <= invite_y + 38
            {
                let room_id = self.lobby_invite_input.trim().to_string();
                if !room_id.is_empty() {
                    self.join_lobby_room(room_id);
                    self.lobby_invite_input.clear();
                    self.lobby_invite_focus = false;
                }
                return;
            }
            self.trigger_gather();
            return;
        }

        // In-Game HUD hitboxes
        // Chat input box (desktop or open mobile chat overlay)
        let compact_hud = w < 680 || self.touch_mode;
        if !compact_hud {
            if x >= 16 && x <= 290 && y >= h - 46 && y <= h - 16 {
                self.chat_focused = true;
                return;
            }
        } else if self.chat_focused {
            let cw = (w - 24).min(300);
            let cx = (w - cw) / 2;
            let cy = 54;
            if x >= cx && x <= cx + cw && y >= cy && y <= cy + 122 {
                return;
            }
        }
        self.chat_focused = false;

        // Action Bar (Bottom-Center)
        let compact_actions = w < 680;
        let bar_w = if compact_actions { 208 } else { 280 };
        let bar_x = (w - bar_w) / 2;
        let bar_y = h - 68;
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

        // World click: target nearest mob near click ray, else basic attack
        let cam = self.current_camera();
        let mut clicked_mob: Option<String> = None;
        let mut best_screen_d = 48.0f32;
        for m in self.mobs.values() {
            if !m.alive {
                continue;
            }
            let hy = if m.kind == MobKind::Colossus {
                2.2
            } else {
                1.1
            };
            if let Some((sx, sy, _)) =
                cam.project_point(Vec3::new(m.x, m.y + hy, m.z), self.fb.width, self.fb.height)
            {
                let d =
                    sqrt_f32((sx - x as f32) * (sx - x as f32) + (sy - y as f32) * (sy - y as f32));
                if d < best_screen_d {
                    best_screen_d = d;
                    clicked_mob = Some(m.id.clone());
                }
            }
        }
        if let Some(mid) = clicked_mob {
            self.target_mob_id = Some(mid);
        } else {
            if let Some((id, _)) = self.players.iter().find(|(id, p)| {
                self.self_id.as_ref().map(|sid| sid != *id).unwrap_or(false)
                    && sqrt_f32(
                        (p.x - self.local_x) * (p.x - self.local_x)
                            + (p.z - self.local_z) * (p.z - self.local_z),
                    ) < 6.0
            }) {
                self.outbox.push(ClientPacket::DuelChallenge {
                    target_id: id.clone(),
                });
            } else {
                self.trigger_skill(0);
            }
        }
    }

    pub fn on_wheel(&mut self, delta: f32) {
        if self.joined {
            self.cam_dist = clamp_f32(self.cam_dist + delta * 0.8, 6.5, 22.0);
        }
    }
}
