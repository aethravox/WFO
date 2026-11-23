use super::*;

impl WorldState {
    pub(crate) fn new(accounts: Arc<AccountStore>) -> Self {
        let mut ws = Self {
            accounts,
            next_id: 1,
            rng_seed: 0x9E37_79B9_7F4A_7C15,
            players: HashMap::new(),
            mobs: HashMap::new(),
            crystals: HashMap::new(),
            chat: Vec::new(),
        };
        ws.spawn_entities();
        ws
    }

    pub(crate) fn complete_auth(
        &mut self,
        player_id: &str,
        result: Result<Account, AuthError>,
    ) -> bool {
        if let Ok(account) = &result {
            let account_in_use = self
                .players
                .iter()
                .any(|(id, player)| id != player_id && player.account_id == Some(account.id));
            if account_in_use {
                if let Some(player) = self.players.get_mut(player_id) {
                    player.auth_attempts = player.auth_attempts.saturating_add(1);
                    player.sink.send_packet(&ServerPacket::AuthFailure {
                        message: String::from("Esta cuenta ya tiene una sesión activa."),
                    });
                    return player.auth_attempts < 5;
                }
                return false;
            }
        }
        let Some(player) = self.players.get_mut(player_id) else {
            return false;
        };
        if player.authenticated {
            return true;
        }

        match result {
            Ok(account) => {
                player.authenticated = true;
                player.account_id = Some(account.id);
                player.auth_attempts = 0;
                player.snap.name = account.username.clone();
                player.snap.class_id = account.class_id;
                player.snap.level = account.level;
                player.snap.xp = account.xp;
                player.snap.xp_next = account.xp_next;
                player.snap.gold = account.gold;
                player.snap.rank_points = account.rank_points;
                player.snap.max_hp =
                    account.class_id.base_hp() + (account.level.saturating_sub(1)) * 28;
                player.snap.hp = player.snap.max_hp;
                player.snap.max_mp =
                    account.class_id.base_mp() + (account.level.saturating_sub(1)) * 16;
                player.snap.mp = player.snap.max_mp;
                player.sink.send_packet(&ServerPacket::AuthSuccess {
                    username: account.username,
                    class_id: account.class_id,
                    level: account.level,
                    xp: account.xp,
                    xp_next: account.xp_next,
                    gold: account.gold,
                });
                true
            }
            Err(error) => {
                if let AuthError::Storage(ref reason) = error {
                    eprintln!("[wfo-server] Account storage error: {reason}");
                }
                player.auth_attempts = player.auth_attempts.saturating_add(1);
                let message = match error {
                    AuthError::InvalidInput => "Nombre o contraseña no válidos.",
                    AuthError::InvalidCredentials => "Usuario o contraseña incorrectos.",
                    AuthError::RateLimited => "Demasiados intentos. Espera un minuto.",
                    AuthError::RegistrationFailed => "No se pudo crear la cuenta.",
                    AuthError::Storage(_) => "Error al acceder a las cuentas.",
                };
                player.sink.send_packet(&ServerPacket::AuthFailure {
                    message: message.to_string(),
                });
                player.auth_attempts < 5
            }
        }
    }

    pub(crate) fn persist_account(&self, player_id: &str) {
        let Some(player) = self.players.get(player_id) else {
            return;
        };
        let Some(account_id) = player.account_id else {
            return;
        };
        if let Err(error) = self.accounts.save_progress(
            account_id,
            player.snap.class_id,
            player.snap.level,
            player.snap.xp,
            player.snap.xp_next,
            player.snap.gold,
            player.snap.rank_points,
        ) {
            eprintln!(
                "[wfo-server] Could not persist account {}: {error}",
                player.snap.name
            );
        }
    }

    pub(crate) fn next_rand(&mut self) -> f32 {
        self.rng_seed ^= self.rng_seed << 13;
        self.rng_seed ^= self.rng_seed >> 7;
        self.rng_seed ^= self.rng_seed << 17;
        ((self.rng_seed & 0xFFFF) as f32) / 65535.0
    }

    pub(crate) fn gen_id(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{}_{}", prefix, self.next_id)
    }

