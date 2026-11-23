use super::*;
impl WorldState {
    pub(crate) fn handle_packet(&mut self, player_id: &str, pkt: ClientPacket) {
        if matches!(
            &pkt,
            ClientPacket::RegisterAccount { .. } | ClientPacket::LoginAccount { .. }
        ) {
            return;
        }
        if !self
            .players
            .get(player_id)
            .is_some_and(|player| player.authenticated)
        {
            return;
        }

        let now = Instant::now();
        match pkt {
            ClientPacket::RegisterAccount { .. } | ClientPacket::LoginAccount { .. } => return,
            ClientPacket::DuelChallenge { target_id } => {
                let target_joined = self
                    .players
                    .get(&target_id)
                    .map(|p| p.joined)
                    .unwrap_or(false);
                if !target_joined {
                    return;
                }
                let challenger_joined = self
                    .players
                    .get(player_id)
                    .map(|p| p.joined)
                    .unwrap_or(false);
                if !challenger_joined {
                    return;
                }
                let duel = DuelState {
                    opponent_id: target_id.clone(),
                    round: 1,
                    wins: 0,
                    opponent_wins: 0,
                    round_active: true,
                    round_winner: String::new(),
                    pending_skill: None,
                };
                if let Some(p) = self.players.get_mut(player_id) {
                    p.duel = Some(duel.clone());
                }
                if let Some(p) = self.players.get_mut(&target_id) {
                    p.duel = Some(DuelState {
                        opponent_id: player_id.to_string(),
                        round: 1,
                        wins: 0,
                        opponent_wins: 0,
                        round_active: true,
                        round_winner: String::new(),
                        pending_skill: None,
                    });
                }
                if let (Some(a), Some(b)) =
                    (self.players.get(player_id), self.players.get(&target_id))
                {
                    let state_a = ServerPacket::DuelState {
                        opponent_id: b.snap.id.clone(),
                        opponent_name: b.snap.name.clone(),
                        round: 1,
                        wins: 0,
                        opponent_wins: 0,
                        round_active: true,
                        round_winner: String::new(),
                    };
                    let state_b = ServerPacket::DuelState {
                        opponent_id: a.snap.id.clone(),
                        opponent_name: a.snap.name.clone(),
                        round: 1,
                        wins: 0,
                        opponent_wins: 0,
                        round_active: true,
                        round_winner: String::new(),
                    };
                    a.sink.send_packet(&state_a);
                    b.sink.send_packet(&state_b);
                }
            }
            ClientPacket::DuelAction { target_id, skill } => {
                if target_id.is_empty() {
                    return;
                }
                let skill = skill.min(2);
                if let Some(p) = self.players.get_mut(player_id) {
                    if let Some(ref mut duel) = p.duel {
                        if duel.opponent_id != target_id || !duel.round_active {
                            return;
                        }
                        duel.pending_skill = Some(skill);
                    } else {
                        return;
                    }
                }

                let opponent_ready = self
                    .players
                    .get(&target_id)
                    .and_then(|p| p.duel.as_ref())
                    .and_then(|d| d.pending_skill)
                    .is_some();
                let own_skill = self
                    .players
                    .get(player_id)
                    .and_then(|p| p.duel.as_ref())
                    .and_then(|d| d.pending_skill);
                if !opponent_ready {
                    return;
                }
                let opponent_skill = self
                    .players
                    .get(&target_id)
                    .and_then(|p| p.duel.as_ref())
                    .and_then(|d| d.pending_skill)
                    .unwrap_or(0);
                let own_skill = own_skill.unwrap_or(0);

                let round_winner = if own_skill == opponent_skill {
                    String::from("EMPATE")
                } else if (own_skill == 0 && opponent_skill == 2)
                    || (own_skill == 1 && opponent_skill == 0)
                    || (own_skill == 2 && opponent_skill == 1)
                {
                    let name = self
                        .players
                        .get(player_id)
                        .map(|p| p.snap.name.clone())
                        .unwrap_or_default();
                    if let Some(p) = self.players.get_mut(player_id) {
                        if let Some(ref mut duel) = p.duel {
                            duel.wins += 1;
                        }
                    }
                    if let Some(p) = self.players.get_mut(&target_id) {
                        if let Some(ref mut duel) = p.duel {
                            duel.opponent_wins += 1;
                        }
                    }
                    name
                } else {
                    let name = self
                        .players
                        .get(&target_id)
                        .map(|p| p.snap.name.clone())
                        .unwrap_or_default();
                    if let Some(p) = self.players.get_mut(&target_id) {
                        if let Some(ref mut duel) = p.duel {
                            duel.wins += 1;
                        }
                    }
                    if let Some(p) = self.players.get_mut(player_id) {
                        if let Some(ref mut duel) = p.duel {
                            duel.opponent_wins += 1;
                        }
                    }
                    name
                };

                for pid in [player_id, target_id.as_str()] {
                    if let Some(p) = self.players.get_mut(pid) {
                        if let Some(ref mut duel) = p.duel {
                            duel.round_winner = round_winner.clone();
                            duel.round_active = duel.wins < 3 && duel.opponent_wins < 3;
                            if duel.round_active {
                                duel.round = (duel.round + 1).min(5);
                            } else {
                                duel.round = 5;
                            }
                            duel.pending_skill = None;
                        }
                    }
                }

                for pid in [player_id, target_id.as_str()] {
                    if let (Some(p), Some(d)) = (
                        self.players.get(pid),
                        self.players.get(pid).and_then(|p| p.duel.clone()),
                    ) {
                        let opponent = self.players.get(&d.opponent_id);
                        if let Some(opponent) = opponent {
                            p.sink.send_packet(&ServerPacket::DuelState {
                                opponent_id: opponent.snap.id.clone(),
                                opponent_name: opponent.snap.name.clone(),
                                round: d.round,
                                wins: d.wins,
                                opponent_wins: d.opponent_wins,
                                round_active: d.round_active,
                                round_winner: d.round_winner,
                            });
                        }
                    }
                }
            }
            ClientPacket::DuelSurrender => {
                if let Some(p) = self.players.get_mut(player_id) {
                    p.duel = None;
                }
            }
            ClientPacket::EnterLobby { name } => {
                if self
                    .players
                    .get(player_id)
                    .map(|p| p.joined || p.in_lobby)
                    .unwrap_or(true)
                {
                    return;
                }
                let clean = sanitize_field(name.trim(), 18);
                let clean_name = if clean.is_empty() {
                    String::from("Traveler")
                } else {
                    clean
                };
                let Some(room_id) = generate_room_id() else {
                    return;
                };
                let sink = {
                    let Some(p) = self.players.get_mut(player_id) else {
                        return;
                    };
                    p.snap.name = clean_name;
                    p.snap.x = 0.0;
                    p.snap.y = 0.0;
                    p.snap.z = 4.0;
                    p.movement_budget.reset(now);
                    p.joined = false;
                    p.in_lobby = true;
                    p.room_id = room_id.clone();
                    p.sink.clone()
                };
                sink.send_packet(&ServerPacket::LobbyInit {
                    self_id: player_id.to_string(),
                    room_id,
                });
            }
            ClientPacket::CreateInvite => {
                let sink = {
                    let Some(p) = self.players.get(player_id) else {
                        return;
                    };
                    if !p.in_lobby {
                        return;
                    }
                    p.sink.clone()
                };
                let room_id = self
                    .players
                    .get(player_id)
                    .map(|p| p.room_id.clone())
                    .unwrap_or_default();
                if !room_id.is_empty() {
                    sink.send_packet(&ServerPacket::LobbyInvite { room_id });
                }
            }
            ClientPacket::JoinLobbyRoom { room_id } => {
                let clean_room = sanitize_field(room_id.trim(), 24);
                let exists = self
                    .players
                    .values()
                    .any(|p| p.in_lobby && p.room_id == clean_room);
                if !exists {
                    return;
                }
                let sink = {
                    let Some(p) = self.players.get_mut(player_id) else {
                        return;
                    };
                    if !p.in_lobby {
                        return;
                    }
                    p.room_id = clean_room.clone();
                    p.snap.x = 0.0;
                    p.snap.z = 4.0;
                    p.movement_budget.reset(now);
                    p.sink.clone()
                };
                sink.send_packet(&ServerPacket::LobbyJoined {
                    room_id: clean_room.clone(),
                });
                for (id, p) in &self.players {
                    if p.in_lobby && p.room_id == clean_room {
                        sink.send_packet(&ServerPacket::Player(p.snap.clone()));
                        if id != player_id {
                            p.sink.send_packet(&ServerPacket::Player(
                                self.players
                                    .get(player_id)
                                    .map(|p| p.snap.clone())
                                    .unwrap_or_else(|| p.snap.clone()),
                            ));
                        }
                    }
                }
            }
            ClientPacket::RandomClass => {
                let seed = self.next_rand();
                let class_id = match (seed * 9.0) as u8 % 9 {
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
                let sink = {
                    let Some(p) = self.players.get_mut(player_id) else {
                        return;
                    };
                    if !p.in_lobby {
                        return;
                    }
                    p.snap.class_id = class_id;
                    let level_bonus = p.snap.level.saturating_sub(1);
                    p.snap.max_hp = class_id.base_hp() + level_bonus * 28;
                    p.snap.hp = p.snap.max_hp;
                    p.snap.max_mp = class_id.base_mp() + level_bonus * 16;
                    p.snap.mp = p.snap.max_mp;
                    p.snap.shield_hp = 0;
                    p.sink.clone()
                };
                self.persist_account(player_id);
                sink.send_packet(&ServerPacket::LobbyClass { class_id });
            }
            ClientPacket::EnterWorld => {
                let sink = {
                    let Some(p) = self.players.get_mut(player_id) else {
                        return;
                    };
                    if !p.in_lobby || p.joined {
                        return;
                    }
                    p.in_lobby = false;
                    p.joined = true;
                    p.snap.x = 0.0;
                    p.snap.y = ground_height_at(0.0, 4.0);
                    p.snap.z = 4.0;
                    p.snap.moving = false;
                    p.movement_budget.reset(now);
                    p.sink.clone()
                };
                sink.send_packet(&ServerPacket::WorldEntered);
                sink.send_packet(&ServerPacket::Init {
                    self_id: player_id.to_string(),
                });
                let world_snap = self.players.get(player_id).map(|p| p.snap.clone());
                if let Some(snap) = world_snap {
                    for other in self.players.values() {
                        if other.joined {
                            sink.send_packet(&ServerPacket::Player(other.snap.clone()));
                        }
                    }
                    for m in self.mobs.values() {
                        sink.send_packet(&ServerPacket::Mob(m.snap.clone()));
                    }
                    for c in self.crystals.values() {
                        sink.send_packet(&ServerPacket::Crystal(c.snap.clone()));
                    }
                    sink.send_packet(&ServerPacket::SnapshotEnd);
                    self.broadcast(&ServerPacket::Player(snap), Some(player_id));
                }
            }
            ClientPacket::Join { .. } => return,
            ClientPacket::Move {
                dx,
                dz,
                yaw,
                moving,
            } => {
                if !dx.is_finite() || !dz.is_finite() || !yaw.is_finite() {
                    return;
                }
                let Some(p) = self.players.get(player_id) else {
                    return;
                };
                let in_lobby = p.in_lobby;
                if !in_lobby && !p.joined {
                    return;
                }
                let speed = if in_lobby {
                    5.4
                } else {
                    p.snap.class_id.move_speed() * if now < p.haste_expires_at { 1.25 } else { 1.0 }
                };
                let Some(player) = self.players.get_mut(player_id) else {
                    return;
                };
                let (from_x, from_z) = (player.snap.x, player.snap.z);
                let direction = if moving { (dx, dz) } else { (0.0, 0.0) };
                let (mut cx, mut cz) = player.movement_budget.move_authoritatively(
                    from_x,
                    from_z,
                    direction.0,
                    direction.1,
                    speed,
                    now,
                );
                if in_lobby {
                    cx = cx.clamp(-11.5, 11.5);
                    cz = cz.clamp(-9.0, 9.0);
                    player.snap.x = cx;
                    player.snap.y = 0.0;
                    player.snap.z = cz;
                    player.snap.yaw = yaw;
                    let lobby_dx = cx - from_x;
                    let lobby_dz = cz - from_z;
                    player.snap.moving = moving && lobby_dx.hypot(lobby_dz) > 0.001;
                    let room_id = player.room_id.clone();
                    let snap = player.snap.clone();
                    if !room_id.is_empty() {
                        self.broadcast_lobby(&room_id, &ServerPacket::Player(snap), None);
                    }
                    return;
                }
                let dist = sqrt_f32(cx * cx + cz * cz);
                if dist > PLAYABLE_RADIUS {
                    cx = cx * PLAYABLE_RADIUS / dist;
                    cz = cz * PLAYABLE_RADIUS / dist;
                }
                let (cx, cz) = move_with_tree_collision(player.snap.x, player.snap.z, cx, cz);
                let moved_x = cx - player.snap.x;
                let moved_z = cz - player.snap.z;
                if moved_x.hypot(moved_z) > 0.001 {
                    player.snap.yaw = atan2_approx(moved_x, moved_z);
                }
                player.snap.x = cx;
                if player.jump_velocity == 0.0 {
                    player.snap.y = ground_height_at(cx, cz);
                }
                player.snap.z = cz;
                player.snap.moving = moved_x.hypot(moved_z) > 0.001;
            }
            ClientPacket::Dodge => {
                let Some(player) = self.players.get_mut(player_id) else {
                    return;
                };
                if !player.joined || now < player.dodge_cd_until || player.jump_velocity != 0.0 {
                    return;
                }
                let ground = ground_height_at(player.snap.x, player.snap.z);
                if player.snap.y > ground + 0.05 {
                    return;
                }
                let target_x = player.snap.x + sin_approx(player.snap.yaw) * 3.2;
                let target_z = player.snap.z + cos_approx(player.snap.yaw) * 3.2;
                let distance = sqrt_f32(target_x * target_x + target_z * target_z);
                let (target_x, target_z) = if distance > PLAYABLE_RADIUS {
                    (
                        target_x * PLAYABLE_RADIUS / distance,
                        target_z * PLAYABLE_RADIUS / distance,
                    )
                } else {
                    (target_x, target_z)
                };
                let (target_x, target_z) =
                    move_with_tree_collision(player.snap.x, player.snap.z, target_x, target_z);
                player.snap.x = target_x;
                player.snap.y = ground_height_at(target_x, target_z);
                player.snap.z = target_z;
                player.movement_budget.reset(now);
                player.dodge_cd_until = now + Duration::from_secs(3);
                player.snap.moving = false;
                let snapshot = player.snap.clone();
                self.broadcast(&ServerPacket::Player(snapshot), None);
            }
            ClientPacket::Jump => {
                let Some(player) = self.players.get_mut(player_id) else {
                    return;
                };
                let ground = ground_height_at(player.snap.x, player.snap.z);
                if player.joined && player.jump_velocity == 0.0 && player.snap.y <= ground + 0.05 {
                    player.snap.y = ground;
                    player.jump_velocity = 8.4;
                }
            }
            ClientPacket::Attack {
                skill,
                target_id,
                yaw: _,
            } => {
                let s_idx = match skill {
                    1 => 1,
                    2 => 2,
                    _ => 0,
                };
                let (px, py, pz, class_id, level, facing_yaw, hp_ratio, mp_ok, cd_ok) = {
                    let Some(p) = self.players.get_mut(player_id) else {
                        return;
                    };
                    if !p.joined {
                        return;
                    }
                    let mp_cost = match s_idx {
                        1 => 22,
                        2 => 28,
                        _ => 0,
                    };
                    let cd_dur = match s_idx {
                        1 => Duration::from_millis(3000),
                        2 => Duration::from_millis(5000),
                        _ => Duration::from_millis(if p.snap.class_id == ClassId::Duelist {
                            320
                        } else {
                            520
                        }),
                    };
                    let cd_ok = now >= p.skill_cd_until[s_idx];
                    let mp_ok = p.snap.mp >= mp_cost;
                    if cd_ok && mp_ok {
                        p.skill_cd_until[s_idx] = now + cd_dur;
                        p.snap.mp -= mp_cost;
                        p.snap.attack_tick = p.snap.attack_tick.wrapping_add(1);
                    }
                    (
                        p.snap.x,
                        p.snap.y,
                        p.snap.z,
                        p.snap.class_id,
                        p.snap.level,
                        p.snap.yaw,
                        p.snap.hp as f32 / p.snap.max_hp.max(1) as f32,
                        mp_ok,
                        cd_ok,
                    )
                };
                let (px, py, pz, class_id, level, facing_yaw, hp_ratio, mp_ok, cd_ok) = (
                    px, py, pz, class_id, level, facing_yaw, hp_ratio, mp_ok, cd_ok,
                );
                if !cd_ok || !mp_ok {
                    return;
                }

                // Skill 2: Heal / Mend
                if s_idx == 2 {
                    match class_id {
                        ClassId::Guardian => {
                            self.apply_guardian_shield(player_id, now);
                            self.broadcast(
                                &ServerPacket::CombatFx(CombatFx {
                                    attacker_id: player_id.to_string(),
                                    x: px,
                                    y: py + 1.4,
                                    z: pz,
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
                            return;
                        }
                        ClassId::Summoner | ClassId::Artificer => {
                            self.activate_companion(player_id, class_id, px, py, pz, now);
                            return;
                        }
                        ClassId::Chronicler => {
                            self.apply_chronicler_support(player_id, now);
                            return;
                        }
                        _ => {}
                    }
                    let heal = {
                        let p = self.players.get_mut(player_id).unwrap();
                        let h = ((p.snap.max_hp as f32) * 0.34) as i32;
                        p.snap.hp = (p.snap.hp + h).min(p.snap.max_hp);
                        h
                    };
                    self.broadcast(
                        &ServerPacket::CombatFx(CombatFx {
                            attacker_id: player_id.to_string(),
                            x: px,
                            y: py + 1.4,
                            z: pz,
                            skill: 2,
                            class_id,
                            damage: 0,
                            crit: false,
                            heal,
                            xp_gained: 0,
                            gold_gained: 0,
                        }),
                        None,
                    );
                    return;
                }

                let is_aoe = s_idx == 1;
                let range = class_id.attack_range() + if is_aoe { 3.0 } else { 0.0 };
                let dmg_mult = if is_aoe { 1.75 } else { 1.0 };

                let mut hit_ids: Vec<String> = Vec::new();
                if !is_aoe && !target_id.is_empty() {
                    if let Some(m) = self.mobs.get(&target_id) {
                        if m.snap.alive {
                            let d = sqrt_f32((m.snap.x - px).powi(2) + (m.snap.z - pz).powi(2));
                            if d <= range + 1.5
                                && (is_aoe
                                    || target_is_in_front(facing_yaw, px, pz, m.snap.x, m.snap.z))
                                && !terrain_blocks_line(
                                    px,
                                    py + 1.2,
                                    pz,
                                    m.snap.x,
                                    m.snap.y + 0.8,
                                    m.snap.z,
                                )
                            {
                                hit_ids.push(m.snap.id.clone());
                            }
                        }
                    }
                }
                if hit_ids.is_empty() {
                    let mut best: Option<(String, f32)> = None;
                    for (mid, m) in &self.mobs {
                        if !m.snap.alive {
                            continue;
                        }
                        let d = sqrt_f32((m.snap.x - px).powi(2) + (m.snap.z - pz).powi(2));
                        if d <= range
                            && (is_aoe
                                || target_is_in_front(facing_yaw, px, pz, m.snap.x, m.snap.z))
                            && !terrain_blocks_line(
                                px,
                                py + 1.2,
                                pz,
                                m.snap.x,
                                m.snap.y + 0.8,
                                m.snap.z,
                            )
                        {
                            if is_aoe {
                                hit_ids.push(mid.clone());
                            } else if best.as_ref().map(|b| d < b.1).unwrap_or(true) {
                                best = Some((mid.clone(), d));
                            }
                        }
                    }
                    if !is_aoe {
                        if let Some((bid, _)) = best {
                            hit_ids.push(bid);
                        }
                    }
                }

                if hit_ids.is_empty() {
                    self.broadcast(
                        &ServerPacket::CombatFx(CombatFx {
                            attacker_id: player_id.to_string(),
                            x: px + sin_approx(facing_yaw) * 2.2,
                            y: py + 1.2,
                            z: pz + cos_approx(facing_yaw) * 2.2,
                            skill: s_idx as u8,
                            class_id,
                            damage: 0,
                            crit: false,
                            heal: 0,
                            xp_gained: 0,
                            gold_gained: 0,
                        }),
                        None,
                    );
                    return;
                }

                let mut total_xp = 0;
                let mut total_gold = 0;
                let mut fx_list = Vec::new();

                for mid in hit_ids.into_iter().take(if is_aoe { 4 } else { 1 }) {
                    let r1 = self.next_rand();
                    let r2 = self.next_rand();
                    let crit = r1
                        < match class_id {
                            ClassId::Ranger => 0.28,
                            ClassId::Duelist => 0.34,
                            _ => 0.18,
                        };
                    let lvl_bonus = 1.0 + ((level - 1) as f32) * 0.16;
                    let berserker_bonus = if class_id == ClassId::Berserker {
                        1.0 + (1.0 - hp_ratio.clamp(0.0, 1.0)) * 0.8
                    } else {
                        1.0
                    };
                    let raw = (class_id.base_damage() as f32)
                        * lvl_bonus
                        * dmg_mult
                        * (0.9 + r2 * 0.22)
                        * berserker_bonus
                        * if crit { 1.65 } else { 1.0 };
                    let dmg = (raw as i32).max(4);

                    if let Some(m) = self.mobs.get_mut(&mid) {
                        m.snap.hp = (m.snap.hp - dmg).max(0);
                        m.aggro_target = Some(player_id.to_string());
                        let mut xp_gained = 0;
                        let mut gold_gained = 0;
                        if m.snap.hp == 0 {
                            m.snap.alive = false;
                            m.aggro_target = None;
                            let respawn_secs = if m.snap.kind == MobKind::Colossus {
                                25
                            } else {
                                12
                            };
                            m.respawn_at = now + Duration::from_secs(respawn_secs);
                            xp_gained = m.xp_reward;
                            gold_gained = m.gold_reward;
                            total_xp += xp_gained;
                            total_gold += gold_gained;
                        }
                        fx_list.push(CombatFx {
                            attacker_id: player_id.to_string(),
                            x: m.snap.x,
                            y: m.snap.y
                                + if m.snap.kind == MobKind::Colossus {
                                    3.2
                                } else {
                                    1.7
                                },
                            z: m.snap.z,
                            skill: s_idx as u8,
                            class_id,
                            damage: dmg,
                            crit,
                            heal: 0,
                            xp_gained,
                            gold_gained,
                        });
                    }
                }

                if total_xp > 0 || total_gold > 0 {
                    self.grant_rewards(player_id, total_xp, total_gold);
                }
                for fx in fx_list {
                    self.broadcast(&ServerPacket::CombatFx(fx), None);
                }
            }
            ClientPacket::Gather { crystal_id } => {
                let Some(p) = self.players.get(player_id) else {
                    return;
                };
                if !p.joined {
                    return;
                }
                let (px, pz) = (p.snap.x, p.snap.z);
                let class_id = p.snap.class_id;

                let gathered = if let Some(c) = self.crystals.get_mut(&crystal_id) {
                    if c.snap.available
                        && sqrt_f32((c.snap.x - px).powi(2) + (c.snap.z - pz).powi(2)) <= 6.5
                    {
                        c.snap.available = false;
                        c.respawn_at = now + Duration::from_secs(16);
                        Some(c.snap.clone())
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(cs) = gathered {
                    self.grant_rewards(player_id, 22, 18);
                    self.broadcast(&ServerPacket::Crystal(cs.clone()), None);
                    self.broadcast(
                        &ServerPacket::CombatFx(CombatFx {
                            attacker_id: player_id.to_string(),
                            x: cs.x,
                            y: cs.y + 1.5,
                            z: cs.z,
                            skill: 3,
                            class_id,
                            damage: 0,
                            crit: false,
                            heal: 0,
                            xp_gained: 22,
                            gold_gained: 18,
                        }),
                        None,
                    );
                }
            }
            ClientPacket::Chat { text } => {
                let clean = sanitize_field(text.trim(), 140);
                if clean.is_empty() {
                    return;
                }
                let Some(p) = self.players.get(player_id) else {
                    return;
                };
                if !p.joined {
                    return;
                }
                let entry = ChatEntry {
                    sender_id: player_id.to_string(),
                    sender_name: p.snap.name.clone(),
                    class_id: p.snap.class_id,
                    text: clean,
                };
                self.chat.push(entry.clone());
                if self.chat.len() > 40 {
                    self.chat.remove(0);
                }
                self.broadcast(&ServerPacket::Chat(entry), None);
            }
            ClientPacket::Emote { emote } => {
                if !self
                    .players
                    .get(player_id)
                    .is_some_and(|player| player.joined)
                {
                    return;
                }
                self.broadcast(
                    &ServerPacket::Emote {
                        player_id: player_id.to_string(),
                        emote,
                    },
                    None,
                );
            }
        }
    }
}
