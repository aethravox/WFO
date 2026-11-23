use super::*;

impl MmoClientEngine {
    pub fn toggle_personal_marker(&mut self) {
        if !self.joined {
            return;
        }
        self.personal_marker = match self.personal_marker {
            Some(_) => None,
            None => Some((self.local_x, self.local_z)),
        };
    }

    pub fn toggle_bestiary(&mut self) {
        if self.joined {
            self.bestiary_open = !self.bestiary_open;
        }
    }

    pub fn trigger_emote(&mut self, emote: EmoteId) {
        if !self.joined {
            return;
        }
        if self.offline_mode {
            if let Some(player_id) = self.self_id.clone() {
                self.apply_server_packet(ServerPacket::Emote { player_id, emote });
            }
        } else {
            self.outbox.push(ClientPacket::Emote { emote });
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
        } as usize;
        if self.lan_mode && self.lan_duel_opponent.is_some() {
            self.trigger_lan_skill(s_idx as u8);
            return;
        }
        if let Some(ref duel) = self.duel {
            if duel.round_active {
                self.outbox.push(ClientPacket::DuelAction {
                    target_id: duel.opponent_id.clone(),
                    skill: s_idx as u8,
                });
                return;
            }
        }
        if self.cooldowns[s_idx] > 0.0 {
            return;
        }
        self.cooldowns[s_idx] = match s_idx {
            0 => 0.52,
            1 => 3.0,
            _ => 5.0,
        };
        if self.offline_mode {
            self.offline_attack(s_idx as u8);
            return;
        }
        let target_id = self.target_mob_id.clone().unwrap_or_default();
        self.outbox.push(ClientPacket::Attack {
            skill: s_idx as u8,
            target_id,
            yaw: self.local_yaw,
        });
    }

    pub fn trigger_gather(&mut self) {
        if !self.joined {
            let dx = self.local_x - 0.0;
            let dz = self.local_z - -9.35;
            if sqrt_f32(dx * dx + dz * dz) < 4.5 {
                self.submit_join();
                return;
            }

            let cx = self.local_x + 9.0;
            let cz = self.local_z - 4.0;
            if sqrt_f32(cx * cx + cz * cz) < 3.2 {
                if self.lobby_server_ready {
                    self.outbox.push(ClientPacket::RandomClass);
                    return;
                }
                let seed = self
                    .lobby_name
                    .chars()
                    .fold(7u32, |acc, ch| acc.wrapping_mul(33).wrapping_add(ch as u32));
                self.lobby_class = match seed % 9 {
                    0 => ClassId::Vanguard,
                    1 => ClassId::Arcanist,
                    2 => ClassId::Ranger,
                    3 => ClassId::Guardian,
                    4 => ClassId::Duelist,
                    5 => ClassId::Summoner,
                    6 => ClassId::Artificer,
                    7 => ClassId::Chronicler,
                    _ => ClassId::Berserker,
                };
            }
            return;
        }
        if self.offline_mode {
            if let Some(crystal_id) = self.gathering_id.clone() {
                self.gathering_id = None;
                self.gathering_progress = 0.0;
                let _ = crystal_id;
            } else {
                self.start_gather();
            }
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
            if self.gathering_id.as_deref() == Some(cid.as_str()) {
                self.gathering_id = None;
                self.gathering_progress = 0.0;
            } else {
                self.gathering_id = Some(cid);
                self.gathering_progress = 2.4;
            }
        }
    }

