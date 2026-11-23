use super::*;

impl MmoClientEngine {
    pub(crate) fn offline_attack(&mut self, skill: u8) {
        use super::*;

        let Some(player_id) = self.self_id.clone() else {
            return;
        };
        let (class_id, player_x, player_y, player_z, level, max_hp) = {
            let Some(player) = self.players.get_mut(&player_id) else {
                return;
            };
            (
                player.class_id,
                player.x,
                player.y,
                player.z,
                player.level,
                player.max_hp,
            )
        };
        let mp_cost = match skill {
            1 => 22,
            2 => 28,
            _ => 0,
        };
        let can_pay = self
            .players
            .get(&player_id)
            .is_some_and(|player| player.mp >= mp_cost);
        if !can_pay {
            return;
        }
        if let Some(player) = self.players.get_mut(&player_id) {
            player.mp -= mp_cost;
            player.attack_tick = player.attack_tick.wrapping_add(1);
        }

        if skill == 2 {
            match class_id {
                ClassId::Guardian => {
                    let shield = ((max_hp.max(1) as f32) * 0.3) as i32;
                    if let Some(player) = self.players.get_mut(&player_id) {
                        player.shield_hp = player.shield_hp.max(shield);
                    }
                    self.offline_shield_expires_at = self.elapsed + 8.0;
                }
                ClassId::Summoner | ClassId::Artificer => {
                    let companion_damage = if class_id == ClassId::Summoner {
                        34
                    } else {
                        28
                    };
                    let mut companion_effects = Vec::new();
                    let mut companion_rewards = (0i32, 0i32);
                    for mob in self.mobs.values_mut() {
                        if mob.alive
                            && distance_xz(mob.x, mob.z, player_x, player_z)
                                <= if class_id == ClassId::Summoner {
                                    18.0
                                } else {
                                    13.0
                                }
                        {
                            mob.hp = (mob.hp - companion_damage).max(0);
                            if mob.hp == 0 {
                                mob.alive = false;
                                let (xp, gold) = offline_mob_rewards(mob.kind, mob.level);
                                companion_rewards.0 = companion_rewards.0.saturating_add(xp);
                                companion_rewards.1 = companion_rewards.1.saturating_add(gold);
                            }
                            companion_effects.push(CombatFx {
                                attacker_id: player_id.clone(),
                                x: mob.x,
                                y: mob.y + 1.7,
                                z: mob.z,
                                skill,
                                class_id,
                                damage: companion_damage,
                                crit: false,
                                heal: 0,
                                xp_gained: 0,
                                gold_gained: 0,
                            });
                        }
                    }
                    for effect in companion_effects {
                        self.apply_server_packet(ServerPacket::CombatFx(effect));
                    }
                    self.grant_offline_rewards(companion_rewards.0, companion_rewards.1);
                }
                ClassId::Chronicler => {
                    let heal = ((max_hp as f32) * 0.22) as i32;
                    if let Some(player) = self.players.get_mut(&player_id) {
                        player.hp = (player.hp + heal).min(player.max_hp);
                    }
                }
                _ => {
                    let heal = ((max_hp as f32) * 0.34) as i32;
                    if let Some(player) = self.players.get_mut(&player_id) {
                        player.hp = (player.hp + heal).min(player.max_hp);
                    }
                }
            }
            if class_id == ClassId::Summoner
                || class_id == ClassId::Artificer
                || class_id == ClassId::Chronicler
                || class_id == ClassId::Guardian
            {
                self.apply_server_packet(ServerPacket::CombatFx(CombatFx {
                    attacker_id: player_id,
                    x: player_x,
                    y: player_y + 1.4,
                    z: player_z,
                    skill,
                    class_id,
                    damage: 0,
                    crit: false,
                    heal: 0,
                    xp_gained: 0,
                    gold_gained: 0,
                }));
                return;
            }
            self.apply_server_packet(ServerPacket::CombatFx(CombatFx {
                attacker_id: player_id,
                x: player_x,
                y: player_y + 1.4,
                z: player_z,
                skill,
                class_id,
                damage: 0,
                crit: false,
                heal: ((max_hp as f32) * 0.34) as i32,
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

        let hp_ratio = self
            .players
            .get(&player_id)
            .map(|player| player.hp as f32 / player.max_hp.max(1) as f32)
            .unwrap_or(1.0);
        let berserker_bonus = if class_id == ClassId::Berserker {
            1.0 + (1.0 - hp_ratio.clamp(0.0, 1.0)) * 0.8
        } else {
            1.0
        };
        let duelist_bonus = if class_id == ClassId::Duelist && !aoe {
            1.18
        } else {
            1.0
        };
        let damage = ((class_id.base_damage() as f32)
            * (1.0 + level.saturating_sub(1) as f32 * 0.16)
            * if aoe { 1.75 } else { 1.0 }
            * berserker_bonus
            * duelist_bonus) as i32;
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

    pub(crate) fn offline_gather_crystal(&mut self, crystal_id: &str) {
        let Some(player_id) = self.self_id.clone() else {
            return;
        };
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        let (x, z, class_id) = (player.x, player.z, player.class_id);
        let Some(crystal) = self.crystals.get(crystal_id) else {
            return;
        };
        if !crystal.available || distance_xz(crystal.x, crystal.z, x, z) > 6.5 {
            return;
        };
        let Some(crystal) = self.crystals.get_mut(crystal_id) else {
            return;
        };
        crystal.available = false;
        let snapshot = crystal.clone();
        self.offline_crystal_respawns
            .insert(crystal_id.to_string(), self.elapsed + 16.0);
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
        let old_rank = AdventurerRank::from_points(player.rank_points);
        player.xp = player.xp.saturating_add(xp_gain.max(0));
        player.gold = player.gold.saturating_add(gold_gain.max(0));
        player.rank_points = player.rank_points.saturating_add(xp_gain.max(0));
        let new_rank = AdventurerRank::from_points(player.rank_points);
        let rank_up = (new_rank > old_rank).then_some(new_rank);
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
        if let Some(rank) = rank_up {
            self.apply_server_packet(ServerPacket::RankUp {
                id: player_id.clone(),
                rank,
            });
        }
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
            self.offline_rank_points = player.rank_points;
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

pub(crate) fn distance_xz(ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    sqrt_f32((ax - bx) * (ax - bx) + (az - bz) * (az - bz))
}
