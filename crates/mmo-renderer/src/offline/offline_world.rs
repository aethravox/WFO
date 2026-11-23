use super::*;

const OFFLINE_PLAYER_ID: &str = "offline_player";
pub(crate) const MAX_OFFLINE_LEVEL: i32 = 10_000;

impl MmoClientEngine {
    pub(crate) fn start_offline_world(&mut self) {
        if !self.offline_mode || self.joined {
            return;
        }

        self.offline_level = self.offline_level.clamp(1, MAX_OFFLINE_LEVEL);
        self.offline_xp_next = self.offline_xp_next.max(1);
        self.offline_xp = self.offline_xp.clamp(0, self.offline_xp_next - 1);
        self.offline_gold = self.offline_gold.max(0);
        self.offline_rank_points = self.offline_rank_points.max(0);
        self.local_x = 0.0;
        self.local_z = 4.0;
        self.local_y = ground_height_at(self.local_x, self.local_z);
        self.local_yaw = 3.14159;
        self.local_vy = 0.0;
        self.grounded = true;
        self.local_moving = false;
        self.move_net_accum = 0.0;
        self.offline_regen_at = self.elapsed + 1.0;

        let player_id = String::from(OFFLINE_PLAYER_ID);
        let class_id = self.lobby_class;
        let level_bonus = self.offline_level.saturating_sub(1);
        let max_hp = class_id
            .base_hp()
            .saturating_add(level_bonus.saturating_mul(28));
        let max_mp = class_id
            .base_mp()
            .saturating_add(level_bonus.saturating_mul(16));
        let name = if self.lobby_name.trim().is_empty() {
            String::from("Wayfarer")
        } else {
            self.lobby_name.trim().to_string()
        };
        let player = PlayerSnapshot {
            id: player_id.clone(),
            name,
            class_id,
            x: self.local_x,
            y: self.local_y,
            z: self.local_z,
            yaw: self.local_yaw,
            moving: false,
            hp: max_hp,
            max_hp,
            mp: max_mp,
            max_mp,
            level: self.offline_level,
            xp: self.offline_xp,
            xp_next: self.offline_xp_next,
            gold: self.offline_gold,
            rank_points: self.offline_rank_points,
            attack_tick: 0,
            shield_hp: 0,
            haste: false,
        };

        self.players.clear();
        self.players.insert(player_id.clone(), player);
        self.self_id = Some(player_id);
        self.mobs.clear();
        self.crystals.clear();
        self.offline_mob_respawns.clear();
        self.offline_mob_attack_timers.clear();
        self.offline_crystal_respawns.clear();
        self.offline_shield_expires_at = 0.0;
        self.target_mob_id = None;
        self.discovered_zones = [false; 4];
        self.personal_marker = None;
        self.bestiary_known = [false; 4];
        self.bestiary_open = false;
        self.joined = true;
        self.connecting = false;
        self.auth_panel_open = false;
        self.auth_status = String::from("Mundo local: progreso guardado en este dispositivo.");

        let mobs = [
            (
                "local_mob_1",
                MobKind::Stalker,
                "Rift Stalker",
                -32.0,
                18.0,
                1,
                90,
            ),
            (
                "local_mob_2",
                MobKind::Stalker,
                "Rift Stalker",
                -46.0,
                4.0,
                1,
                95,
            ),
            (
                "local_mob_3",
                MobKind::Stalker,
                "Rift Stalker",
                -36.0,
                -22.0,
                2,
                115,
            ),
            (
                "local_mob_4",
                MobKind::Stalker,
                "Rift Stalker",
                -16.0,
                38.0,
                1,
                90,
            ),
            (
                "local_mob_5",
                MobKind::Stalker,
                "Rift Stalker",
                12.0,
                44.0,
                2,
                115,
            ),
            (
                "local_mob_6",
                MobKind::Wisp,
                "Crystal Wisp",
                34.0,
                16.0,
                2,
                105,
            ),
            (
                "local_mob_7",
                MobKind::Wisp,
                "Crystal Wisp",
                50.0,
                4.0,
                2,
                110,
            ),
            (
                "local_mob_8",
                MobKind::Wisp,
                "Crystal Wisp",
                42.0,
                -18.0,
                3,
                135,
            ),
            (
                "local_mob_9",
                MobKind::Wisp,
                "Crystal Wisp",
                28.0,
                -32.0,
                3,
                135,
            ),
            (
                "local_mob_10",
                MobKind::Sentinel,
                "Obsidian Sentinel",
                -18.0,
                -38.0,
                4,
                210,
            ),
            (
                "local_mob_11",
                MobKind::Sentinel,
                "Obsidian Sentinel",
                18.0,
                -40.0,
                4,
                210,
            ),
            (
                "local_mob_12",
                MobKind::Colossus,
                "Ancient Colossus",
                0.0,
                -58.0,
                6,
                620,
            ),
        ];
        for (id, kind, name, x, z, level, max_hp) in mobs {
            let id = String::from(id);
            self.mobs.insert(
                id.clone(),
                MobSnapshot {
                    id: id.clone(),
                    kind,
                    name: String::from(name),
                    x,
                    y: ground_height_at(x, z),
                    z,
                    yaw: 3.14159,
                    hp: max_hp,
                    max_hp,
                    level,
                    alive: true,
                    attack_tick: 0,
                },
            );
            self.offline_mob_attack_timers.insert(id, self.elapsed);
        }

        for (id, x, z) in [
            ("local_crystal_1", 28.0, 8.0),
            ("local_crystal_2", 40.0, 22.0),
            ("local_crystal_3", 54.0, -8.0),
            ("local_crystal_4", 38.0, -28.0),
            ("local_crystal_5", -26.0, 26.0),
            ("local_crystal_6", -44.0, -12.0),
            ("local_crystal_7", -12.0, -26.0),
            ("local_crystal_8", 16.0, 32.0),
        ] {
            self.crystals.insert(
                String::from(id),
                CrystalSnapshot {
                    id: String::from(id),
                    x,
                    y: ground_height_at(x, z),
                    z,
                    available: true,
                },
            );
        }
    }