    pub fn start_gather(&mut self) {
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
            self.gathering_id = Some(cid);
            self.gathering_progress = 2.4;
        }
    }

    pub fn trigger_dodge(&mut self) {
        if !self.joined || !self.grounded || self.chat_focused || self.dodge_cooldown > 0.0 {
            return;
        }
        let mut dx = 0.0f32;
        let mut dz = 0.0f32;
        if self.input.d {
            dx += 1.0;
        }
        if self.input.a {
            dx -= 1.0;
        }
        if self.input.s {
            dz += 1.0;
        }
        if self.input.w {
            dz -= 1.0;
        }
        dx += self.input.joy_x;
        dz += self.input.joy_y;
        let dodge_magnitude = sqrt_f32(dx * dx + dz * dz);
        if dodge_magnitude < 0.1 {
            dx = sin_approx(self.local_yaw);
            dz = cos_approx(self.local_yaw);
        } else {
            let length = dodge_magnitude.max(f32::EPSILON);
            dx /= length;
            dz /= length;
        }
        if self.offline_mode {
            let target_x = self.local_x + dx * 3.2;
            let target_z = self.local_z + dz * 3.2;
            (self.local_x, self.local_z) = mmo_protocol::move_with_tree_collision(
                self.local_x,
                self.local_z,
                target_x,
                target_z,
            );
            let distance = sqrt_f32(self.local_x * self.local_x + self.local_z * self.local_z);
            if distance > PLAYABLE_RADIUS {
                self.local_x *= PLAYABLE_RADIUS / distance;
                self.local_z *= PLAYABLE_RADIUS / distance;
            }
            self.local_y = ground_height_at(self.local_x, self.local_z);
        } else {
            self.outbox.push(ClientPacket::Dodge);
        }
        self.dodge_cooldown = 3.0;
        self.gathering_id = None;
        self.gathering_progress = 0.0;
    }

    pub fn trigger_jump(&mut self) {
        if self.joined && self.grounded && !self.chat_focused {
            self.local_vy = 8.4;
            self.grounded = false;
            if !self.offline_mode {
                self.outbox.push(ClientPacket::Jump);
            }
        }
    }

    pub fn on_char_input(&mut self, ch: char) {
        if ch.is_control() {
            return;
        }
        if !self.joined && !self.account_authenticated && self.auth_panel_open {
            if self.auth_password_focus {
                if self.auth_password.len() < 128 && ch != '|' {
                    self.auth_password.push(ch);
                }
            } else if (ch.is_ascii_alphanumeric() || ch == '_') && self.auth_username.len() < 18 {
                self.auth_username.push(ch);
                self.lobby_name = self.auth_username.clone();
            }
        } else if !self.joined && self.lobby_invite_focus {
            if self.lobby_invite_input.len() < 24 && ch != '|' {
                self.lobby_invite_input.push(ch);
            }
        } else if self.joined && self.chat_focused {
            if self.chat_input.len() < 120 && ch != '|' {
                self.chat_input.push(ch);
            }
        }
    }

    pub fn on_backspace(&mut self) {
        if !self.joined && !self.account_authenticated && self.auth_panel_open {
            if self.auth_password_focus {
                self.auth_password.pop();
            } else {
                self.auth_username.pop();
                self.lobby_name = self.auth_username.clone();
            }
        } else if !self.joined && self.lobby_invite_focus {
            self.lobby_invite_input.pop();
        } else if self.joined && self.chat_focused {
            self.chat_input.pop();
        }
    }

    pub fn on_enter(&mut self) {
        if !self.joined && !self.account_authenticated && self.auth_panel_open {
            if self.auth_password_focus {
                self.submit_auth(false);
            } else {
                self.auth_password_focus = true;
            }
            return;
        }
        if !self.joined {
            if self.lobby_invite_focus {
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
        if self.chat_focused {
            let trimmed = self.chat_input.trim().to_string();
            if !trimmed.is_empty() {
                let emote = match trimmed.as_str() {
                    "/saludar" | "/wave" => Some(EmoteId::Wave),
                    "/celebrar" | "/cheer" => Some(EmoteId::Cheer),
                    "/reir" | "/laugh" => Some(EmoteId::Laugh),
                    "/bailar" | "/dance" => Some(EmoteId::Dance),
                    _ => None,
                };
                if let Some(emote) = emote {
                    self.trigger_emote(emote);
                } else {
                    self.outbox.push(ClientPacket::Chat { text: trimmed });
                }
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
        if self.offline_mode {
            self.start_offline_world();
            return;
        }
        if !self.account_authenticated {
            if !self.offline_mode {
                self.auth_panel_open = true;
                self.auth_status = String::from("Inicia sesión para entrar al mundo online.");
            }
            return;
        }
        self.connecting = true;
        let name = if self.lobby_name.trim().is_empty() {
            String::from("Hero")
        } else {
            self.lobby_name.trim().to_string()
        };
        if self.lobby_server_ready {
            self.outbox.push(ClientPacket::EnterWorld);
        } else {
            self.outbox.push(ClientPacket::EnterLobby { name });
        }
    }

    pub fn join_lobby_room(&mut self, room_id: String) {
        let room_id = room_id.trim().chars().take(24).collect::<String>();
        if room_id.is_empty() {
            return;
        }
        self.outbox.push(ClientPacket::JoinLobbyRoom { room_id });
    }
}
