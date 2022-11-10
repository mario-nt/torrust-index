use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{query_as, SqlitePool};
use torrust_index_backend::upgrades::from_v1_0_0_to_v2_0_0::databases::sqlite_v2_0_0::TorrentRecordV2;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserRecordV2 {
    pub user_id: i64,
    pub date_registered: Option<String>,
    pub date_imported: Option<String>,
    pub administrator: bool,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserProfileRecordV2 {
    pub user_id: i64,
    pub username: String,
    pub email: String,
    pub email_verified: bool,
    pub bio: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserAuthenticationRecordV2 {
    pub user_id: i64,
    pub password_hash: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TrackerKeyRecordV2 {
    pub tracker_key_id: i64,
    pub user_id: i64,
    pub tracker_key: String,
    pub date_expiry: i64,
}

pub struct SqliteDatabaseV2_0_0 {
    pub pool: SqlitePool,
}

impl SqliteDatabaseV2_0_0 {
    pub async fn db_connection(database_file: &str) -> Self {
        let connect_url = format!("sqlite://{}?mode=rwc", database_file);
        Self::new(&connect_url).await
    }

    pub async fn new(database_url: &str) -> Self {
        let db = SqlitePoolOptions::new()
            .connect(database_url)
            .await
            .expect("Unable to create database pool.");
        Self { pool: db }
    }

    pub async fn get_user(&self, user_id: i64) -> Result<UserRecordV2, sqlx::Error> {
        query_as::<_, UserRecordV2>("SELECT * FROM torrust_users WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await
    }

    pub async fn get_user_profile(&self, user_id: i64) -> Result<UserProfileRecordV2, sqlx::Error> {
        query_as::<_, UserProfileRecordV2>("SELECT * FROM torrust_user_profiles WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await
    }

    pub async fn get_user_authentication(
        &self,
        user_id: i64,
    ) -> Result<UserAuthenticationRecordV2, sqlx::Error> {
        query_as::<_, UserAuthenticationRecordV2>(
            "SELECT * FROM torrust_user_authentication WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_tracker_key(
        &self,
        tracker_key_id: i64,
    ) -> Result<TrackerKeyRecordV2, sqlx::Error> {
        query_as::<_, TrackerKeyRecordV2>("SELECT * FROM torrust_tracker_keys WHERE user_id = ?")
            .bind(tracker_key_id)
            .fetch_one(&self.pool)
            .await
    }

    pub async fn get_torrent(&self, torrent_id: i64) -> Result<TorrentRecordV2, sqlx::Error> {
        query_as::<_, TorrentRecordV2>("SELECT * FROM torrust_torrents WHERE torrent_id = ?")
            .bind(torrent_id)
            .fetch_one(&self.pool)
            .await
    }
}
