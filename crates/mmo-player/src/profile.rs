use super::*;

pub(crate) struct LocalProfile {
    pub(crate) path: Option<PathBuf>,
    pub(crate) username: String,
    pub(crate) class_id: ClassId,
    pub(crate) level: i32,
    pub(crate) xp: i32,
    pub(crate) xp_next: i32,
    pub(crate) gold: i32,
    pub(crate) rank_points: i32,
}

impl LocalProfile {
    pub(crate) fn load(path: Option<PathBuf>) -> Self {
        let contents = path
            .as_ref()
            .and_then(|path| match fs::read_to_string(path) {
                Ok(contents) => Some(contents),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => {
                    eprintln!("[wfo-player] Could not read local profile: {error}");
                    None
                }
            })
            .unwrap_or_default();
        let mut lines = contents.lines();
        let username = lines.next().unwrap_or("").trim();
        let username = if valid_local_username(username) {
            username.to_string()
        } else {
            String::new()
        };
        let class_id = lines
            .next()
            .map(ClassId::from_str_code)
            .unwrap_or(ClassId::Vanguard);
        let level = lines
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|level| (1..=10_000).contains(level))
            .unwrap_or(1);
        let xp_next = lines
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|xp_next| *xp_next > 0)
            .unwrap_or(100);
        let xp = lines
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|xp| (0..xp_next).contains(xp))
            .unwrap_or(0);
        let gold = lines
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|gold| *gold >= 0)
            .unwrap_or(0);
        let rank_points = lines
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|points| *points >= 0)
            .unwrap_or(0);
        Self {
            path,
            username,
            class_id,
            level,
            xp,
            xp_next,
            gold,
            rank_points,
        }
    }

    pub(crate) fn save(
        &mut self,
        username: &str,
        class_id: ClassId,
        level: i32,
        xp: i32,
        xp_next: i32,
        gold: i32,
        rank_points: i32,
    ) {
        let username = username.trim();
        if (!username.is_empty() && !valid_local_username(username))
            || (self.username == username
                && self.class_id == class_id
                && self.level == level
                && self.xp == xp
                && self.xp_next == xp_next
                && self.gold == gold
                && self.rank_points == rank_points)
        {
            return;
        }
        self.username = username.to_string();
        self.class_id = class_id;
        self.level = level.clamp(1, 10_000);
        self.xp_next = xp_next.max(1);
        self.xp = xp.clamp(0, self.xp_next - 1);
        self.gold = gold.max(0);
        self.rank_points = rank_points.max(0);
        if let Some(path) = self.path.as_ref() {
            let result = (|| {
                if let Some(parent) = path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                {
                    fs::create_dir_all(parent)?;
                }
                fs::write(
                    path,
                    format!(
                        "{}\n{}\n{}\n{}\n{}\n{}\n{}",
                        self.username,
                        class_id.as_code(),
                        self.level,
                        self.xp_next,
                        self.xp,
                        self.gold,
                        self.rank_points
                    ),
                )
            })();
            if let Err(error) = result {
                eprintln!("[wfo-player] Could not save local profile: {error}");
            }
        }
    }
}

pub(crate) fn valid_local_username(name: &str) -> bool {
    (3..=18).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

pub(crate) fn local_profile_path(_android_data_dir: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("WFO_PROFILE_PATH") {
        return Some(PathBuf::from(path));
    }
    #[cfg(target_os = "android")]
    {
        return _android_data_dir.map(|dir| dir.join("wfo-profile.txt"));
    }
    #[cfg(not(target_os = "android"))]
    {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
        base.map(|dir| dir.join("WFO").join("profile.txt"))
    }
}
