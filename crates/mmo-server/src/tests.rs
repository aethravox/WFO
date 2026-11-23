#[cfg(test)]
mod tests_inner {
    use crate::network::register_player;
    use crate::*;
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::{Duration, Instant};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn movement_budget_moves_from_server_direction_not_client_position() {
        let start = Instant::now();
        let mut budget = MovementBudget::new(start);

        let first = budget.move_authoritatively(
            0.0,
            0.0,
            1.0,
            1.0,
            10.0,
            start + Duration::from_millis(100),
        );
        assert!((first.0 - 0.70710677).abs() < 0.001);
        assert!((first.1 - 0.70710677).abs() < 0.001);

        let repeated = budget.move_authoritatively(
            first.0,
            first.1,
            100.0,
            0.0,
            10.0,
            start + Duration::from_millis(150),
        );
        assert!((repeated.0 - 1.2071068).abs() < 0.001);
        assert!((repeated.1 - 0.70710677).abs() < 0.001);
    }

    #[test]
    fn lobby_invite_ids_are_unpredictable_bearer_tokens() {
        let ids = (0..64)
            .map(|_| generate_room_id().expect("generate lobby token"))
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(ids.len(), 64);
        assert!(ids.iter().all(|id| id.len() <= 24
            && id.starts_with("room_")
            && id[5..].bytes().all(|b| b.is_ascii_hexdigit())));
    }

    #[test]
    fn melee_target_must_be_in_front_of_server_facing() {
        assert!(target_is_in_front(0.0, 0.0, 0.0, 0.0, 5.0));
        assert!(!target_is_in_front(0.0, 0.0, 0.0, 0.0, -5.0));
    }

    #[test]
    fn online_rewards_promote_adventurer_rank_and_notify_the_player() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "wfo-rank-rewards-{}-{nonce}.sqlite3",
            std::process::id()
        ));
        let accounts = Arc::new(AccountStore::open(&path).expect("open test database"));
        let state = Arc::new(Mutex::new(WorldState::new(Arc::clone(&accounts))));
        let (sink, receiver) = mpsc::channel();
        let player_id = register_player(&state, ClientSink(sink));

        {
            let mut world = state.lock().expect("lock world");
            let player = world
                .players
                .get_mut(&player_id)
                .expect("registered player");
            player.joined = true;
            player.snap.rank_points = 90;

            world.grant_rewards(&player_id, 20, 0);

            assert_eq!(world.players[&player_id].snap.rank_points, 110);
        }

        assert!(receiver.try_iter().any(|packet| matches!(
            packet,
            ServerPacket::RankUp {
                id,
                rank: AdventurerRank::G
            } if id == player_id
        )));
        drop(state);
        drop(accounts);
        std::fs::remove_file(path).expect("remove test database");
    }

    #[test]
    fn guardian_skill_shields_nearby_allies_and_absorbs_damage() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "wfo-guardian-shield-{}-{nonce}.sqlite3",
            std::process::id()
        ));
        let accounts = Arc::new(AccountStore::open(&path).expect("open test database"));
        let state = Arc::new(Mutex::new(WorldState::new(Arc::clone(&accounts))));
        let (guardian_tx, _guardian_rx) = mpsc::channel();
        let guardian_id = register_player(&state, ClientSink(guardian_tx));
        let (ally_tx, _ally_rx) = mpsc::channel();
        let ally_id = register_player(&state, ClientSink(ally_tx));

        {
            let mut world = state.lock().expect("lock world");
            let guardian = world
                .players
                .get_mut(&guardian_id)
                .expect("registered guardian");
            guardian.authenticated = true;
            guardian.joined = true;
            guardian.snap.class_id = ClassId::Guardian;
            guardian.snap.max_hp = ClassId::Guardian.base_hp();
            guardian.snap.hp = guardian.snap.max_hp;
            guardian.snap.mp = guardian.snap.max_mp;

            let ally = world.players.get_mut(&ally_id).expect("registered ally");
            ally.authenticated = true;
            ally.joined = true;

            world.handle_packet(
                &guardian_id,
                ClientPacket::Attack {
                    skill: 2,
                    target_id: String::new(),
                    yaw: 0.0,
                },
            );

            assert_eq!(world.players[&guardian_id].snap.shield_hp, 57);
            assert_eq!(world.players[&ally_id].snap.shield_hp, 48);
            let now = Instant::now();
            let ally = world.players.get_mut(&ally_id).expect("registered ally");
            assert_eq!(ally.absorb_shield(20, now), 0);
            assert_eq!(ally.snap.shield_hp, 28);
            assert_eq!(ally.absorb_shield(10, now + Duration::from_secs(9)), 10);
            assert_eq!(ally.snap.shield_hp, 0);
        }

        drop(state);
        drop(accounts);
        std::fs::remove_file(path).expect("remove test database");
    }

    #[test]
    fn world_player_cannot_reenter_lobby_or_use_legacy_join() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "wfo-transitions-{}-{nonce}.sqlite3",
            std::process::id()
        ));
        let accounts = Arc::new(AccountStore::open(&path).expect("open test database"));
        let state = Arc::new(Mutex::new(WorldState::new(Arc::clone(&accounts))));
        let (sink, _rx) = mpsc::channel();
        let player_id = register_player(&state, ClientSink(sink));

        {
            let mut world = state.lock().expect("lock world");
            let player = world
                .players
                .get_mut(&player_id)
                .expect("registered player");
            player.authenticated = true;
            player.joined = true;
            player.snap.x = 12.0;
            player.snap.z = -7.0;
            player.snap.class_id = ClassId::Ranger;
            player.snap.hp = 80;

            world.handle_packet(
                &player_id,
                ClientPacket::EnterLobby {
                    name: String::from("Traveler"),
                },
            );
            world.handle_packet(
                &player_id,
                ClientPacket::Join {
                    name: String::from("Other"),
                    class_id: ClassId::Arcanist,
                },
            );

            let player = &world.players[&player_id];
            assert!(player.joined);
            assert!(!player.in_lobby);
            assert_eq!((player.snap.x, player.snap.z), (12.0, -7.0));
            assert_eq!(player.snap.class_id, ClassId::Ranger);
            assert_eq!(player.snap.hp, 80);
        }

        drop(state);
        drop(accounts);
        std::fs::remove_file(path).expect("remove test database");
    }

    #[test]
    fn account_cannot_be_authenticated_by_two_players() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "wfo-sessions-{}-{nonce}.sqlite3",
            std::process::id()
        ));
        let accounts = Arc::new(AccountStore::open(&path).expect("open test database"));
        let account = accounts
            .authenticate("Session_User", "correct horse battery", true)
            .expect("register test account");
        let state = Arc::new(Mutex::new(WorldState::new(Arc::clone(&accounts))));
        let (first_tx, _first_rx) = mpsc::channel();
        let first = register_player(&state, ClientSink(first_tx));
        let (second_tx, second_rx) = mpsc::channel();
        let second = register_player(&state, ClientSink(second_tx));

        {
            let mut world = state.lock().expect("lock world");
            assert!(world.complete_auth(&first, Ok(account.clone())));
            assert!(world.complete_auth(&second, Ok(account.clone())));
            assert!(world.players[&first].authenticated);
            assert!(!world.players[&second].authenticated);
        }
        let response = second_rx.try_iter().last().expect("authentication failure");
        assert!(matches!(response, ServerPacket::AuthFailure { .. }));

        drop(accounts);
        std::fs::remove_file(path).expect("remove test database");
    }
}
