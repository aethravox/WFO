use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use mmo_protocol::ClassId;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

pub struct AccountStore {
    connection: Mutex<Connection>,
}

#[derive(Clone, Debug)]
pub struct Account {
    pub id: i64,
    pub username: String,
    pub class_id: ClassId,
    pub level: i32,
    pub xp: i32,
    pub xp_next: i32,
    pub gold: i32,
    pub rank_points: i32,
}

#[derive(Debug)]
pub enum AuthError {
    InvalidInput,
    InvalidCredentials,
    RateLimited,
    RegistrationFailed,
    Storage(String),
}

impl AccountStore {
    pub fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS accounts (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL COLLATE NOCASE UNIQUE,
                password_hash TEXT NOT NULL,
                class_id TEXT NOT NULL DEFAULT 'vanguard',
                level INTEGER NOT NULL DEFAULT 1,
                xp INTEGER NOT NULL DEFAULT 0,
                xp_next INTEGER NOT NULL DEFAULT 100,
                gold INTEGER NOT NULL DEFAULT 0,
                rank_points INTEGER NOT NULL DEFAULT 0
            );",
        )?;
        let has_rank_points = {
            let mut statement = connection.prepare("PRAGMA table_info(accounts)")?;
            let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
            columns
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .any(|column| column == "rank_points")
        };
        if !has_rank_points {
            connection.execute(
                "ALTER TABLE accounts ADD COLUMN rank_points INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn authenticate(
        &self,
        username: &str,
        password: &str,
        register: bool,
    ) -> Result<Account, AuthError> {
        if !valid_username(username) || !valid_password(password) {
            return Err(AuthError::InvalidInput);
        }
        let username = username.to_ascii_lowercase();

        if register {
            let salt = SaltString::generate(&mut OsRng);
            let password_hash = Argon2::default()
                .hash_password(password.as_bytes(), &salt)
                .map_err(|_| AuthError::InvalidInput)?
                .to_string();
            let connection = self
                .connection
                .lock()
                .map_err(|_| AuthError::Storage(String::from("database lock poisoned")))?;
            let result = connection.execute(
                "INSERT INTO accounts (username, password_hash) VALUES (?1, ?2)",
                params![username, password_hash],
            );
            match result {
                Ok(_) => {}
                Err(rusqlite::Error::SqliteFailure(error, _))
                    if error.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    return Err(AuthError::RegistrationFailed);
                }
                Err(error) => return Err(AuthError::Storage(error.to_string())),
            }
        }

        let connection = self
            .connection
            .lock()
            .map_err(|_| AuthError::Storage(String::from("database lock poisoned")))?;
        let row: Option<(i64, String, String)> = connection
            .query_row(
                "SELECT id, username, password_hash FROM accounts WHERE username = ?1",
                params![username],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| AuthError::Storage(error.to_string()))?;
        let Some((id, stored_username, password_hash)) = row else {
            return Err(if register {
                AuthError::RegistrationFailed
            } else {
                AuthError::InvalidCredentials
            });
        };
        let parsed_hash =
            PasswordHash::new(&password_hash).map_err(|_| AuthError::InvalidCredentials)?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| {
                if register {
                    AuthError::RegistrationFailed
                } else {
                    AuthError::InvalidCredentials
                }
            })?;

        connection
            .query_row(
                "SELECT class_id, level, xp, xp_next, gold, rank_points FROM accounts WHERE id = ?1",
                params![id],
                |row| {
                    let class_code: String = row.get(0)?;
                    Ok(Account {
                        id,
                        username: stored_username,
                        class_id: ClassId::from_str_code(&class_code),
                        level: row.get(1)?,
                        xp: row.get(2)?,
                        xp_next: row.get(3)?,
                        gold: row.get(4)?,
                        rank_points: row.get(5)?,
                    })
                },
            )
            .map_err(|error| AuthError::Storage(error.to_string()))
    }

    pub fn save_progress(
        &self,
        account_id: i64,
        class_id: ClassId,
        level: i32,
        xp: i32,
        xp_next: i32,
        gold: i32,
        rank_points: i32,
    ) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| String::from("database lock poisoned"))?;
        connection
            .execute(
                "UPDATE accounts
             SET class_id = ?1, level = ?2, xp = ?3, xp_next = ?4, gold = ?5, rank_points = ?6
             WHERE id = ?7",
                params![
                    class_id.as_code(),
                    level,
                    xp,
                    xp_next,
                    gold,
                    rank_points.max(0),
                    account_id
                ],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

fn valid_username(username: &str) -> bool {
    (3..=18).contains(&username.len())
        && username
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn valid_password(password: &str) -> bool {
    (8..=128).contains(&password.len()) && !password.chars().any(|ch| ch.is_control() || ch == '|')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn account_credentials_and_progress_survive_reopening_the_database() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "wfo-accounts-{}-{nonce}.sqlite3",
            std::process::id()
        ));
        let account_id = {
            let store = AccountStore::open(&path).expect("open test database");
            let account = store
                .authenticate("Test_User", "correct horse battery", true)
                .expect("register test account");
            store
                .save_progress(account.id, ClassId::Arcanist, 4, 25, 300, 90, 1_234)
                .expect("save progression");
            assert!(matches!(
                store.authenticate("test_user", "another password", false),
                Err(AuthError::InvalidCredentials)
            ));
            account.id
        };

        let store = AccountStore::open(&path).expect("reopen test database");
        let account = store
            .authenticate("TEST_USER", "correct horse battery", false)
            .expect("authenticate existing account");
        assert_eq!(account.id, account_id);
        assert_eq!(account.username, "test_user");
        assert_eq!(account.class_id, ClassId::Arcanist);
        assert_eq!(
            (account.level, account.xp, account.xp_next, account.gold),
            (4, 25, 300, 90)
        );
        assert_eq!(account.rank_points, 1_234);
        drop(store);
        std::fs::remove_file(path).expect("remove test database");
    }

    #[test]
    fn existing_account_databases_gain_rank_points_with_a_zero_default() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "wfo-legacy-accounts-{}-{nonce}.sqlite3",
            std::process::id()
        ));
        let connection = Connection::open(&path).expect("open legacy database");
        connection
            .execute_batch(
                "CREATE TABLE accounts (
                    id INTEGER PRIMARY KEY,
                    username TEXT NOT NULL COLLATE NOCASE UNIQUE,
                    password_hash TEXT NOT NULL,
                    class_id TEXT NOT NULL DEFAULT 'vanguard',
                    level INTEGER NOT NULL DEFAULT 1,
                    xp INTEGER NOT NULL DEFAULT 0,
                    xp_next INTEGER NOT NULL DEFAULT 100,
                    gold INTEGER NOT NULL DEFAULT 0
                );",
            )
            .expect("create legacy schema");
        drop(connection);

        let store = AccountStore::open(&path).expect("migrate account database");
        let account = store
            .authenticate("Fresh_User", "correct horse battery", true)
            .expect("register account with migrated schema");
        assert_eq!(account.rank_points, 0);
        drop(store);
        std::fs::remove_file(path).expect("remove test database");
    }

    #[test]
    fn account_input_is_validated() {
        assert!(!valid_username("ab"));
        assert!(!valid_username("no spaces"));
        assert!(!valid_password("short"));
        assert!(!valid_password("password|delimiter"));
        assert!(valid_username("Player_01"));
        assert!(valid_password("correct horse battery"));
    }
}
