use super::*;

const OFFLINE_PLAYER_ID: &str = "offline_player";
const MAX_OFFLINE_LEVEL: i32 = 10_000;

impl MmoClientEngine {
    pub(crate) fn start_offline_world(&mut self) {
        if !self.offline_mode || self.joined {
            return;
        }

        self.offline_level = self.offline_level.clamp(1, MAX_OFFLINE_LEVEL);
        self.offline_xp_next = self.offline_xp_next.max(1);
        self.offline_xp = self.offline_xp.clamp(0, self.offline_xp_next - 1);
        self.offline_gold = self.offline_gold.max(0);
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
            attack_tick: 0,
        };

        self.players.clear();
        self.players.insert(player_id.clone(), player);
        self.self_id = Some(player_id);
        self.mobs.clear();
        self.crystals.clear();
        self.offline_mob_respawns.clear();
        self.offline_mob_attack_timers.clear();
        self.offline_crystal_respawns.clear();
        self.target_mob_id = None;
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
            let Some((class_id, fx_x, fx_y, fx_z, respawned)) =
                self.players.get_mut(&player_id).map(|player| {
                    player.hp = (player.hp - damage).max(0);
                    let respawned = player.hp == 0;
                    if respawned {
                        player.hp = player.max_hp;
                        player.mp = player.max_mp;
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
                damage,
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

    pub(crate) fn offline_attack(&mut self, skill: u8) {
        let Some(player_id) = self.self_id.clone() else {
            return;
        };
        let Some(player) = self.players.get_mut(&player_id) else {
            return;
        };
        let class_id = player.class_id;
        let player_x = player.x;
        let player_y = player.y;
        let player_z = player.z;
        let level = player.level;
        let mp_cost = match skill {
            1 => 22,
            2 => 28,
            _ => 0,
        };
        if player.mp < mp_cost {
            return;
        }
        player.mp -= mp_cost;
        player.attack_tick = player.attack_tick.wrapping_add(1);

        if skill == 2 {
            let heal = ((player.max_hp as f32) * 0.34) as i32;
            player.hp = (player.hp + heal).min(player.max_hp);
            self.apply_server_packet(ServerPacket::CombatFx(CombatFx {
                attacker_id: player_id,
                x: player_x,
                y: player_y + 1.4,
                z: player_z,
                skill,
                class_id,
                damage: 0,
                crit: false,
                heal,
                xp_gained: 0,
                gold_gained: 0,
            }));
            return;
        }

        let aoe = skill == 1;
        let range = class_id.attack_range() + if aoe { 3.0 } else { 0.0 };
        let target_id = self.target_mob_id.clone();
        let mut hit_ids = Vec::new();
        if !aoe {
            if let Some(target_id) = target_id {
                if self.mobs.get(&target_id).is_some_and(|mob| {
                    mob.alive && distance_xz(mob.x, mob.z, player_x, player_z) <= range + 1.5
                }) {
                    hit_ids.push(target_id);
                }
            }
        }
        if hit_ids.is_empty() {
            let mut candidates = self
                .mobs
                .iter()
                .filter(|(_, mob)| {
                    mob.alive && distance_xz(mob.x, mob.z, player_x, player_z) <= range
                })
                .map(|(id, mob)| (id.clone(), distance_xz(mob.x, mob.z, player_x, player_z)))
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| left.1.total_cmp(&right.1));
            hit_ids.extend(
                candidates
                    .into_iter()
                    .take(if aoe { 4 } else { 1 })
                    .map(|(id, _)| id),
            );
        }

        let damage = ((class_id.base_damage() as f32)
            * (1.0 + level.saturating_sub(1) as f32 * 0.16)
            * if aoe { 1.75 } else { 1.0 }) as i32;
        let mut rewards = (0i32, 0i32);
        let mut effects = Vec::new();
        for mob_id in hit_ids {
            let Some(mob) = self.mobs.get_mut(&mob_id) else {
                continue;
            };
            mob.hp = (mob.hp - damage.max(4)).max(0);
            let mut xp_gained = 0;
            let mut gold_gained = 0;
            if mob.hp == 0 {
                mob.alive = false;
                let (xp, gold) = offline_mob_rewards(mob.kind, mob.level);
                xp_gained = xp;
                gold_gained = gold;
                rewards.0 = rewards.0.saturating_add(xp);
                rewards.1 = rewards.1.saturating_add(gold);
                let respawn_delay = if mob.kind == MobKind::Colossus {
                    25.0
                } else {
                    12.0
                };
                self.offline_mob_respawns
                    .insert(mob_id.clone(), self.elapsed + respawn_delay);
            }
            effects.push(CombatFx {
                attacker_id: player_id.clone(),
                x: mob.x,
                y: mob.y + 1.7,
                z: mob.z,
                skill,
                class_id,
                damage: damage.max(4),
                crit: false,
                heal: 0,
                xp_gained,
                gold_gained,
            });
        }
        if rewards.0 > 0 || rewards.1 > 0 {
            self.grant_offline_rewards(rewards.0, rewards.1);
        }
        for effect in effects {
            self.apply_server_packet(ServerPacket::CombatFx(effect));
        }
        self.sync_offline_profile_stats();
    }

    pub(crate) fn offline_gather(&mut self) {
        let Some(player_id) = self.self_id.clone() else {
            return;
        };
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        let (x, z, class_id) = (player.x, player.z, player.class_id);
        let closest = self
            .crystals
            .iter()
            .filter(|(_, crystal)| crystal.available)
            .map(|(id, crystal)| (id.clone(), distance_xz(crystal.x, crystal.z, x, z)))
            .filter(|(_, distance)| *distance <= 6.5)
            .min_by(|left, right| left.1.total_cmp(&right.1));
        let Some((crystal_id, _)) = closest else {
            return;
        };
        let Some(crystal) = self.crystals.get_mut(&crystal_id) else {
            return;
        };
        crystal.available = false;
        let snapshot = crystal.clone();
        self.offline_crystal_respawns
            .insert(crystal_id, self.elapsed + 16.0);
        self.grant_offline_rewards(22, 18);
        self.apply_server_packet(ServerPacket::Crystal(snapshot.clone()));
        self.apply_server_packet(ServerPacket::CombatFx(CombatFx {
            attacker_id: player_id,
            x: snapshot.x,
            y: snapshot.y + 1.5,
            z: snapshot.z,
            skill: 3,
            class_id,
            damage: 0,
            crit: false,
            heal: 0,
            xp_gained: 22,
            gold_gained: 18,
        }));
        self.sync_offline_profile_stats();
    }

    fn grant_offline_rewards(&mut self, xp_gain: i32, gold_gain: i32) {
        let Some(player_id) = self.self_id.clone() else {
            return;
        };
        let Some(player) = self.players.get_mut(&player_id) else {
            return;
        };
        player.xp = player.xp.saturating_add(xp_gain.max(0));
        player.gold = player.gold.saturating_add(gold_gain.max(0));
        let mut level_up = None;
        while player.xp >= player.xp_next && player.level < MAX_OFFLINE_LEVEL {
            player.xp -= player.xp_next;
            player.level += 1;
            player.xp_next = ((player.xp_next as f32) * 1.45).clamp(1.0, i32::MAX as f32) as i32;
            let level_bonus = player.level.saturating_sub(1);
            player.max_hp = player
                .class_id
                .base_hp()
                .saturating_add(level_bonus.saturating_mul(28));
            player.max_mp = player
                .class_id
                .base_mp()
                .saturating_add(level_bonus.saturating_mul(16));
            player.hp = player.max_hp;
            player.mp = player.max_mp;
            level_up = Some(player.level);
        }
        if player.level >= MAX_OFFLINE_LEVEL {
            player.xp = player.xp.min(player.xp_next.saturating_sub(1));
        }
        self.sync_offline_profile_stats();
        if let Some(level) = level_up {
            self.apply_server_packet(ServerPacket::LevelUp {
                id: player_id,
                level,
            });
        }
    }

    fn sync_offline_profile_stats(&mut self) {
        let Some(player_id) = self.self_id.as_deref() else {
            return;
        };
        if let Some(player) = self.players.get(player_id) {
            self.offline_level = player.level;
            self.offline_xp = player.xp;
            self.offline_xp_next = player.xp_next;
            self.offline_gold = player.gold;
        }
    }
}

fn offline_mob_rewards(kind: MobKind, level: i32) -> (i32, i32) {
    match (kind, level) {
        (MobKind::Stalker, 2) => (36, 18),
        (MobKind::Wisp, 3) => (45, 26),
        (MobKind::Sentinel, _) => (68, 38),
        (MobKind::Colossus, _) => (240, 140),
        (MobKind::Stalker, _) => (28, 12),
        (MobKind::Wisp, _) => (34, 20),
    }
}

fn distance_xz(ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    sqrt_f32((ax - bx) * (ax - bx) + (az - bz) * (az - bz))
}
