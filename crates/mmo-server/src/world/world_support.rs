use super::*;
impl WorldState {
    pub(crate) fn apply_guardian_shield(&mut self, caster_id: &str, now: Instant) {
        let Some(caster) = self.players.get(caster_id) else {
            return;
        };
        let (x, z) = (caster.snap.x, caster.snap.z);
        let shield_until = now + Duration::from_secs(8);
        let protected_players = self
            .players
            .iter()
            .filter(|(_, player)| {
                player.joined
                    && sqrt_f32((player.snap.x - x).powi(2) + (player.snap.z - z).powi(2)) <= 12.0
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();

        for id in protected_players {
            if let Some(player) = self.players.get_mut(&id) {
                if now >= player.shield_expires_at {
                    player.snap.shield_hp = 0;
                }
                let shield_amount = ((player.snap.max_hp.max(1) as f32) * 0.3) as i32;
                player.snap.shield_hp = player.snap.shield_hp.max(shield_amount);
                player.shield_expires_at = shield_until;
            }
        }
    }

    pub(crate) fn activate_companion(
        &mut self,
        caster_id: &str,
        class_id: ClassId,
        x: f32,
        y: f32,
        z: f32,
        now: Instant,
    ) {
        let kind = if class_id == ClassId::Summoner {
            CompanionKind::Spirit
        } else {
            CompanionKind::Turret
        };
        if let Some(player) = self.players.get_mut(caster_id) {
            player.companion = Some(ActiveCompanion {
                kind,
                x,
                z,
                expires_at: now + Duration::from_secs(8),
                next_attack_at: now + Duration::from_millis(450),
            });
        }
        self.broadcast(
            &ServerPacket::CombatFx(CombatFx {
                attacker_id: caster_id.to_string(),
                x,
                y: y + 1.0,
                z,
                skill: 2,
                class_id,
                damage: 0,
                crit: false,
                heal: 0,
                xp_gained: 0,
                gold_gained: 0,
            }),
            None,
        );
    }

    pub(crate) fn apply_chronicler_support(&mut self, caster_id: &str, now: Instant) {
        let Some(caster) = self.players.get(caster_id) else {
            return;
        };
        let (x, y, z, max_hp) = (
            caster.snap.x,
            caster.snap.y,
            caster.snap.z,
            caster.snap.max_hp,
        );
        let buff_until = now + Duration::from_secs(5);
        let healing = ((max_hp.max(1) as f32) * 0.22) as i32;
        let targets = self
            .players
            .iter()
            .filter(|(_, player)| {
                player.joined
                    && sqrt_f32((player.snap.x - x).powi(2) + (player.snap.z - z).powi(2)) <= 12.0
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();

        let mut heal_fxs = Vec::new();
        for id in targets {
            if let Some(player) = self.players.get_mut(&id) {
                player.haste_expires_at = buff_until;
                let amount = healing.min(player.snap.max_hp - player.snap.hp).max(0);
                player.snap.hp += amount;
                if amount > 0 {
                    let fx = ServerPacket::CombatFx(CombatFx {
                        attacker_id: caster_id.to_string(),
                        x: player.snap.x,
                        y: player.snap.y + 1.4,
                        z: player.snap.z,
                        skill: 2,
                        class_id: ClassId::Chronicler,
                        damage: 0,
                        crit: false,
                        heal: amount,
                        xp_gained: 0,
                        gold_gained: 0,
                    });
                    heal_fxs.push(fx);
                }
            }
        }
        for fx in heal_fxs {
            self.broadcast(&fx, None);
        }

        self.broadcast(
            &ServerPacket::CombatFx(CombatFx {
                attacker_id: caster_id.to_string(),
                x,
                y: y + 1.4,
                z,
                skill: 2,
                class_id: ClassId::Chronicler,
                damage: 0,
                crit: false,
                heal: 0,
                xp_gained: 0,
                gold_gained: 0,
            }),
            None,
        );
    }

    fn tick_companions(&mut self, now: Instant) {
        let player_ids = self.players.keys().cloned().collect::<Vec<_>>();
        for player_id in player_ids {
            let companion_data = {
                let Some(player) = self.players.get_mut(&player_id) else {
                    continue;
                };
                let Some(mut companion) = player.companion else {
                    continue;
                };
                if now >= companion.expires_at {
                    player.companion = None;
                    continue;
                }
                if now < companion.next_attack_at || !player.joined {
                    continue;
                }
                companion.next_attack_at = now + Duration::from_millis(1250);
                player.companion = Some(companion);
                (player.snap.class_id, player.snap.level, companion)
            };
            let (class_id, level, companion) = companion_data;
            let max_range = match companion.kind {
                CompanionKind::Spirit => 18.0,
                CompanionKind::Turret => 13.0,
            };
            let target = self
                .mobs
                .iter()
                .filter(|(_, mob)| {
                    mob.snap.alive
                        && sqrt_f32(
                            (mob.snap.x - companion.x).powi(2) + (mob.snap.z - companion.z).powi(2),
                        ) <= max_range
                })
                .map(|(id, mob)| {
                    (
                        id.clone(),
                        sqrt_f32(
                            (mob.snap.x - companion.x).powi(2) + (mob.snap.z - companion.z).powi(2),
                        ),
                    )
                })
                .min_by(|left, right| left.1.total_cmp(&right.1))
                .map(|(id, _)| id);
            let Some(mob_id) = target else {
                continue;
            };

            let damage = match companion.kind {
                CompanionKind::Spirit => 12 + level.saturating_sub(1) * 2,
                CompanionKind::Turret => 10 + level.saturating_sub(1) * 2,
            };
            let mut rewards = (0, 0);
            let effect = {
                let Some(mob) = self.mobs.get_mut(&mob_id) else {
                    continue;
                };
                mob.snap.hp = (mob.snap.hp - damage).max(0);
                mob.aggro_target = Some(player_id.clone());
                if mob.snap.hp == 0 {
                    mob.snap.alive = false;
                    mob.aggro_target = None;
                    mob.respawn_at = now
                        + Duration::from_secs(if mob.snap.kind == MobKind::Colossus {
                            25
                        } else {
                            12
                        });
                    rewards = (mob.xp_reward, mob.gold_reward);
                }
                CombatFx {
                    attacker_id: player_id.clone(),
                    x: mob.snap.x,
                    y: mob.snap.y + 1.7,
                    z: mob.snap.z,
                    skill: 2,
                    class_id,
                    damage,
                    crit: false,
                    heal: 0,
                    xp_gained: rewards.0,
                    gold_gained: rewards.1,
                }
            };
            if rewards.0 > 0 || rewards.1 > 0 {
                self.grant_rewards(&player_id, rewards.0, rewards.1);
            }
            self.broadcast(&ServerPacket::CombatFx(effect), None);
        }
    }

    pub(crate) fn tick(&mut self) {
        let now = Instant::now();
        let dt = 0.05f32;

        // Respawn Crystals
        let mut crystal_updates = Vec::new();
        for c in self.crystals.values_mut() {
            if !c.snap.available && now >= c.respawn_at {
                c.snap.available = true;
                crystal_updates.push(c.snap.clone());
            }
        }
        for cs in crystal_updates {
            self.broadcast(&ServerPacket::Crystal(cs), None);
        }

        // Passive Player Regen (faster in Safe Sanctuary Plaza)
        for p in self.players.values_mut() {
            if p.snap.shield_hp > 0 && now >= p.shield_expires_at {
                p.snap.shield_hp = 0;
            }
            p.snap.haste = now < p.haste_expires_at;
            if !p.joined {
                continue;
            }
            let ground = ground_height_at(p.snap.x, p.snap.z);
            if p.jump_velocity != 0.0 || p.snap.y > ground {
                p.jump_velocity -= 22.0 * dt;
                p.snap.y = (p.snap.y + p.jump_velocity * dt).max(ground);
                if p.snap.y <= ground {
                    p.snap.y = ground;
                    p.jump_velocity = 0.0;
                }
            } else {
                p.snap.y = ground;
            }
            let in_plaza = sqrt_f32(p.snap.x * p.snap.x + p.snap.z * p.snap.z) < SAFE_PLAZA_RADIUS;
            if in_plaza && p.snap.hp < p.snap.max_hp {
                p.snap.hp = (p.snap.hp + 2).min(p.snap.max_hp);
            }
            if p.snap.mp < p.snap.max_mp {
                p.snap.mp = (p.snap.mp + if in_plaza { 2 } else { 1 }).min(p.snap.max_mp);
            }
        }

        // Mob AI & Attacks
        let mob_ids: Vec<String> = self.mobs.keys().cloned().collect();
        let mut mob_fx: Vec<CombatFx> = Vec::new();

        for mid in mob_ids {
            let rand_hit = self.next_rand();
            let mob_alive = self.mobs.get(&mid).map(|m| m.snap.alive).unwrap_or(false);
            if !mob_alive {
                let m = self.mobs.get_mut(&mid).unwrap();
                if now >= m.respawn_at {
                    m.snap.alive = true;
                    m.snap.hp = m.snap.max_hp;
                    m.snap.x = m.anchor_x;
                    m.snap.z = m.anchor_z;
                    m.snap.y = ground_height_at(m.anchor_x, m.anchor_z);
                    m.aggro_target = None;
                }
                continue;
            }

            let (mx, mz, anchor_x, anchor_z, aggro_range, current_target) = {
                let m = self.mobs.get(&mid).unwrap();
                (
                    m.snap.x,
                    m.snap.z,
                    m.anchor_x,
                    m.anchor_z,
                    m.aggro_range,
                    m.aggro_target.clone(),
                )
            };

            let mut target_pos: Option<(String, f32, f32, ClassId)> = None;
            if let Some(tid) = current_target {
                if let Some(tp) = self.players.get(&tid) {
                    let in_plaza =
                        sqrt_f32(tp.snap.x * tp.snap.x + tp.snap.z * tp.snap.z) < SAFE_PLAZA_RADIUS;
                    let leash_d =
                        sqrt_f32((tp.snap.x - anchor_x).powi(2) + (tp.snap.z - anchor_z).powi(2));
                    if tp.joined && !in_plaza && leash_d < 28.0 {
                        target_pos = Some((tid, tp.snap.x, tp.snap.z, tp.snap.class_id));
                    }
                }
            }

            if target_pos.is_none() {
                let mut best_d = aggro_range;
                for (pid, tp) in &self.players {
                    if !tp.joined {
                        continue;
                    }
                    if sqrt_f32(tp.snap.x * tp.snap.x + tp.snap.z * tp.snap.z) < SAFE_PLAZA_RADIUS {
                        continue;
                    }
                    let d = sqrt_f32((tp.snap.x - mx).powi(2) + (tp.snap.z - mz).powi(2));
                    if d < best_d {
                        best_d = d;
                        target_pos = Some((pid.clone(), tp.snap.x, tp.snap.z, tp.snap.class_id));
                    }
                }
            }

            let mut dealt_damage: Option<(String, i32, f32, f32, f32, ClassId)> = None;
            {
                let m = self.mobs.get_mut(&mid).unwrap();
                if let Some((tid, tx, tz, t_class)) = target_pos {
                    m.aggro_target = Some(tid.clone());
                    let dx = tx - m.snap.x;
                    let dz = tz - m.snap.z;
                    let d = sqrt_f32(dx * dx + dz * dz);
                    m.snap.yaw = atan2_approx(dx, dz);
                    if d > m.attack_range * 0.85 {
                        let step = (d - m.attack_range * 0.75).min(m.speed * dt);
                        m.snap.x += (dx / d) * step;
                        m.snap.z += (dz / d) * step;
                        m.snap.y = ground_height_at(m.snap.x, m.snap.z);
                    } else {
                        let cd = if m.snap.kind == MobKind::Colossus {
                            Duration::from_millis(1900)
                        } else {
                            Duration::from_millis(1450)
                        };
                        if now.duration_since(m.last_attack) >= cd {
                            m.last_attack = now;
                            m.snap.attack_tick = m.snap.attack_tick.wrapping_add(1);
                            let dmg = ((m.damage as f32) * (0.85 + rand_hit * 0.3)) as i32;
                            dealt_damage =
                                Some((tid, dmg, tx, ground_height_at(tx, tz), tz, t_class));
                        }
                    }
                } else {
                    m.aggro_target = None;
                    m.patrol_phase += dt * 0.65;
                    let px = m.anchor_x + cos_approx(m.patrol_phase) * 3.2;
                    let pz = m.anchor_z + sin_approx(m.patrol_phase * 0.8) * 3.2;
                    let dx = px - m.snap.x;
                    let dz = pz - m.snap.z;
                    let d = sqrt_f32(dx * dx + dz * dz);
                    if d > 0.15 {
                        m.snap.yaw = atan2_approx(dx, dz);
                        let step = d.min(m.speed * 0.45 * dt);
                        m.snap.x += (dx / d) * step;
                        m.snap.z += (dz / d) * step;
                        m.snap.y = ground_height_at(m.snap.x, m.snap.z);
                    }
                }
            }

            if let Some((tid, dmg, tx, ty, tz, t_class)) = dealt_damage {
                let mut reset_aggro = false;
                let mut hp_damage = dmg;
                if let Some(tp) = self.players.get_mut(&tid) {
                    hp_damage = tp.absorb_shield(dmg, now);
                    tp.snap.hp = (tp.snap.hp - hp_damage).max(0);
                    if tp.snap.hp == 0 {
                        tp.snap.hp = tp.snap.max_hp;
                        tp.snap.mp = tp.snap.max_mp;
                        tp.snap.shield_hp = 0;
                        tp.shield_expires_at = now;
                        tp.snap.x = 0.0;
                        tp.snap.z = 3.5;
                        tp.snap.y = ground_height_at(0.0, 3.5);
                        reset_aggro = true;
                    }
                }
                if reset_aggro {
                    if let Some(m) = self.mobs.get_mut(&mid) {
                        m.aggro_target = None;
                    }
                }
                mob_fx.push(CombatFx {
                    attacker_id: mid,
                    x: tx,
                    y: ty + 1.5,
                    z: tz,
                    skill: 0,
                    class_id: t_class,
                    damage: hp_damage,
                    crit: false,
                    heal: 0,
                    xp_gained: 0,
                    gold_gained: 0,
                });
            }
        }

        for fx in mob_fx {
            self.broadcast(&ServerPacket::CombatFx(fx), None);
        }
        self.tick_companions(now);

        // Broadcast authoritative snapshot
        let any_joined = self.players.values().any(|p| p.joined);
        if !any_joined {
            return;
        }
        for p in self.players.values() {
            if p.joined {
                self.broadcast(&ServerPacket::Player(p.snap.clone()), None);
            }
        }
        for m in self.mobs.values() {
            self.broadcast(&ServerPacket::Mob(m.snap.clone()), None);
        }
        self.broadcast(&ServerPacket::SnapshotEnd, None);
    }
}
