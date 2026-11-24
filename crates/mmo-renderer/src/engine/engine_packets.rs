use super::*;

impl MmoClientEngine {
    pub fn apply_server_packet(&mut self, pkt: ServerPacket) {
        match pkt {
            ServerPacket::AuthRequired => {
                self.auth_connected = true;
                self.auth_status = String::from("Servidor seguro conectado.");
            }
            ServerPacket::ConnectionStatus { connected } => {
                self.auth_connected = connected;
                if connected {
                    if !self.account_authenticated {
                        self.auth_panel_open = true;
                        self.auth_status = String::from("Servidor seguro conectado.");
                    }
                } else {
                    let had_session = self.account_authenticated || self.auth_connected;
                    self.account_authenticated = false;
                    self.auth_password.clear();
                    self.lobby_server_ready = false;
                    self.joined = false;
                    self.self_id = None;
                    self.players.clear();
                    self.mobs.clear();
                    self.crystals.clear();
                    self.offline_mode = had_session;
                    self.auth_panel_open = !had_session;
                    self.auth_status = if had_session {
                        String::from("Desconectado: perfil local, sin sincronización.")
                    } else {
                        String::from("Sin conexión TLS; comprueba servidor y certificado.")
                    };
                }
            }
            ServerPacket::AuthSuccess {
                username, class_id, ..
            } => {
                self.account_authenticated = true;
                self.offline_mode = false;
                self.auth_panel_open = false;
                self.lobby_name = username.clone();
                self.auth_username = username.clone();
                self.auth_password.clear();
                self.lobby_class = class_id;
                self.auth_status = String::from("Sesión iniciada.");
                self.outbox
                    .push(ClientPacket::EnterLobby { name: username });
            }
            ServerPacket::AuthFailure { message } => {
                self.auth_password.clear();
                self.auth_status = message;
            }
            ServerPacket::LobbyInit { self_id, room_id } => {
                self.self_id = Some(self_id);
                self.lobby_room_id = room_id;
                self.lobby_server_ready = true;
                self.connecting = false;
                self.outbox.push(ClientPacket::CreateInvite);
            }
            ServerPacket::LobbyInvite { room_id } => {
                self.lobby_room_id = room_id;
            }
            ServerPacket::LobbyJoined { room_id } => {
                self.lobby_room_id = room_id;
            }
            ServerPacket::LobbyClass { class_id } => {
                self.lobby_class = class_id;
            }
            ServerPacket::WorldEntered => {
                self.joined = true;
                self.connecting = false;
                self.local_x = 0.0;
                self.local_y = ground_height_at(0.0, 4.0);
                self.local_z = 4.0;
            }
            ServerPacket::DuelState {
                opponent_id,
                opponent_name,
                round,
                wins,
                opponent_wins,
                round_active,
                round_winner,
            } => {
                self.duel = Some(DuelUiState {
                    opponent_id,
                    opponent_name,
                    round,
                    wins,
                    opponent_wins,
                    round_active,
                    round_winner,
                });
            }
            ServerPacket::Init { self_id } => {
                self.self_id = Some(self_id);
                self.joined = true;
                self.connecting = false;
                self.discovered_zones = [false; 4];
                self.personal_marker = None;
                self.bestiary_known = [false; 4];
                self.bestiary_open = false;
                self.touch_emote_menu = false;
            }
            ServerPacket::Player(p) => {
                if let Some(ref sid) = self.self_id {
                    if &p.id == sid {
                        let drift = sqrt_f32(
                            (p.x - self.local_x) * (p.x - self.local_x)
                                + (p.z - self.local_z) * (p.z - self.local_z),
                        );
                        if self.elapsed < 0.5 {
                            self.local_x = p.x;
                            self.local_y = p.y;
                            self.local_z = p.z;
                        } else {
                            let correction = if drift > 0.5 { 0.35 } else { 0.12 };
                            self.local_x += (p.x - self.local_x) * correction;
                            self.local_z += (p.z - self.local_z) * correction;
                            self.local_y += (p.y - self.local_y) * correction;
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
                    self.rings.push(ActiveRingFx {
                        x: p.x,
                        y: p.y + 0.1,
                        z: p.z,
                        color: 0xFBBF24,
                        age: 0.0,
                        life: 0.9,
                        max_r: 5.5,
                    });
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
            ServerPacket::RankUp { id, rank } => {
                if let Some(p) = self.players.get(&id) {
                    self.popups.push(FloatingPopup {
                        x: p.x,
                        y: p.y + 3.0,
                        z: p.z,
                        text: format!("RANGO {}!", rank.as_code()),
                        color: 0xF472B6,
                        age: 0.0,
                    });
                }
            }
            ServerPacket::GuildNotice {
                kind,
                rank,
                missing,
            } => {
                let message = match kind {
                    0 => String::from("GREMIO: TARJETA DE AVENTURERO OTORGADA (RANGO H)"),
                    1 => format!("GREMIO: ASCENDIDO A RANGO {}!", rank.as_code()),
                    2 => format!(
                        "GREMIO: FALTAN {} PR PARA RANGO {}",
                        missing,
                        rank.as_code()
                    ),
                    _ => String::from("GREMIO: YA POSEES EL RANGO MAXIMO SSS"),
                };
                self.guild_notice = Some((message, 4.2));
                if kind == 0 {
                    self.popups.push(FloatingPopup {
                        x: self.local_x,
                        y: self.local_y + 2.9,
                        z: self.local_z,
                        text: String::from("TARJETA DE AVENTURERO!"),
                        color: 0xFBBF24,
                        age: 0.0,
                    });
                }
            }
            ServerPacket::Emote { player_id, emote } => {
                if let Some(player) = self.players.get(&player_id) {
                    self.popups.push(FloatingPopup {
                        x: player.x,
                        y: player.y + 2.8,
                        z: player.z,
                        text: String::from(emote.label()),
                        color: 0xF472B6,
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
}