    pub(crate) fn sync_offline_player(&mut self) {
        if !self.offline_mode || !self.joined {
            return;
        }
        let Some(player_id) = self.self_id.as_deref() else {
            return;
        };
        if let Some(player) = self.players.get_mut(player_id) {
            player.x = self.local_x;
            player.y = self.local_y;
            player.z = self.local_z;
            player.yaw = self.local_yaw;
            player.moving = self.local_moving;
        }
    }

    pub(crate) fn tick_offline_world(&mut self) {
        if self.elapsed >= self.offline_shield_expires_at {
            if let Some(player_id) = self.self_id.as_ref() {
                if let Some(player) = self.players.get_mut(player_id) {
                    player.shield_hp = 0;
                }
            }
        }

        let expired_mobs = self
            .offline_mob_respawns
            .iter()
            .filter(|(_, respawn_at)| self.elapsed >= **respawn_at)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in expired_mobs {
            self.offline_mob_respawns.remove(&id);
            if let Some(mob) = self.mobs.get_mut(&id) {
                mob.alive = true;
                mob.hp = mob.max_hp;
                mob.attack_tick = mob.attack_tick.wrapping_add(1);
            }
            self.offline_mob_attack_timers
                .insert(id, self.elapsed + 1.5);
        }

        let expired_crystals = self
            .offline_crystal_respawns
            .iter()
            .filter(|(_, respawn_at)| self.elapsed >= **respawn_at)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in expired_crystals {
            self.offline_crystal_respawns.remove(&id);
            if let Some(crystal) = self.crystals.get_mut(&id) {
                crystal.available = true;
            }
        }

        let player_position = (self.local_x, self.local_z);
        let safe_plaza =
            sqrt_f32(player_position.0 * player_position.0 + player_position.1 * player_position.1)
                < SAFE_PLAZA_RADIUS;
        let mut enemy_hits = Vec::new();
        if !safe_plaza {
            for mob in self.mobs.values_mut() {
                if !mob.alive {
                    continue;
                }
                let distance = distance_xz(mob.x, mob.z, player_position.0, player_position.1);
                let attack_range = if mob.kind == MobKind::Wisp { 6.0 } else { 3.2 };
                let attack_at = self
                    .offline_mob_attack_timers
                    .entry(mob.id.clone())
                    .or_insert(self.elapsed);
                if distance <= attack_range && self.elapsed >= *attack_at {
                    *attack_at = self.elapsed + 1.45;
                    mob.attack_tick = mob.attack_tick.wrapping_add(1);
                    let damage = match mob.kind {
                        MobKind::Stalker => 9,
                        MobKind::Wisp => 11,
                        MobKind::Sentinel => 18,
                        MobKind::Colossus => 26,
                    };
                    enemy_hits.push((mob.id.clone(), damage));
                }
            }
        }

        for (mob_id, damage) in enemy_hits {
            let Some(player_id) = self.self_id.clone() else {
                break;
            };
            let Some((class_id, fx_x, fx_y, fx_z, respawned, hp_damage)) =
                self.players.get_mut(&player_id).map(|player| {
                    let absorbed = damage.min(player.shield_hp.max(0));
                    player.shield_hp -= absorbed;
                    let hp_damage = damage - absorbed;
                    player.hp = (player.hp - hp_damage).max(0);
                    let respawned = player.hp == 0;
                    if respawned {
                        player.hp = player.max_hp;
                        player.mp = player.max_mp;
                        player.shield_hp = 0;
                    }
                    (
                        player.class_id,
                        if respawned { 0.0 } else { player.x },
                        if respawned {
                            ground_height_at(0.0, 4.0) + 1.4
                        } else {
                            player.y + 1.4
                        },
                        if respawned { 4.0 } else { player.z },
                        respawned,
                        hp_damage,
                    )
                })
            else {
                break;
            };
            if respawned {
                self.local_x = 0.0;
                self.local_z = 4.0;
                self.local_y = ground_height_at(self.local_x, self.local_z);
                self.local_vy = 0.0;
                self.grounded = true;
            }
            self.apply_server_packet(ServerPacket::CombatFx(CombatFx {
                attacker_id: mob_id,
                x: fx_x,
                y: fx_y,
                z: fx_z,
                skill: 0,
                class_id,
                damage: hp_damage,
                crit: false,
                heal: 0,
                xp_gained: 0,
                gold_gained: 0,
            }));
        }

        if safe_plaza && self.elapsed >= self.offline_regen_at {
            self.offline_regen_at = self.elapsed + 1.0;
            if let Some(player_id) = self.self_id.as_ref() {
                if let Some(player) = self.players.get_mut(player_id) {
                    player.hp = (player.hp + 2).min(player.max_hp);
                    player.mp = (player.mp + 2).min(player.max_mp);
                }
            }
        }
        self.sync_offline_player();
    }
}
