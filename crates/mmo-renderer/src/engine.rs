use super::*;

impl MmoClientEngine {
    pub fn new(width: usize, height: usize) -> Self {
        let slot = Rect::new(width as i32 - 86, 16, 70, 30);
        Self {
            fb: Framebuffer::new(width, height),
            lang: MmoLangController::new(slot),
            joined: false,
            connecting: false,
            self_id: None,
            lobby_name: String::from("Wayfarer"),
            lobby_class: ClassId::Vanguard,
            lobby_focus_name: false,
            players: BTreeMap::new(),
            mobs: BTreeMap::new(),
            crystals: BTreeMap::new(),
            chat: Vec::new(),
            chat_input: String::new(),
            chat_focused: false,
            target_mob_id: None,
            local_x: 0.0,
            local_y: 0.0,
            local_z: 4.0,
            local_yaw: 3.14159,
            local_vy: 0.0,
            grounded: true,
            local_moving: false,
            cam_yaw: 0.0,
            cam_pitch: 0.38,
            cam_dist: 12.5,
            elapsed: 0.0,
            move_net_accum: 0.0,
            cooldowns: [0.0, 0.0, 0.0],
            input: InputState::new(),
            rings: Vec::new(),
            popups: Vec::new(),
            outbox: Vec::new(),
            touch_mode: false,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        let w = width.max(320);
        let h = height.max(240);
        self.fb.resize(w, h);
        self.lang.set_slot(Rect::new(w as i32 - 86, 16, 70, 30));
    }

    pub fn apply_server_packet(&mut self, pkt: ServerPacket) {
        match pkt {
            ServerPacket::Init { self_id } => {
                self.self_id = Some(self_id);
                self.joined = true;
                self.connecting = false;
            }
            ServerPacket::Player(p) => {
                if let Some(ref sid) = self.self_id {
                    if &p.id == sid {
                        let drift = sqrt_f32(
                            (p.x - self.local_x) * (p.x - self.local_x)
                                + (p.z - self.local_z) * (p.z - self.local_z),
                        );
                        if drift > 16.0 || self.elapsed < 0.5 {
                            self.local_x = p.x;
                            self.local_y = p.y;
                            self.local_z = p.z;
                        }
                    }
                }
                self.players.insert(p.id.clone(), p);
            }
            ServerPacket::Mob(m) => {
                self.mobs.insert(m.id.clone(), m);
            }
            ServerPacket::Crystal(c) => {
                self.crystals.insert(c.id.clone(), c);
            }
            ServerPacket::PlayerLeft { id } => {
                self.players.remove(&id);
            }
            ServerPacket::CombatFx(fx) => {
                let color = if fx.heal > 0 {
                    0x10B981
                } else if fx.skill == 3 {
                    0x22D3EE
                } else {
                    fx.class_id.accent_rgb()
                };
                self.rings.push(ActiveRingFx {
                    x: fx.x,
                    y: ground_height_at(fx.x, fx.z) + 0.12,
                    z: fx.z,
                    color,
                    age: 0.0,
                    life: 0.45,
                    max_r: if fx.skill == 1 { 4.8 } else { 2.4 },
                });
                if fx.damage > 0 {
                    self.popups.push(FloatingPopup {
                        x: fx.x,
                        y: fx.y + 0.6,
                        z: fx.z,
                        text: if fx.crit {
                            format!("CRIT -{}", fx.damage)
                        } else {
                            format!("-{}", fx.damage)
                        },
                        color: if fx.crit { 0xFBBF24 } else { 0xFFFFFF },
                        age: 0.0,
                    });
                } else if fx.heal > 0 {
                    self.popups.push(FloatingPopup {
                        x: fx.x,
                        y: fx.y + 0.8,
                        z: fx.z,
                        text: format!("+{}", fx.heal),
                        color: 0x34D399,
                        age: 0.0,
                    });
                }
                if fx.xp_gained > 0 {
                    if let Some(ref sid) = self.self_id {
                        if &fx.attacker_id == sid {
                            self.popups.push(FloatingPopup {
                                x: fx.x,
                                y: fx.y + 1.3,
                                z: fx.z,
                                text: format!("+{} XP", fx.xp_gained),
                                color: 0x38BDF8,
                                age: 0.0,
                            });
                        }
                    }
                }
            }
            ServerPacket::LevelUp { id, level } => {
                if let Some(p) = self.players.get(&id) {
                    self.popups.push(FloatingPopup {
                        x: p.x,
                        y: p.y + 2.4,
                        z: p.z,
                        text: format!("LV {}!", level),
                        color: 0xF59E0B,
                        age: 0.0,
                    });
                }
            }
            ServerPacket::Chat(c) => {
                self.chat.push(c);
                if self.chat.len() > 24 {
                    self.chat.remove(0);
                }
            }
            ServerPacket::SnapshotEnd => {}
        }
    }

    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.001, 0.08);
        self.elapsed += dt;

