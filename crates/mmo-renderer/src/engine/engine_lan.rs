use super::*;

impl MmoClientEngine {
    pub fn apply_lan_packet(&mut self, pkt: LanPacket) {
        match pkt {
            LanPacket::Announce {
                peer_id,
                name,
                udp_port,
                address,
            } => {
                if let Some(peer) = self.lan_peers.iter_mut().find(|p| p.id == peer_id) {
                    peer.name = name;
                    peer.address = address;
                    peer.udp_port = udp_port;
                } else {
                    self.lan_peers.push(LanPeer {
                        id: peer_id,
                        name,
                        address,
                        udp_port,
                    });
                }
            }
            LanPacket::Challenge { from_id, from_name } => {
                self.lan_duel_opponent = Some(from_id.clone());
                self.lan_duel_wins = 0;
                self.lan_duel_opponent_wins = 0;
                self.lan_duel_round = 1;
                self.lan_pending_skill = None;
                self.lan_remote_skill = None;
                self.lan_remote_skill = None;
                self.lan_outbox.push(LanPacket::Accept {
                    from_id: self
                        .self_id
                        .clone()
                        .unwrap_or_else(|| String::from("lan_local")),
                });
                let _ = from_name;
            }
            LanPacket::Accept { from_id } => {
                self.lan_duel_opponent = Some(from_id);
                self.lan_duel_wins = 0;
                self.lan_duel_opponent_wins = 0;
                self.lan_duel_round = 1;
                self.lan_pending_skill = None;
                self.lan_remote_skill = None;
            }
            LanPacket::Action {
                from_id,
                round,
                skill,
            } => {
                if self.lan_duel_opponent.as_deref() != Some(from_id.as_str()) {
                    return;
                }
                if round != self.lan_duel_round {
                    return;
                }
                self.lan_remote_skill = Some(skill);
                if let (Some(own_skill), Some(opponent_skill)) =
                    (self.lan_pending_skill, self.lan_remote_skill)
                {
                    if own_skill == opponent_skill {
                    } else if (own_skill == 0 && opponent_skill == 2)
                        || (own_skill == 1 && opponent_skill == 0)
                        || (own_skill == 2 && opponent_skill == 1)
                    {
                        self.lan_duel_wins += 1;
                    } else {
                        self.lan_duel_opponent_wins += 1;
                    }
                    self.lan_pending_skill = None;
                    self.lan_remote_skill = None;
                    if self.lan_duel_wins < 3 && self.lan_duel_opponent_wins < 3 {
                        self.lan_duel_round = (self.lan_duel_round + 1).min(5);
                    }
                }
            }
            LanPacket::Surrender { from_id } => {
                if self.lan_duel_opponent.as_deref() == Some(from_id.as_str()) {
                    self.lan_duel_opponent = None;
                    self.lan_pending_skill = None;
                    self.lan_remote_skill = None;
                }
            }
        }
    }

    pub fn challenge_lan_peer(&mut self, peer_id: &str) {
        let peer = self.lan_peers.iter().find(|p| p.id == peer_id);
        if peer.is_none() {
            return;
        }
        let from_id = self
            .self_id
            .clone()
            .unwrap_or_else(|| String::from("lan_local"));
        self.lan_duel_opponent = Some(peer_id.to_string());
        self.lan_duel_wins = 0;
        self.lan_duel_opponent_wins = 0;
        self.lan_duel_round = 1;
        self.lan_pending_skill = None;
        self.lan_outbox.push(LanPacket::Challenge {
            from_id,
            from_name: self.lobby_name.clone(),
        });
    }

    pub fn trigger_lan_skill(&mut self, skill: u8) {
        if self.lan_duel_opponent.is_some() {
            if self.lan_pending_skill.is_none() {
                self.lan_pending_skill = Some(skill);
                self.lan_outbox.push(LanPacket::Action {
                    from_id: self
                        .self_id
                        .clone()
                        .unwrap_or_else(|| String::from("lan_local")),
                    round: self.lan_duel_round,
                    skill,
                });
            }
        }
    }

    pub fn toggle_lan_mode(&mut self) {
        self.lan_mode = !self.lan_mode;
    }
}