    fn spawn_entities(&mut self) {
        let now = Instant::now();
        let templates = [
            (
                MobKind::Stalker,
                "Rift Stalker",
                -32.0f32,
                18.0f32,
                1,
                90,
                5.0,
                14.0,
                2.6,
                9,
                28,
                12,
            ),
            (
                MobKind::Stalker,
                "Rift Stalker",
                -46.0,
                4.0,
                1,
                95,
                5.2,
                14.0,
                2.6,
                10,
                30,
                14,
            ),
            (
                MobKind::Stalker,
                "Rift Stalker",
                -36.0,
                -22.0,
                2,
                115,
                5.4,
                15.0,
                2.6,
                12,
                36,
                18,
            ),
            (
                MobKind::Stalker,
                "Rift Stalker",
                -16.0,
                38.0,
                1,
                90,
                5.0,
                14.0,
                2.6,
                9,
                28,
                12,
            ),
            (
                MobKind::Stalker,
                "Rift Stalker",
                12.0,
                44.0,
                2,
                115,
                5.3,
                15.0,
                2.6,
                12,
                36,
                18,
            ),
            (
                MobKind::Wisp,
                "Crystal Wisp",
                34.0,
                16.0,
                2,
                105,
                4.6,
                15.0,
                6.0,
                11,
                34,
                20,
            ),
            (
                MobKind::Wisp,
                "Crystal Wisp",
                50.0,
                4.0,
                2,
                110,
                4.8,
                15.0,
                6.0,
                12,
                36,
                22,
            ),
            (
                MobKind::Wisp,
                "Crystal Wisp",
                42.0,
                -18.0,
                3,
                135,
                4.9,
                16.0,
                6.0,
                14,
                45,
                26,
            ),
            (
                MobKind::Wisp,
                "Crystal Wisp",
                28.0,
                -32.0,
                3,
                135,
                4.9,
                16.0,
                6.0,
                14,
                45,
                26,
            ),
            (
                MobKind::Sentinel,
                "Obsidian Sentinel",
                -18.0,
                -38.0,
                4,
                210,
                4.0,
                15.0,
                3.2,
                18,
                68,
                38,
            ),
            (
                MobKind::Sentinel,
                "Obsidian Sentinel",
                18.0,
                -40.0,
                4,
                210,
                4.0,
                15.0,
                3.2,
                18,
                68,
                38,
            ),
            (
                MobKind::Colossus,
                "Ancient Colossus",
                0.0,
                -58.0,
                6,
                620,
                3.6,
                18.0,
                4.5,
                28,
                240,
                140,
            ),
        ];

        for (idx, &(kind, name, x, z, level, hp, speed, aggro, atk_r, dmg, xp, gold)) in
            templates.iter().enumerate()
        {
            let id = format!("mob_{}", idx + 1);
            self.mobs.insert(
                id.clone(),
                ServerMob {
                    snap: MobSnapshot {
                        id,
                        kind,
                        name: name.to_string(),
                        x,
                        y: ground_height_at(x, z),
                        z,
                        yaw: 3.14159,
                        hp,
                        max_hp: hp,
                        level,
                        alive: true,
                        attack_tick: 0,
                    },
                    anchor_x: x,
                    anchor_z: z,
                    patrol_phase: (idx as f32) * 1.3,
                    speed,
                    aggro_range: aggro,
                    attack_range: atk_r,
                    damage: dmg,
                    xp_reward: xp,
                    gold_reward: gold,
                    aggro_target: None,
                    last_attack: now,
                    respawn_at: now,
                },
            );
        }

        let crystals = [
            (28.0f32, 8.0f32),
            (40.0, 22.0),
            (54.0, -8.0),
            (38.0, -28.0),
            (-26.0, 26.0),
            (-44.0, -12.0),
            (-12.0, -26.0),
            (16.0, 32.0),
        ];
        for (idx, &(x, z)) in crystals.iter().enumerate() {
            let id = format!("crystal_{}", idx + 1);
            self.crystals.insert(
                id.clone(),
                ServerCrystal {
                    snap: CrystalSnapshot {
                        id,
                        x,
                        y: ground_height_at(x, z),
                        z,
                        available: true,
                    },
                    respawn_at: now,
                },
            );
        }
    }

    pub(crate) fn broadcast(&self, pkt: &ServerPacket, exclude: Option<&str>) {
        for (id, p) in &self.players {
            if !p.joined {
                continue;
            }
            if let Some(ex) = exclude {
                if id == ex {
                    continue;
                }
            }
            p.sink.send_packet(pkt);
        }
    }

    pub(crate) fn broadcast_lobby(&self, room_id: &str, pkt: &ServerPacket, exclude: Option<&str>) {
        for (id, p) in &self.players {
            if !p.in_lobby || p.room_id != room_id {
                continue;
            }
            if let Some(ex) = exclude {
                if id == ex {
                    continue;
                }
            }
            p.sink.send_packet(pkt);
        }
    }

    pub(crate) fn grant_rewards(&mut self, player_id: &str, xp_gain: i32, gold_gain: i32) {
        let mut leveled_to: Option<i32> = None;
        let mut rank_up: Option<AdventurerRank> = None;
        if let Some(p) = self.players.get_mut(player_id) {
            let old_rank = AdventurerRank::from_points(p.snap.rank_points);
            p.snap.xp += xp_gain;
            p.snap.gold += gold_gain;
            p.snap.rank_points = p.snap.rank_points.saturating_add(xp_gain.max(0));
            let new_rank = AdventurerRank::from_points(p.snap.rank_points);
            if new_rank > old_rank {
                rank_up = Some(new_rank);
            }
            while p.snap.xp >= p.snap.xp_next {
                p.snap.xp -= p.snap.xp_next;
                p.snap.level += 1;
                p.snap.xp_next = ((p.snap.xp_next as f32) * 1.45) as i32;
                p.snap.max_hp = p.snap.class_id.base_hp() + (p.snap.level - 1) * 28;
                p.snap.max_mp = p.snap.class_id.base_mp() + (p.snap.level - 1) * 16;
                p.snap.hp = p.snap.max_hp;
                p.snap.mp = p.snap.max_mp;
                leveled_to = Some(p.snap.level);
            }
        }
        if let Some(lvl) = leveled_to {
            self.broadcast(
                &ServerPacket::LevelUp {
                    id: player_id.to_string(),
                    level: lvl,
                },
                None,
            );
        }
        if let Some(rank) = rank_up {
            self.broadcast(
                &ServerPacket::RankUp {
                    id: player_id.to_string(),
                    rank,
                },
                None,
            );
        }
        self.persist_account(player_id);
    }
}