        for cd in &mut self.cooldowns {
            if *cd > 0.0 {
                *cd = (*cd - dt).max(0.0);
            }
        }

        // Update VFX
        let mut i = self.rings.len();
        while i > 0 {
            i -= 1;
            self.rings[i].age += dt;
            if self.rings[i].age >= self.rings[i].life {
                self.rings.remove(i);
            }
        }

        let mut j = self.popups.len();
        while j > 0 {
            j -= 1;
            self.popups[j].age += dt;
            self.popups[j].y += dt * 1.3;
            if self.popups[j].age >= 0.95 {
                self.popups.remove(j);
            }
        }

        if self.joined {
            let speed = self
                .self_id
                .as_ref()
                .and_then(|id| self.players.get(id))
                .map(|p| p.class_id.move_speed())
                .unwrap_or(9.0);

            let mut mx = 0.0f32;
            let mut mz = 0.0f32;
            if !self.chat_focused {
                if self.input.w {
                    mz -= 1.0;
                }
                if self.input.s {
                    mz += 1.0;
                }
                if self.input.a {
                    mx -= 1.0;
                }
                if self.input.d {
                    mx += 1.0;
                }
                mx += self.input.joy_x;
                mz += self.input.joy_y;
            }

            let mag_sq = mx * mx + mz * mz;
            if mag_sq > 0.02 {
                let len = sqrt_f32(mag_sq).max(1.0);
                let nx = mx / len;
                let nz = mz / len;
                let sy = sin_approx(self.cam_yaw);
                let cy = cos_approx(self.cam_yaw);
                let wx = nx * cy + nz * sy;
                let wz = -nx * sy + nz * cy;

                self.local_x += wx * speed * dt;
                self.local_z += wz * speed * dt;
                let d = sqrt_f32(self.local_x * self.local_x + self.local_z * self.local_z);
                if d > PLAYABLE_RADIUS {
                    self.local_x = (self.local_x / d) * PLAYABLE_RADIUS;
                    self.local_z = (self.local_z / d) * PLAYABLE_RADIUS;
                }
                self.local_yaw = mmo_protocol::atan2_approx(wx, wz);
                self.local_moving = true;
            } else {
                self.local_moving = false;
            }

            let gy = ground_height_at(self.local_x, self.local_z);
            if !self.grounded || self.local_y > gy {
                self.local_vy -= 22.0 * dt;
                self.local_y += self.local_vy * dt;
                if self.local_y <= gy {
                    self.local_y = gy;
                    self.local_vy = 0.0;
                    self.grounded = true;
                }
            } else {
                self.local_y = gy;
            }

            self.move_net_accum += dt;
            if self.move_net_accum >= 0.05 {
                self.move_net_accum = 0.0;
                self.outbox.push(ClientPacket::Move {
                    x: self.local_x,
                    y: self.local_y,
                    z: self.local_z,
                    yaw: self.local_yaw,
                    moving: self.local_moving,
                });
            }
        }
    }

    pub fn trigger_skill(&mut self, skill: u8) {
        if !self.joined {
            return;
        }
        let s_idx = match skill {
            1 => 1,
            2 => 2,
            _ => 0,
        };
        if self.cooldowns[s_idx] > 0.0 {
            return;
        }
        self.cooldowns[s_idx] = match s_idx {
            0 => 0.52,
            1 => 3.0,
            _ => 5.0,
        };
        let target_id = self.target_mob_id.clone().unwrap_or_default();
        self.outbox.push(ClientPacket::Attack {
            skill: s_idx as u8,
            target_id,
            yaw: self.local_yaw,
        });
    }

    pub fn trigger_gather(&mut self) {
        if !self.joined {
            return;
        }
        let mut best_id: Option<String> = None;
        let mut best_d = 6.5f32;
        for c in self.crystals.values() {
            if !c.available {
                continue;
            }
            let dx = c.x - self.local_x;
            let dz = c.z - self.local_z;
            let d = sqrt_f32(dx * dx + dz * dz);
            if d < best_d {
                best_d = d;
                best_id = Some(c.id.clone());
            }
        }
        if let Some(cid) = best_id {
            self.outbox.push(ClientPacket::Gather { crystal_id: cid });
        }
    }

    pub fn trigger_jump(&mut self) {
        if self.joined && self.grounded && !self.chat_focused {
            self.local_vy = 8.4;
            self.grounded = false;
        }
    }

    pub fn on_char_input(&mut self, ch: char) {
        if ch.is_control() {
            return;
        }
        if !self.joined && self.lobby_focus_name {
            if self.lobby_name.len() < 16 && ch != '|' {
                self.lobby_name.push(ch);
            }
        } else if self.joined && self.chat_focused {
            if self.chat_input.len() < 120 && ch != '|' {
                self.chat_input.push(ch);
            }
        }
    }

    pub fn on_backspace(&mut self) {
        if !self.joined && self.lobby_focus_name {
            self.lobby_name.pop();
        } else if self.joined && self.chat_focused {
            self.chat_input.pop();
        }
    }

    pub fn on_enter(&mut self) {
        if !self.joined {
            self.submit_join();
            return;
        }
        if self.chat_focused {
            let trimmed = self.chat_input.trim().to_string();
            if !trimmed.is_empty() {
                self.outbox.push(ClientPacket::Chat { text: trimmed });
                self.chat_input.clear();
            }
            self.chat_focused = false;
        } else {
            self.chat_focused = true;
        }
    }

    pub fn submit_join(&mut self) {
        if self.connecting {
            return;
        }
        self.connecting = true;
        let name = if self.lobby_name.trim().is_empty() {
            String::from("Hero")
        } else {
            self.lobby_name.trim().to_string()
        };
        self.outbox.push(ClientPacket::Join {
            name,
            class_id: self.lobby_class,
        });
    }

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
        if x >= w - 154 && x <= w - 92 && y >= 16 && y <= 46 {
            self.touch_mode = !self.touch_mode;
            return;
        }

        // Virtual Jump button when touch_mode is active in-game
        if self.joined && self.touch_mode && x >= w - 88 && x <= w - 16 && y >= h - 134 && y <= h - 88 {
            self.trigger_jump();
            return;
        }

        if !self.joined {
            // Lobby UI hitboxes
            let panel_w = 540.min(w - 32);
            let panel_x = (w - panel_w) / 2;
            let panel_y = h - 245;

            // Name input box
            if x >= panel_x + 16 && x <= panel_x + panel_w - 170 && y >= panel_y + 34 && y <= panel_y + 66 {
                self.lobby_focus_name = true;
                return;
            }
            self.lobby_focus_name = false;

            // Enter World button
            if x >= panel_x + panel_w - 160 && x <= panel_x + panel_w - 16 && y >= panel_y + 34 && y <= panel_y + 66 {
                self.submit_join();
                return;
            }

            // 3 Class cards
            let card_w = (panel_w - 48) / 3;
            let card_y = panel_y + 98;
            for idx in 0..3 {
                let cx = panel_x + 16 + (idx as i32) * (card_w + 8);
                if x >= cx && x <= cx + card_w && y >= card_y && y <= card_y + 72 {
                    self.lobby_class = match idx {
                        0 => ClassId::Vanguard,
                        1 => ClassId::Arcanist,
                        _ => ClassId::Ranger,
                    };
                    return;
                }
            }
            return;
        }

        // In-Game HUD hitboxes
        // Chat input box
        if x >= 16 && x <= 290 && y >= h - 46 && y <= h - 16 {
            self.chat_focused = true;
            return;
        }
        self.chat_focused = false;

        // Action Bar (Bottom-Center)
        let bar_x = (w - 280) / 2;
        let bar_y = h - 68;
        if y >= bar_y && y <= bar_y + 52 && x >= bar_x && x <= bar_x + 280 {
            let slot = (x - bar_x) / 70;
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
            let hy = if m.kind == MobKind::Colossus { 2.2 } else { 1.1 };
            if let Some((sx, sy, _)) = cam.project_point(Vec3::new(m.x, m.y + hy, m.z), self.fb.width, self.fb.height) {
                let d = sqrt_f32((sx - x as f32) * (sx - x as f32) + (sy - y as f32) * (sy - y as f32));
                if d < best_screen_d {
                    best_screen_d = d;
                    clicked_mob = Some(m.id.clone());
                }
            }
        }
        if let Some(mid) = clicked_mob {
            self.target_mob_id = Some(mid);
        } else {
            self.trigger_skill(0);
        }
    }

    pub fn on_wheel(&mut self, delta: f32) {
        if self.joined {
            self.cam_dist = clamp_f32(self.cam_dist + delta * 0.8, 6.5, 22.0);
        }
    }

    pub fn on_touch_start(&mut self, id: u64, x: i32, y: i32) {
        self.touch_mode = true;
        let w = self.fb.width as i32;
        let h = self.fb.height as i32;

        if self.joined {
            // Virtual joystick zone (left side, above chat)
            let jx = 68;
            let jy = h - 208;
            let dx = x - jx;
            let dy = y - jy;
            if dx * dx + dy * dy <= 62 * 62 && self.input.joy_pointer.is_none() {
                self.input.joy_pointer = Some(id);
                self.update_joystick(x, y);
                return;
            }

            // Virtual Jump button (right side)
            if x >= w - 88 && x <= w - 16 && y >= h - 134 && y <= h - 88 {
                self.trigger_jump();
                return;
            }
        }

        if self.input.cam_pointer.is_none() {
            self.input.cam_pointer = Some(id);
            self.input.cam_last_x = x;
            self.input.cam_last_y = y;
            self.input.cam_drag_dist = 0;
        }
    }

    pub fn on_touch_move(&mut self, id: u64, x: i32, y: i32) {
        if self.input.joy_pointer == Some(id) {
            self.update_joystick(x, y);
            return;
        }
        if self.input.cam_pointer == Some(id) && self.joined {
            let dx = x - self.input.cam_last_x;
            let dy = y - self.input.cam_last_y;
            self.input.cam_drag_dist += dx.abs() + dy.abs();
            self.input.cam_last_x = x;
            self.input.cam_last_y = y;
            self.cam_yaw -= (dx as f32) * 0.007;
            self.cam_pitch = clamp_f32(self.cam_pitch + (dy as f32) * 0.0055, 0.14, 1.08);
        }
    }

    pub fn on_touch_end(&mut self, id: u64, x: i32, y: i32) {
        if self.input.joy_pointer == Some(id) {
            self.input.joy_pointer = None;
            self.input.joy_x = 0.0;
            self.input.joy_y = 0.0;
            return;
        }
        if self.input.cam_pointer == Some(id) {
            self.input.cam_pointer = None;
            if self.input.cam_drag_dist < 12 {
                // Emulate click at tap point
                self.input.dragging = true;
                self.input.drag_dist = 0;
                self.on_pointer_up(x, y);
            }
        }
    }
}
