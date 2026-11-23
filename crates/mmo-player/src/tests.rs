#[cfg(test)]
mod tests_inner {
    use crate::{profile::LocalProfile, ClassId};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn local_profile_persists_offline_progress_and_ignores_invalid_input() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("wfo-profile-{}-{nonce}.txt", std::process::id()));
        let mut profile = LocalProfile::load(Some(path.clone()));
        profile.save("Player_01", ClassId::Guardian, 4, 17, 200, 39, 123);
        assert_eq!(
            fs::read_to_string(&path).expect("read profile"),
            "Player_01\nguardian\n4\n200\n17\n39\n123"
        );

        let mut loaded = LocalProfile::load(Some(path.clone()));
        assert_eq!(loaded.username, "Player_01");
        assert_eq!(loaded.class_id, ClassId::Guardian);
        assert_eq!(
            (loaded.level, loaded.xp, loaded.xp_next, loaded.gold),
            (4, 17, 200, 39)
        );
        assert_eq!(loaded.rank_points, 123);
        loaded.save("not valid", ClassId::Ranger, 1, 0, 100, 0, 0);
        assert_eq!(
            fs::read_to_string(&path).expect("profile remains unchanged"),
            "Player_01\nguardian\n4\n200\n17\n39\n123"
        );
        fs::write(&path, "Legacy_User").expect("write previous profile format");
        let legacy = LocalProfile::load(Some(path.clone()));
        assert_eq!(legacy.username, "Legacy_User");
        assert_eq!(legacy.class_id, ClassId::Vanguard);
        assert_eq!(
            (legacy.level, legacy.xp, legacy.xp_next, legacy.gold),
            (1, 0, 100, 0)
        );
        assert_eq!(legacy.rank_points, 0);
        std::fs::remove_file(path).expect("remove test profile");
    }
}
