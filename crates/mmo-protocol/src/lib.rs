#![no_std]
extern crate alloc;

pub const PLAYABLE_RADIUS: f32 = 84.0;
pub const SAFE_PLAZA_RADIUS: f32 = 16.0;
pub const FOREST_TREE_COUNT: usize = 56;
pub const TREE_COLLISION_RADIUS: f32 = 0.7;
pub const GUILD_POS_X: f32 = -10.5;
pub const GUILD_POS_Z: f32 = -5.5;

mod language;
mod math;
mod packets;
mod types;
pub use language::*;
pub use math::*;
pub use packets::*;
pub use types::*;
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

    #[test]
    fn adventurer_ranks_advance_at_their_documented_thresholds() {
        assert_eq!(AdventurerRank::from_points(-1), AdventurerRank::H);
        assert_eq!(AdventurerRank::from_points(99), AdventurerRank::H);
        assert_eq!(AdventurerRank::from_points(100), AdventurerRank::G);
        assert_eq!(AdventurerRank::from_points(9_999), AdventurerRank::SS);
        assert_eq!(AdventurerRank::from_points(10_000), AdventurerRank::SSS);
        assert_eq!(AdventurerRank::SSS.next_threshold_points(), None);
        assert_eq!(AdventurerRank::from_code("SSS"), Some(AdventurerRank::SSS));
        assert_eq!(AdventurerRank::from_code("X"), None);
    }

    #[test]
    fn emote_packets_accept_only_known_emote_codes() {
        assert!(matches!(
            ClientPacket::decode(
                &ClientPacket::Emote {
                    emote: EmoteId::Wave
                }
                .encode()
            ),
            Some(ClientPacket::Emote {
                emote: EmoteId::Wave
            })
        ));
        assert!(matches!(
            ServerPacket::decode(
                &ServerPacket::Emote {
                    player_id: String::from("hero_1"),
                    emote: EmoteId::Dance,
                }
                .encode()
            ),
            Some(ServerPacket::Emote {
                player_id,
                emote: EmoteId::Dance,
            }) if player_id == "hero_1"
        ));
        assert!(ClientPacket::decode("EMOTE|unknown").is_none());
    }

    #[test]
    fn player_packets_preserve_rank_points_and_decode_legacy_snapshots() {
        let player = PlayerSnapshot {
            id: String::from("hero_1"),
            name: String::from("Traveler"),
            class_id: ClassId::Vanguard,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            moving: false,
            hp: 100,
            max_hp: 100,
            mp: 70,
            max_mp: 70,
            level: 1,
            xp: 0,
            xp_next: 100,
            gold: 0,
            rank_points: 321,
            attack_tick: 12,
            shield_hp: 27,
            haste: true,
            has_adventurer_card: true,
            adventurer_rank: AdventurerRank::E,
        };
        assert!(matches!(
            ServerPacket::decode(&ServerPacket::Player(player).encode()),
            Some(ServerPacket::Player(PlayerSnapshot {
                rank_points: 321,
                attack_tick: 12,
                shield_hp: 27,
                haste: true,
                has_adventurer_card: true,
                adventurer_rank: AdventurerRank::E,
                ..
            }))
        ));

        assert!(matches!(
            ServerPacket::decode(
                "PLR|hero_1|Traveler|vanguard|0|0|0|0|0|100|100|70|70|1|0|100|0|12"
            ),
            Some(ServerPacket::Player(PlayerSnapshot {
                rank_points: 0,
                attack_tick: 12,
                shield_hp: 0,
                haste: false,
                has_adventurer_card: false,
                adventurer_rank: AdventurerRank::H,
                ..
            }))
        ));
    }

    #[test]
    fn guardian_class_code_and_stats_are_available() {
        assert_eq!(ClassId::from_str_code("guardian"), ClassId::Guardian);
        assert_eq!(ClassId::from_str_code("3"), ClassId::Guardian);
        assert_eq!(ClassId::Guardian.as_code(), "guardian");
        assert!(ClassId::Guardian.base_hp() > ClassId::Vanguard.base_hp());
        assert!(ClassId::Guardian.base_damage() < ClassId::Vanguard.base_damage());
    }

    #[test]
    fn new_anime_classes_round_trip_and_have_distinct_stats() {
        let classes = [
            ("duelist", ClassId::Duelist),
            ("summoner", ClassId::Summoner),
            ("artificer", ClassId::Artificer),
            ("chronicler", ClassId::Chronicler),
            ("berserker", ClassId::Berserker),
        ];
        for (code, class_id) in classes {
            assert_eq!(ClassId::from_str_code(code), class_id);
            assert_eq!(ClassId::from_str_code(class_id.as_code()), class_id);
        }
        assert!(ClassId::Duelist.move_speed() > ClassId::Ranger.move_speed());
        assert!(ClassId::Berserker.base_damage() > ClassId::Vanguard.base_damage());
        assert!(ClassId::Chronicler.base_mp() > ClassId::Arcanist.base_mp());
    }

    #[test]
    fn rank_up_packets_round_trip() {
        assert!(matches!(
            ServerPacket::decode(
                &ServerPacket::RankUp {
                    id: String::from("hero_1"),
                    rank: AdventurerRank::SSS,
                }
                .encode()
            ),
            Some(ServerPacket::RankUp {
                id,
                rank: AdventurerRank::SSS,
            }) if id == "hero_1"
        ));
    }

    #[test]
    fn tree_collision_stops_swept_movement_at_the_trunk() {
        let (tree_x, tree_z) = forest_tree_position(0).expect("first forest tree");
        let (x, z) = move_with_tree_collision(tree_x - 3.0, tree_z, tree_x + 3.0, tree_z);

        assert!(x < tree_x);
        assert!(((x - tree_x).powi(2) + (z - tree_z).powi(2)).sqrt() >= TREE_COLLISION_RADIUS);
    }

    #[test]
    fn tree_collision_sweeps_only_the_shared_visible_trees() {
        let visible_tree_count = (0..FOREST_TREE_COUNT)
            .filter(|&index| forest_tree_position(index).is_some())
            .count();
        assert!(visible_tree_count > 0 && visible_tree_count < FOREST_TREE_COUNT);
        assert!(forest_tree_position(FOREST_TREE_COUNT).is_none());
    }

    #[test]
    fn account_auth_packets_round_trip_without_exposing_other_fields() {
        let packet = ClientPacket::RegisterAccount {
            username: String::from("Player_01"),
            password: String::from("correct horse battery"),
        };
        let encoded = packet.encode();
        assert!(matches!(
            ClientPacket::decode(&encoded),
            Some(ClientPacket::RegisterAccount { username, password })
                if username == "Player_01" && password == "correct horse battery"
        ));

        let response = ServerPacket::AuthSuccess {
            username: String::from("player_01"),
            class_id: ClassId::Arcanist,
            level: 4,
            xp: 25,
            xp_next: 300,
            gold: 90,
        };
        assert!(matches!(
            ServerPacket::decode(&response.encode()),
            Some(ServerPacket::AuthSuccess {
                username,
                class_id: ClassId::Arcanist,
                level: 4,
                xp: 25,
                xp_next: 300,
                gold: 90,
            }) if username == "player_01"
        ));
    }

    #[test]
    fn guild_packets_round_trip() {
        assert!(matches!(
            ClientPacket::decode(&ClientPacket::GuildInteract.encode()),
            Some(ClientPacket::GuildInteract)
        ));
        assert!(matches!(
            ServerPacket::decode(
                &ServerPacket::GuildNotice {
                    kind: 2,
                    rank: AdventurerRank::G,
                    missing: 42,
                }
                .encode()
            ),
            Some(ServerPacket::GuildNotice {
                kind: 2,
                rank: AdventurerRank::G,
                missing: 42,
            })
        ));
    }

    #[test]
    fn server_can_report_secure_connection_state() {
        assert!(matches!(
            ServerPacket::decode(&ServerPacket::ConnectionStatus { connected: false }.encode()),
            Some(ServerPacket::ConnectionStatus { connected: false })
        ));
    }
}
