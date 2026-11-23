use super::*;

impl MmoClientEngine {
    pub fn submit_auth(&mut self, register: bool) {
        let username = self.auth_username.trim();
        if !(3..=18).contains(&username.len())
            || !username
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            self.auth_status = String::from("Usuario: 3-18 caracteres (letras, números o _).");
            return;
        }
        if !(8..=128).contains(&self.auth_password.len())
            || self
                .auth_password
                .chars()
                .any(|ch| ch.is_control() || ch == '|')
        {
            self.auth_status = String::from("Contraseña: mínimo 8 caracteres.");
            return;
        }
        if !self.auth_connected {
            self.auth_status = String::from("Esperando conexión TLS con el servidor.");
            return;
        }

        self.auth_username = username.to_ascii_lowercase();
        self.lobby_name = self.auth_username.clone();
        self.auth_status = if register {
            String::from("Creando cuenta...")
        } else {
            String::from("Iniciando sesión...")
        };
        let packet = if register {
            ClientPacket::RegisterAccount {
                username: self.auth_username.clone(),
                password: self.auth_password.clone(),
            }
        } else {
            ClientPacket::LoginAccount {
                username: self.auth_username.clone(),
                password: self.auth_password.clone(),
            }
        };
        self.outbox.push(packet);
    }

    pub fn enter_offline_mode(&mut self) {
        self.offline_mode = true;
        self.auth_panel_open = false;
        self.auth_password.clear();
        self.auth_status = String::from("Modo local: el progreso no se sincroniza.");
    }

    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.001, 0.08);
        self.elapsed += dt;

        for cd in &mut self.cooldowns {
            if *cd > 0.0 {
                *cd = (*cd - dt).max(0.0);
            }
        }
        if self.dodge_cooldown > 0.0 {
            self.dodge_cooldown = (self.dodge_cooldown - dt).max(0.0);
        }
        if let Some(crystal_id) = self.gathering_id.clone() {
            if let Some(crystal) = self.crystals.get(&crystal_id) {
                let dx = crystal.x - self.local_x;
                let dz = crystal.z - self.local_z;
                let distance = sqrt_f32(dx * dx + dz * dz);
                if !crystal.available || distance > 7.0 {
                    self.gathering_id = None;
                    self.gathering_progress = 0.0;
                } else if self.local_moving {
                    self.gathering_id = None;
                    self.gathering_progress = 0.0;
                } else {
                    self.gathering_progress -= dt;
                    if self.gathering_progress <= 0.0 {
                        self.gathering_id = None;
                        self.gathering_progress = 0.0;
                        if self.offline_mode {
                            self.offline_gather_crystal(&crystal_id);
                        } else {
                            self.outbox.push(ClientPacket::Gather { crystal_id });
                        }
                    }
                }
            } else {
                self.gathering_id = None;
                self.gathering_progress = 0.0;
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

        if !self.joined {
            let mut mx = 0.0f32;
            let mut mz = 0.0f32;
            mx += if self.input.d { 1.0 } else { 0.0 } - if self.input.a { 1.0 } else { 0.0 };
            mz += if self.input.s { 1.0 } else { 0.0 } - if self.input.w { 1.0 } else { 0.0 };
            mx += self.input.joy_x;
            mz += self.input.joy_y;
            let mag_sq = mx * mx + mz * mz;
            if mag_sq > 0.02 {
                let len = sqrt_f32(mag_sq).max(1.0);
                let wx = mx / len;
                let wz = mz / len;
                self.local_x = (self.local_x + wx * 5.4 * dt).clamp(-11.5, 11.5);
                self.local_z = (self.local_z + wz * 5.4 * dt).clamp(-9.0, 9.0);
                self.local_yaw = mmo_protocol::atan2_approx(wx, wz);
                self.local_moving = true;
            } else {
                self.local_moving = false;
            }
            self.local_y = 0.0;
            self.move_net_accum += dt;
            if !self.offline_mode && self.move_net_accum >= 0.05 {
                self.move_net_accum = 0.0;
                self.outbox.push(ClientPacket::Move {
                    dx: if mag_sq > 0.02 {
                        let len = sqrt_f32(mag_sq).max(1.0);
                        mx / len
                    } else {
                        0.0
                    },
                    dz: if mag_sq > 0.02 {
                        let len = sqrt_f32(mag_sq).max(1.0);
                        mz / len
                    } else {
                        0.0
                    },
                    yaw: self.local_yaw,
                    moving: self.local_moving,
                });
            }
            return;
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

                let from_x = self.local_x;
                let from_z = self.local_z;
                let haste = self
                    .self_id
                    .as_ref()
                    .and_then(|id| self.players.get(id))
                    .is_some_and(|player| player.haste);
                let speed = speed * if haste { 1.25 } else { 1.0 };
                let target_x = (from_x + wx * speed * dt).clamp(-PLAYABLE_RADIUS, PLAYABLE_RADIUS);
                let target_z = (from_z + wz * speed * dt).clamp(-PLAYABLE_RADIUS, PLAYABLE_RADIUS);
                let distance = sqrt_f32(target_x * target_x + target_z * target_z);
                let (target_x, target_z) = if distance > PLAYABLE_RADIUS {
                    (
                        target_x * PLAYABLE_RADIUS / distance,
                        target_z * PLAYABLE_RADIUS / distance,
                    )
                } else {
                    (target_x, target_z)
                };
                (self.local_x, self.local_z) = mmo_protocol::move_with_tree_collision(
                    from_x, from_z, target_x, target_z,
                );
                let corrected_distance =
                    sqrt_f32(self.local_x * self.local_x + self.local_z * self.local_z);
                if corrected_distance > PLAYABLE_RADIUS {
                    self.local_x *= PLAYABLE_RADIUS / corrected_distance;
                    self.local_z *= PLAYABLE_RADIUS / corrected_distance;
                }
                let moved_x = self.local_x - from_x;
                let moved_z = self.local_z - from_z;
                if moved_x * moved_x + moved_z * moved_z > 0.000001 {
                    self.local_yaw = mmo_protocol::atan2_approx(moved_x, moved_z);
                    self.local_moving = true;
                } else {
                    self.local_moving = false;
                }
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
            if self.offline_mode {
                self.sync_offline_player();
                self.tick_offline_world();
            } else if self.move_net_accum >= 0.05 {
                self.move_net_accum %= 0.05;
                self.outbox.push(ClientPacket::Move {
                    dx: if mag_sq > 0.02 {
                        let len = sqrt_f32(mag_sq).max(1.0);
                        let nx = mx / len;
                        let nz = mz / len;
                        let sy = sin_approx(self.cam_yaw);
                        let cy = cos_approx(self.cam_yaw);
                        nx * cy + nz * sy
                    } else {
                        0.0
                    },
                    dz: if mag_sq > 0.02 {
                        let len = sqrt_f32(mag_sq).max(1.0);
                        let nx = mx / len;
                        let nz = mz / len;
                        let sy = sin_approx(self.cam_yaw);
                        let cy = cos_approx(self.cam_yaw);
                        -nx * sy + nz * cy
                    } else {
                        0.0
                    },
                    yaw: self.local_yaw,
                    moving: self.local_moving,
                });
            }
            self.discover_current_zone();
            self.update_bestiary_discoveries();
        }
    }

    fn discover_current_zone(&mut self) {
        let zone = zone_at(self.local_x, self.local_z);
        let index = match zone {
            ZoneId::Plaza => 0,
            ZoneId::Woods => 1,
            ZoneId::Citadel => 2,
            ZoneId::Veins => 3,
        };
        if self.discovered_zones[index] {
            return;
        }
        self.discovered_zones[index] = true;
        let name = match zone {
            ZoneId::Plaza => self.lang.tr("PLAZA SANTUARIO"),
            ZoneId::Woods => self.lang.tr("BOSQUE ESMERALDA"),
            ZoneId::Citadel => self.lang.tr("CIUDADELA"),
            ZoneId::Veins => self.lang.tr("VETAS CRISTAL"),
        };
        self.popups.push(FloatingPopup {
            x: self.local_x,
            y: self.local_y + 2.8,
            z: self.local_z,
            text: format!("ZONA DESCUBIERTA: {}", name),
            color: 0x38BDF8,
            age: 0.0,
        });
    }

    fn update_bestiary_discoveries(&mut self) {
        for mob in self.mobs.values() {
            let dx = mob.x - self.local_x;
            let dz = mob.z - self.local_z;
            if mob.alive && dx * dx + dz * dz <= 24.0 * 24.0 {
                let index = match mob.kind {
                    MobKind::Stalker => 0,
                    MobKind::Wisp => 1,
                    MobKind::Sentinel => 2,
                    MobKind::Colossus => 3,
                };
                self.bestiary_known[index] = true;
            }
        }
    }
}
