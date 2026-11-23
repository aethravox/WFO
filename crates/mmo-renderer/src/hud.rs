use super::*;

impl MmoClientEngine {
    pub(crate) fn render_hud(&mut self, cam: &Camera) {
        let w = self.fb.width as i32;
        let h = self.fb.height as i32;

        // Top-Right LangButton from aethravox/Lang ready_slot!
        let slot = self.lang.host.btn_bounds;
        self.fb.rounded_panel(
            slot.x,
            slot.y,
            slot.width as i32,
            slot.height as i32,
            0x0B121E,
            238,
        );
        self.fb.panel_border(
            slot.x,
            slot.y,
            slot.width as i32,
            slot.height as i32,
            0x38BDF8,
        );
        let lbl = self.lang.host.btn_label;
        let lw = self.fb.measure_text(lbl, 2);
        self.fb.draw_text_shadow(
            slot.x + (slot.width as i32 - lw) / 2,
            slot.y + 6,
            lbl,
            2,
            0xF8FAFC,
        );

        // Touch / Pad HUD toggle button
        let pad_lbl = self.lang.tr("TACTIL");
        let pad_x = w - 152;
        self.fb.fill_rect_alpha(
            pad_x,
            12,
            62,
            30,
            if self.touch_mode { 0x0F766E } else { 0x0B121E },
            225,
        );
        self.fb.stroke_rect(
            pad_x,
            12,
            62,
            30,
            if self.touch_mode { 0x2DD4BF } else { 0x475569 },
        );
        let pw = self.fb.measure_text(pad_lbl, 1);
        self.fb
            .draw_text(pad_x + (62 - pw) / 2, 21, pad_lbl, 1, 0xF8FAFC);

        let lan_x = w - 220;
        self.fb.fill_rect_alpha(
            lan_x,
            12,
            62,
            30,
            if self.lan_mode { 0x1E3A8A } else { 0x0B121E },
            225,
        );
        self.fb.stroke_rect(
            lan_x,
            12,
            62,
            30,
            if self.lan_mode { 0x60A5FA } else { 0x475569 },
        );
        let lan_lbl = "LAN";
        let lw2 = self.fb.measure_text(lan_lbl, 1);
        self.fb
            .draw_text(lan_x + (62 - lw2) / 2, 21, lan_lbl, 1, 0xF8FAFC);

        if self.lan_mode {
            let panel_w = if w < 680 { w - 32 } else { 220 };
            let panel_x = if w < 680 { 16 } else { w - 236 };
            let panel_y = 52;
            self.fb
                .fill_rect_alpha(panel_x, panel_y, panel_w, 96, 0x0B121E, 235);
            self.fb.stroke_rect(panel_x, panel_y, panel_w, 96, 0x60A5FA);
            self.fb
                .draw_text(panel_x + 10, panel_y + 10, "JUGADORES LAN", 1, 0x60A5FA);
            if self.lan_peers.is_empty() {
                self.fb
                    .draw_text(panel_x + 10, panel_y + 34, "BUSCANDO...", 1, 0x94A3B8);
            } else {
                for (idx, peer) in self.lan_peers.iter().take(4).enumerate() {
                    let y = panel_y + 30 + (idx as i32) * 14;
                    let name: String = peer.name.chars().take(16).collect();
                    self.fb.draw_text(panel_x + 10, y, &name, 1, 0xF8FAFC);
                }
            }
        }

        if !self.joined {
            // In-world private lodge HUD: clear top-right controls on narrow portrait phones.
            let narrow = w < 540;
            let base_y = if narrow { 50 } else { 16 };
            let title = self.lang.tr("CASA / MODO LOCAL");
            let title_w = (self.fb.measure_text(title, 2) + 28).min(w - 32);
            self.fb
                .rounded_panel(16, base_y, title_w, 34, 0x0B121E, 225);
            self.fb
                .panel_border(16, base_y, title_w, 34, 0x38BDF8);
            self.fb
                .draw_text_shadow(30, base_y + 8, title, 2, 0xF8FAFC);

            let cname = self.lobby_class.display_name();
            let badge_w = (w - 32).min(230);
            self.fb
                .fill_rect_alpha(16, base_y + 40, badge_w, 28, 0x0B121E, 220);
            self.fb.draw_text(
                26,
                base_y + 49,
                &format!("{} · {}", self.lobby_name, cname),
                1,
                0xF8FAFC,
            );
            if self.lobby_server_ready {
                let room = format!("INSTANCIA {}", self.lobby_room_id);
                self.fb
                    .fill_rect_alpha(16, base_y + 74, badge_w, 24, 0x0B121E, 210);
                self.fb
                    .draw_text(26, base_y + 80, &room, 1, 0x34D399);
            }

            let invite_w = if w < 680 { w - 32 } else { 270 };
            let invite_x = if w < 680 { 16 } else { w - 286 };
            let invite_y = if w < 680 { h - 98 } else { 128 };
            self.fb
                .fill_rect_alpha(invite_x, invite_y, invite_w, 38, 0x0B121E, 230);
            self.fb.stroke_rect(
                invite_x,
                invite_y,
                invite_w,
                38,
                if self.lobby_invite_focus {
                    0x38BDF8
                } else {
                    0x475569
                },
            );
            let input_w = invite_w - 74;
            let disp = if self.lobby_invite_focus && ((self.elapsed * 2.0) as i32 % 2 == 0) {
                format!("{}_", self.lobby_invite_input)
            } else if self.lobby_invite_input.is_empty() {
                String::from("CODIGO DE CASA")
            } else {
                self.lobby_invite_input.clone()
            };
            let disp_clipped: String = disp.chars().take(if w < 680 { 24 } else { 18 }).collect();
            self.fb
                .draw_text(invite_x + 10, invite_y + 15, &disp_clipped, 1, 0xF8FAFC);
            self.fb
                .fill_rect(invite_x + input_w, invite_y + 5, 58, 28, 0xF59E0B);
            let join = "UNIR";
            let jw = self.fb.measure_text(join, 1);
            self.fb.draw_text(
                invite_x + input_w + (58 - jw) / 2,
                invite_y + 13,
                join,
                1,
                0x090D16,
            );

            let prompt = if sqrt_f32(
                self.local_x * self.local_x + (self.local_z + 9.35) * (self.local_z + 9.35),
            ) < 4.5
            {
                "E  CRUZAR PORTAL AL MUNDO"
            } else if sqrt_f32(
                (self.local_x + 9.0) * (self.local_x + 9.0)
                    + (self.local_z - 4.0) * (self.local_z - 4.0),
            ) < 3.2
            {
                "E  SOLICITAR CLASE ALEATORIA"
            } else if self.local_x > 5.5 && self.local_z > 1.0 {
                "ALMACEN  ·  OFFLINE: SOLO TUTORIAL"
            } else if self.local_x > 5.0 && self.local_z < -1.5 {
                "ARENA DE DUELOS  ·  SIN PERDIDA DE OBJETOS"
            } else {
                "WASD / JOYSTICK  ·  E PARA INTERACTUAR"
            };
            let pw = (self.fb.measure_text(prompt, 1) + 24).min(w - 16);
            let px = ((w - pw) / 2).max(8);
            self.fb.fill_rect_alpha(px, h - 52, pw, 36, 0x0B121E, 230);
            self.fb.stroke_rect(px, h - 52, pw, 36, 0x38BDF8);
            self.fb.draw_text(px + 12, h - 40, prompt, 1, 0xF8FAFC);

            if !self.account_authenticated && self.auth_panel_open {
                let (panel_x, panel_y, panel_w) = self.auth_panel_layout();
                self.fb
                    .rounded_panel(panel_x, panel_y, panel_w, 146, 0x0B121E, 245);
                self.fb
                    .panel_border(panel_x, panel_y, panel_w, 146, 0x38BDF8);
                self.fb.draw_text(
                    panel_x + 10,
                    panel_y + 7,
                    "CUENTA ONLINE · TLS",
                    1,
                    0x38BDF8,
                );
                let status: String = self.auth_status.chars().take(42).collect();
                self.fb
                    .draw_text(panel_x + 10, panel_y + 20, &status, 1, 0xCBD5E1);

                let username_y = panel_y + 34;
                self.fb
                    .fill_rect_alpha(panel_x + 8, username_y, panel_w - 16, 26, 0x111827, 255);
                self.fb.stroke_rect(
                    panel_x + 8,
                    username_y,
                    panel_w - 16,
                    26,
                    if self.auth_password_focus {
                        0x475569
                    } else {
                        0x38BDF8
                    },
                );
                let username: String = self.auth_username.chars().take(32).collect();
                self.fb.draw_text(
                    panel_x + 14,
                    username_y + 7,
                    &format!("USER: {}", username),
                    1,
                    0xF8FAFC,
                );

                let password_y = panel_y + 66;
                self.fb
                    .fill_rect_alpha(panel_x + 8, password_y, panel_w - 16, 26, 0x111827, 255);
                self.fb.stroke_rect(
                    panel_x + 8,
                    password_y,
                    panel_w - 16,
                    26,
                    if self.auth_password_focus {
                        0x38BDF8
                    } else {
                        0x475569
                    },
                );
                let password = "*".repeat(self.auth_password.chars().count().min(32));
                self.fb.draw_text(
                    panel_x + 14,
                    password_y + 7,
                    &format!("PASS: {}", password),
                    1,
                    0xF8FAFC,
                );

                let button_y = panel_y + 102;
                let button_width = panel_w / 3;
                let labels = ["ENTRAR", "CREAR", "LOCAL"];
                for (idx, label) in labels.iter().enumerate() {
                    let button_x = panel_x + idx as i32 * button_width;
                    let color = if idx < 2 && !self.auth_connected {
                        0x334155
                    } else if idx == 2 {
                        0x0F766E
                    } else {
                        0x1D4ED8
                    };
                    self.fb
                        .fill_rect(button_x + 4, button_y, button_width - 8, 32, color);
                    let label_w = self.fb.measure_text(label, 1);
                    self.fb.draw_text(
                        button_x + (button_width - label_w) / 2,
                        button_y + 10,
                        label,
                        1,
                        0xF8FAFC,
                    );
                }
            } else if !self.account_authenticated {
                let offline_btn_y = if narrow { 126 } else { 96 };
                self.fb
                    .fill_rect_alpha(16, offline_btn_y, 182, 30, 0x0B121E, 230);
                self.fb
                    .stroke_rect(16, offline_btn_y, 182, 30, 0x0F766E);
                self.fb.draw_text(
                    24,
                    offline_btn_y + 9,
                    "OFFLINE · LOGIN ONLINE",
                    1,
                    0xF8FAFC,
                );
            }

            if self.touch_mode && (self.account_authenticated || !self.auth_panel_open) {
                let jx = if self.input.joy_origin_x > 0 {
                    self.input.joy_origin_x
                } else {
                    74
                };
                let jy = if self.input.joy_origin_y > 0 {
                    self.input.joy_origin_y
                } else {
                    (h - 170).max(150)
                };
                self.fb
                    .rounded_panel(jx - 48, jy - 48, 96, 96, 0x0B121E, 165);
                self.fb.panel_border(jx - 48, jy - 48, 96, 96, 0x38BDF8);
                let kx = jx + (self.input.joy_x * 32.0) as i32;
                let ky = jy + (self.input.joy_y * 32.0) as i32;
                self.fb.fill_rect(kx - 15, ky - 15, 30, 30, 0x38BDF8);

                let ubx = w - 92;
                let uby = h - 156;
                self.fb.rounded_panel(ubx, uby, 76, 46, 0x0B121E, 215);
                self.fb.panel_border(ubx, uby, 76, 46, 0x22D3EE);
                let ulbl = "USAR (E)";
                let uw = self.fb.measure_text(ulbl, 1);
                self.fb
                    .draw_text(ubx + (76 - uw) / 2, uby + 16, ulbl, 1, 0xF8FAFC);
            }
            return;
        }

        // 3D Projected Nameplates & Damage Popups
        for (id, p) in &self.players {
            let is_self = self.self_id.as_ref() == Some(id);
            let (px, py, pz) = if is_self {
                (self.local_x, self.local_y, self.local_z)
            } else {
                (p.x, p.y, p.z)
            };
            if let Some((sx, sy, _)) =
                cam.project_point(Vec3::new(px, py + 2.45, pz), self.fb.width, self.fb.height)
            {
                let ix = sx as i32;
                let iy = sy as i32;
                let rank = AdventurerRank::from_points(p.rank_points);
                let tag = format!("{} · {} LV.{}", rank.as_code(), p.name, p.level);
                let tw = self.fb.measure_text(&tag, 1);
                self.fb
                    .fill_rect_alpha(ix - tw / 2 - 4, iy - 17, tw + 8, 14, 0x0B121E, 195);
                self.fb.draw_text(
                    ix - tw / 2,
                    iy - 16,
                    &tag,
                    1,
                    if is_self { 0xFFFFFF } else { 0x34D399 },
                );
                let hp_w = ((p.hp.max(0) as f32 / p.max_hp.max(1) as f32) * 44.0) as i32;
                self.fb.fill_rect(ix - 22, iy - 2, 44, 3, 0x1E293B);
                self.fb.fill_rect(ix - 22, iy - 2, hp_w, 3, 0x10B981);
            }
        }

        for m in self.mobs.values() {
            if !m.alive {
                continue;
            }
            let d = sqrt_f32(
                (m.x - self.local_x) * (m.x - self.local_x)
                    + (m.z - self.local_z) * (m.z - self.local_z),
            );
            if d > 35.0 && self.target_mob_id.as_ref() != Some(&m.id) {
                continue;
            }
            let hy = if m.kind == MobKind::Colossus {
                5.2
            } else {
                2.3
            };
            if let Some((sx, sy, _)) =
                cam.project_point(Vec3::new(m.x, m.y + hy, m.z), self.fb.width, self.fb.height)
            {
                let ix = sx as i32;
                let iy = sy as i32;
                let tag = format!("{} LV.{}", m.name, m.level);
                let tw = self.fb.measure_text(&tag, 1);
                self.fb
                    .fill_rect_alpha(ix - tw / 2 - 4, iy - 17, tw + 8, 14, 0x0B121E, 195);
                let tc = if m.kind == MobKind::Colossus {
                    0xFBBF24
                } else {
                    0xFCA5A5
                };
                self.fb.draw_text(ix - tw / 2, iy - 16, &tag, 1, tc);
                let hp_w = ((m.hp.max(0) as f32 / m.max_hp.max(1) as f32) * 48.0) as i32;
                self.fb.fill_rect(ix - 24, iy - 2, 48, 3, 0x1E293B);
                self.fb.fill_rect(ix - 24, iy - 2, hp_w, 3, 0xEF4444);
            }
        }

        for pop in &self.popups {
            if let Some((sx, sy, _)) = cam.project_point(
                Vec3::new(pop.x, pop.y, pop.z),
                self.fb.width,
                self.fb.height,
            ) {
                let tw = self.fb.measure_text(&pop.text, 2);
                self.fb.draw_text(
                    sx as i32 - tw / 2 + 1,
                    sy as i32 + 1,
                    &pop.text,
                    2,
                    0x000000,
                );
                self.fb
                    .draw_text(sx as i32 - tw / 2, sy as i32, &pop.text, 2, pop.color);
            }
        }

        if let Some(duel) = &self.duel {
            let panel_w = if w < 680 { w - 32 } else { 360 };
            let panel_x = (w - panel_w) / 2;
            let panel_y = 16;
            self.fb
                .fill_rect_alpha(panel_x, panel_y, panel_w, 58, 0x0B121E, 235);
            self.fb.stroke_rect(panel_x, panel_y, panel_w, 58, 0xF59E0B);
            let title = format!(
                "DUELLO {} · R{} · {} / {}",
                duel.opponent_name, duel.round, duel.wins, duel.opponent_wins
            );
            let title: String = title.chars().take(if w < 680 { 38 } else { 48 }).collect();
            self.fb
                .draw_text(panel_x + 10, panel_y + 10, &title, 1, 0xFBBF24);
            let outcome = if duel.round_winner.is_empty() {
                let skill_three = if let Some(skill_class) = self
                    .self_id
                    .as_ref()
                    .and_then(|id| self.players.get(id))
                    .map(|player| player.class_id)
                {
                    match skill_class {
                        ClassId::Guardian => "ESCUDO",
                        ClassId::Summoner => "ESPIRITU",
                        ClassId::Artificer => "TORRETA",
                        ClassId::Chronicler => "ARMONIA",
                        ClassId::Berserker => "FURIA",
                        _ => "CURAR",
                    }
                } else {
                    "CURAR"
                };
                format!("ELIGE: 1 GOLPE · 2 NOVA · 3 {}", skill_three)
            } else {
                format!("RONDA: {}", duel.round_winner)
            };
            self.fb
                .draw_text(panel_x + 10, panel_y + 34, &outcome, 1, 0xF8FAFC);
        }

        let narrow = w < 540;
        let uf_x = if narrow { 12 } else { 16 };
        let uf_y = if narrow { 50 } else { 16 };
        let uf_w = if narrow { (w - 128).clamp(172, 236) } else { 250 };
        let bar_inner_w = uf_w - 20;

        // Top-Left Self Unit Frame
        if let Some(self_p) = self.self_id.as_ref().and_then(|id| self.players.get(id)) {
            self.fb.fill_rect_alpha(uf_x, uf_y, uf_w, 118, 0x0B121E, 225);
            self.fb.stroke_rect(uf_x, uf_y, uf_w, 118, 0x334155);

            let cname = self_p.class_id.display_name();
            let zname = match zone_at(self.local_x, self.local_z) {
                ZoneId::Plaza => self.lang.tr("PLAZA SANTUARIO"),
                ZoneId::Woods => self.lang.tr("BOSQUE ESMERALDA"),
                ZoneId::Citadel => self.lang.tr("CIUDADELA"),
                ZoneId::Veins => self.lang.tr("VETAS CRISTAL"),
            };

            self.fb.draw_text(
                uf_x + 10,
                uf_y + 8,
                &format!("{} · LV.{}", self_p.name, self_p.level),
                1,
                0xFFFFFF,
            );
            self.fb.draw_text(
                uf_x + 10,
                uf_y + 21,
                &format!("{} · {}", cname, zname),
                1,
                0x94A3B8,
            );
            let rank = AdventurerRank::from_points(self_p.rank_points);
            let rank_progress = match rank.next_threshold_points() {
                Some(next_threshold) => format!(
                    "RANGO {} · {}/{}",
                    rank.as_code(),
                    self_p
                        .rank_points
                        .max(0)
                        .saturating_sub(rank.threshold_points()),
                    next_threshold - rank.threshold_points()
                ),
                None => format!("RANGO {} · MAX", rank.as_code()),
            };
            self.fb
                .draw_text(uf_x + 10, uf_y + 34, &rank_progress, 1, 0xF472B6);

            // HP bar
            let hp_w = ((self_p.hp.max(0) as f32 / self_p.max_hp.max(1) as f32)
                * bar_inner_w as f32) as i32;
            self.fb
                .fill_rect(uf_x + 10, uf_y + 49, bar_inner_w, 13, 0x1E293B);
            self.fb.fill_rect(uf_x + 10, uf_y + 49, hp_w, 13, 0x10B981);
            self.fb.draw_text(
                uf_x + 14,
                uf_y + 50,
                &format!("HP {}/{}", self_p.hp, self_p.max_hp),
                1,
                0xFFFFFF,
            );

            // MP bar
            let mp_w = ((self_p.mp.max(0) as f32 / self_p.max_mp.max(1) as f32)
                * bar_inner_w as f32) as i32;
            self.fb
                .fill_rect(uf_x + 10, uf_y + 66, bar_inner_w, 13, 0x1E293B);
            self.fb.fill_rect(uf_x + 10, uf_y + 66, mp_w, 13, 0x38BDF8);
            self.fb.draw_text(
                uf_x + 14,
                uf_y + 67,
                &format!("MP {}/{}", self_p.mp, self_p.max_mp),
                1,
                0xFFFFFF,
            );

            // XP + Gold
            let xp_w = ((self_p.xp.max(0) as f32 / self_p.xp_next.max(1) as f32)
                * bar_inner_w as f32) as i32;
            self.fb
                .fill_rect(uf_x + 10, uf_y + 84, bar_inner_w, 4, 0x1E293B);
            self.fb.fill_rect(uf_x + 10, uf_y + 84, xp_w, 4, 0xF59E0B);
            self.fb.draw_text(
                uf_x + 10,
                uf_y + 93,
                &format!("{} {}", self_p.gold, self.lang.tr("ORO")),
                1,
                0xFBBF24,
            );
            if self_p.shield_hp > 0 {
                self.fb.draw_text(
                    uf_x + 10,
                    uf_y + 104,
                    &format!("ESCUDO {}", self_p.shield_hp),
                    1,
                    0x67E8F9,
                );
            }
        }

        // Target Mob Frame
        if let Some(tid) = &self.target_mob_id {
            if let Some(m) = self.mobs.get(tid) {
                if m.alive {
                    let tf_y = uf_y + 124;
                    self.fb
                        .fill_rect_alpha(uf_x, tf_y, uf_w, 42, 0x0B121E, 225);
                    self.fb.stroke_rect(uf_x, tf_y, uf_w, 42, 0xEF4444);
                    self.fb.draw_text(
                        uf_x + 10,
                        tf_y + 7,
                        &format!("{} · LV.{}", m.name, m.level),
                        1,
                        0xFCA5A5,
                    );
                    let mhp_w = ((m.hp.max(0) as f32 / m.max_hp.max(1) as f32)
                        * bar_inner_w as f32) as i32;
                    self.fb
                        .fill_rect(uf_x + 10, tf_y + 24, bar_inner_w, 8, 0x1E293B);
                    self.fb.fill_rect(uf_x + 10, tf_y + 24, mhp_w, 8, 0xEF4444);
                }
            }
        }

        // Top-Right Mini-Radar
        let rx = if narrow { w - 108 } else { w - 114 };
        let ry = 52;
        self.fb.fill_rect_alpha(rx, ry, 98, 98, 0x0B121E, 225);
        self.fb.stroke_rect(rx, ry, 98, 98, 0x334155);
        let rcx = rx + 49;
        let rcy = ry + 49;
        let rscale = 44.0 / 90.0;
        for c in self.crystals.values() {
            if c.available {
                self.fb.fill_rect(
                    rcx + (c.x * rscale) as i32 - 1,
                    rcy + (c.z * rscale) as i32 - 1,
                    3,
                    3,
                    0x22D3EE,
                );
            }
        }
        for m in self.mobs.values() {
            if m.alive {
                let col = if m.kind == MobKind::Colossus {
                    0xF59E0B
                } else {
                    0xEF4444
                };
                self.fb.fill_rect(
                    rcx + (m.x * rscale) as i32 - 1,
                    rcy + (m.z * rscale) as i32 - 1,
                    3,
                    3,
                    col,
                );
            }
        }
        for (id, p) in &self.players {
            let is_self = self.self_id.as_ref() == Some(id);
            let (px, pz) = if is_self {
                (self.local_x, self.local_z)
            } else {
                (p.x, p.z)
            };
            self.fb.fill_rect(
                rcx + (px * rscale) as i32 - 2,
                rcy + (pz * rscale) as i32 - 2,
                4,
                4,
                if is_self { 0xFFFFFF } else { 0x10B981 },
            );
        }
        if let Some((marker_x, marker_z)) = self.personal_marker {
            let mx = rcx + (marker_x * rscale) as i32;
            let mz = rcy + (marker_z * rscale) as i32;
            self.fb.fill_rect(mx - 3, mz - 3, 7, 7, 0xF472B6);
            self.fb.fill_rect(mx - 1, mz - 1, 3, 3, 0xFFFFFF);
        }
        self.fb
            .draw_text(rx + 3, ry + 101, "M MARCA  B BEST", 1, 0x94A3B8);

        if self.bestiary_open {
            let panel_w = (w - 32).min(222);
            let panel_x = (w - panel_w) / 2;
            let panel_y = 56;
            self.fb
                .fill_rect_alpha(panel_x, panel_y, panel_w, 106, 0x0B121E, 240);
            self.fb
                .stroke_rect(panel_x, panel_y, panel_w, 106, 0xF472B6);
            self.fb.draw_text(
                panel_x + 8,
                panel_y + 8,
                "BESTIARIO · B CIERRA",
                1,
                0xF8FAFC,
            );
            let entries = [
                ("RIFT STALKER", 0xF87171),
                ("CRYSTAL WISP", 0x38BDF8),
                ("OBSIDIAN SENTINEL", 0xA78BFA),
                ("ANCIENT COLOSSUS", 0xF59E0B),
            ];
            for (index, (name, color)) in entries.iter().enumerate() {
                let discovered = self.bestiary_known[index];
                let label = if discovered {
                    *name
                } else {
                    "??? · NO DESCUBIERTO"
                };
                self.fb.draw_text(
                    panel_x + 8,
                    panel_y + 25 + index as i32 * 17,
                    label,
                    1,
                    if discovered { *color } else { 0x64748B },
                );
            }
        }

        // Bottom-Left Live World Chat (or Mobile Chat Overlay when focused)
        let compact_hud = w < 680 || self.touch_mode;
        if !compact_hud {
            self.fb
                .fill_rect_alpha(16, h - 146, 274, 130, 0x0B121E, 215);
            self.fb.stroke_rect(16, h - 146, 274, 130, 0x334155);
            self.fb
                .draw_text(24, h - 138, self.lang.tr("CHAT"), 1, 0x38BDF8);
            let start_idx = self.chat.len().saturating_sub(5);
            for (row, msg) in self.chat[start_idx..].iter().enumerate() {
                let y_line = h - 122 + (row as i32) * 14;
                let line = format!("{}: {}", msg.sender_name, msg.text);
                let clipped: String = line.chars().take(38).collect();
                self.fb.draw_text(24, y_line, &clipped, 1, 0xE2E8F0);
            }
            self.fb.fill_rect(22, h - 44, 262, 22, 0x111827);
            self.fb.stroke_rect(
                22,
                h - 44,
                262,
                22,
                if self.chat_focused {
                    0x38BDF8
                } else {
                    0x475569
                },
            );
            let chat_disp = if self.chat_focused {
                format!("{}_", self.chat_input)
            } else if self.chat_input.is_empty() {
                String::from("ENTER...")
            } else {
                self.chat_input.clone()
            };
            let chat_clipped: String = chat_disp.chars().take(36).collect();
            self.fb.draw_text(
                28,
                h - 38,
                &chat_clipped,
                1,
                if self.chat_focused {
                    0xFFFFFF
                } else {
                    0x64748B
                },
            );
        } else if self.chat_focused {
            let cw = (w - 24).min(300);
            let cx = (w - cw) / 2;
            let cy = 54;
            self.fb.rounded_panel(cx, cy, cw, 122, 0x0B121E, 240);
            self.fb.panel_border(cx, cy, cw, 122, 0x38BDF8);
            self.fb
                .draw_text(cx + 10, cy + 8, self.lang.tr("CHAT"), 1, 0x38BDF8);
            let start_idx = self.chat.len().saturating_sub(4);
            for (row, msg) in self.chat[start_idx..].iter().enumerate() {
                let y_line = cy + 24 + (row as i32) * 15;
                let line = format!("{}: {}", msg.sender_name, msg.text);
                let clipped: String = line.chars().take(38).collect();
                self.fb.draw_text(cx + 10, y_line, &clipped, 1, 0xE2E8F0);
            }
            self.fb.fill_rect(cx + 8, cy + 90, cw - 16, 24, 0x111827);
            self.fb.stroke_rect(cx + 8, cy + 90, cw - 16, 24, 0x38BDF8);
            let chat_disp = format!("{}_", self.chat_input);
            let chat_clipped: String = chat_disp.chars().take(36).collect();
            self.fb
                .draw_text(cx + 14, cy + 96, &chat_clipped, 1, 0xFFFFFF);
        }

        // Bottom-Center Action Bar (1, 2, 3, E)
        let compact_actions = w < 680;
        let bar_w = if compact_actions { 208 } else { 280 };
        let bar_x = (w - bar_w) / 2;
        let bar_y = h - 68;
        self.fb
            .fill_rect_alpha(bar_x, bar_y, bar_w, 52, 0x0B121E, 225);
        self.fb.stroke_rect(bar_x, bar_y, bar_w, 52, 0x334155);
        let actions = [
            ("1", self.lang.tr("GOLPE"), self.cooldowns[0], 0xF59E0B),
            ("2", self.lang.tr("NOVA"), self.cooldowns[1], 0x38BDF8),
            (
                "3",
                self.self_id
                    .as_ref()
                    .and_then(|id| self.players.get(id))
                    .map(|player| match player.class_id {
                        ClassId::Guardian => "ESCUDO",
                        ClassId::Summoner => "ESPIRITU",
                        ClassId::Artificer => "TORRETA",
                        ClassId::Chronicler => "ARMONIA",
                        ClassId::Berserker => "FURIA",
                        _ => self.lang.tr("CURAR"),
                    })
                    .unwrap_or(self.lang.tr("CURAR")),
                self.cooldowns[2],
                0x10B981,
            ),
            (
                "E",
                self.lang.tr("MINAR"),
                if self.gathering_id.is_some() {
                    self.gathering_progress
                } else {
                    0.0
                },
                0x22D3EE,
            ),
        ];
        for (idx, (key, lbl, cd, col)) in actions.iter().enumerate() {
            let ax = if compact_actions {
                bar_x + 6 + (idx as i32) * 50
            } else {
                bar_x + 6 + (idx as i32) * 68
            };
            let ay = bar_y + 6;
            let slot_w = if compact_actions { 44 } else { 62 };
            self.fb.fill_rect(ax, ay, slot_w, 40, 0x111827);
            self.fb.stroke_rect(ax, ay, slot_w, 40, *col);
            self.fb.draw_text(ax + 5, ay + 4, key, 1, 0x94A3B8);
            if compact_actions {
                let short_lbl: String = lbl.chars().take(4).collect();
                self.fb.draw_text(ax + 5, ay + 21, &short_lbl, 1, 0xF8FAFC);
            } else {
                self.fb.draw_text(ax + 5, ay + 21, lbl, 1, 0xF8FAFC);
            }
            if *cd > 0.05 {
                self.fb.fill_rect_alpha(ax, ay, slot_w, 40, 0x020617, 180);
                self.fb
                    .draw_text(ax + 14, ay + 14, &format!("{:.1}", cd), 1, 0xFFFFFF);
            }
        }

        // Virtual Touch Joystick + Action Buttons (Mobile / Tablet / Touchscreens)
        if self.touch_mode {
            let jx = if self.input.joy_origin_x > 0 {
                self.input.joy_origin_x
            } else {
                74
            };
            let jy = if self.input.joy_origin_y > 0 {
                self.input.joy_origin_y
            } else {
                (h - 170).max(150)
            };
            self.fb
                .rounded_panel(jx - 48, jy - 48, 96, 96, 0x0B121E, 165);
            self.fb.panel_border(jx - 48, jy - 48, 96, 96, 0x38BDF8);
            let kx = jx + (self.input.joy_x * 32.0) as i32;
            let ky = jy + (self.input.joy_y * 32.0) as i32;
            self.fb.fill_rect(kx - 15, ky - 15, 30, 30, 0x38BDF8);

            // Jump button on right side
            let jbx = w - 88;
            let jby = h - 134;
            self.fb.rounded_panel(jbx, jby, 72, 46, 0x0B121E, 210);
            self.fb.panel_border(jbx, jby, 72, 46, 0xF59E0B);
            let jlbl = self.lang.tr("SALTAR");
            let jw = self.fb.measure_text(jlbl, 1);
            self.fb
                .draw_text(jbx + (72 - jw) / 2, jby + 16, jlbl, 1, 0xF8FAFC);

            // Dodge button next to jump.
            let dbx = w - 160;
            let dby = h - 134;
            self.fb.rounded_panel(dbx, dby, 64, 46, 0x0B121E, 210);
            self.fb.panel_border(
                dbx,
                dby,
                64,
                46,
                if self.dodge_cooldown > 0.0 {
                    0x475569
                } else {
                    0xF472B6
                },
            );
            let dlbl = if self.dodge_cooldown > 0.0 {
                format!("{:.1}", self.dodge_cooldown)
            } else {
                String::from("ESQUIVA")
            };
            let dw = self.fb.measure_text(&dlbl, 1);
            self.fb
                .draw_text(dbx + (64 - dw) / 2, dby + 16, &dlbl, 1, 0xF8FAFC);

            // Compact chat button that stays clear of movement and jump controls.
            let cbx = w - 88;
            let cby = h - 188;
            self.fb.rounded_panel(cbx, cby, 72, 46, 0x0B121E, 210);
            self.fb.panel_border(
                cbx,
                cby,
                72,
                46,
                if self.chat_focused {
                    0x38BDF8
                } else {
                    0x475569
                },
            );
            let clbl = self.lang.tr("CHAT");
            let cw = self.fb.measure_text(clbl, 1);
            self.fb
                .draw_text(cbx + (72 - cw) / 2, cby + 16, clbl, 1, 0xF8FAFC);

            // Touch equivalents for M (personal marker), B (bestiary),
            // and the emote shortcuts that otherwise require a keyboard.
            let utility = [
                (h - 350, "EMO", 0xA78BFA),
                (h - 296, "MARCA", 0xF472B6),
                (h - 242, "BEST", 0x38BDF8),
            ];
            for (uby, label, color) in utility {
                self.fb.rounded_panel(w - 88, uby, 72, 46, 0x0B121E, 210);
                self.fb.panel_border(w - 88, uby, 72, 46, color);
                let lw = self.fb.measure_text(label, 1);
                self.fb
                    .draw_text(w - 88 + (72 - lw) / 2, uby + 16, label, 1, 0xF8FAFC);
            }

            if self.touch_emote_menu {
                let names = ["OLA", "FIESTA", "RISA", "BAILE"];
                let slot_w = 58;
                let panel_x = w - 88 - 4 * slot_w - 12;
                let panel_y = h - 350;
                self.fb.fill_rect_alpha(
                    panel_x - 6,
                    panel_y - 6,
                    4 * slot_w + 12,
                    58,
                    0x0B121E,
                    235,
                );
                self.fb
                    .stroke_rect(panel_x - 6, panel_y - 6, 4 * slot_w + 12, 58, 0xA78BFA);
                for (idx, name) in names.iter().enumerate() {
                    let bx = panel_x + idx as i32 * slot_w;
                    self.fb.fill_rect(bx, panel_y, slot_w - 4, 46, 0x111827);
                    self.fb.stroke_rect(bx, panel_y, slot_w - 4, 46, 0x475569);
                    let lw = self.fb.measure_text(name, 1);
                    self.fb
                        .draw_text(bx + (slot_w - 4 - lw) / 2, panel_y + 16, name, 1, 0xF8FAFC);
                }
            }
        }
    }
}
