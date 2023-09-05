use async_trait::async_trait;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::databases::mysql::Mysql;
use crate::databases::sqlite::Sqlite;
use crate::models::category::CategoryId;
use crate::models::info_hash::InfoHash;
use crate::models::response::TorrentsResponse;
use crate::models::torrent::TorrentListing;
use crate::models::torrent_file::{DbTorrentInfo, Torrent, TorrentFile};
use crate::models::torrent_tag::{TagId, TorrentTag};
use crate::models::tracker_key::TrackerKey;
use crate::models::user::{User, UserAuthentication, UserCompact, UserId, UserProfile};
use crate::services::torrent::OriginalInfoHashes;

/// Database tables to be truncated when upgrading from v1.0.0 to v2.0.0.
/// They must be in the correct order to avoid foreign key errors.
pub const TABLES_TO_TRUNCATE: &[&str] = &[
    "torrust_torrent_announce_urls",
    "torrust_torrent_files",
    "torrust_torrent_info",
    "torrust_torrent_tag_links",
    "torrust_torrent_tracker_stats",
    "torrust_torrents",
    "torrust_tracker_keys",
    "torrust_user_authentication",
    "torrust_user_bans",
    "torrust_user_invitation_uses",
    "torrust_user_invitations",
    "torrust_user_profiles",
    "torrust_user_public_keys",
    "torrust_users",
    "torrust_categories",
    "torrust_torrent_tags",
];

/// Database drivers.
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum Driver {
    Sqlite3,
    Mysql,
}

/// Compact representation of torrent.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TorrentCompact {
    pub torrent_id: i64,
    pub info_hash: String,
}

/// Torrent category.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Category {
    pub category_id: i64,
    pub name: String,
    pub num_torrents: i64,
}

/// Sorting options for torrents.
#[derive(Clone, Copy, Debug, Deserialize)]
pub enum Sorting {
    UploadedAsc,
    UploadedDesc,
    SeedersAsc,
    SeedersDesc,
    LeechersAsc,
    LeechersDesc,
    NameAsc,
    NameDesc,
    SizeAsc,
    SizeDesc,
}

/// Database errors.
#[derive(Debug)]
pub enum Error {
    Error,
    ErrorWithText(String),
    UnrecognizedDatabaseDriver, // when the db path does not start with sqlite or mysql
    UsernameTaken,
    EmailTaken,
    UserNotFound,
    CategoryAlreadyExists,
    CategoryNotFound,
    TagAlreadyExists,
    TagNotFound,
    TorrentNotFound,
    TorrentAlreadyExists, // when uploading an already uploaded info_hash
    TorrentTitleAlreadyExists,
    TorrentInfoHashNotFound,
}

/// Get the Driver of the Database from the Connection String
///
/// # Errors
///
/// This function will return an `Error::UnrecognizedDatabaseDriver` if unable to match database type.
pub fn get_driver(db_path: &str) -> Result<Driver, Error> {
    match &db_path.chars().collect::<Vec<char>>() as &[char] {
        ['s', 'q', 'l', 'i', 't', 'e', ..] => Ok(Driver::Sqlite3),
        ['m', 'y', 's', 'q', 'l', ..] => Ok(Driver::Mysql),
        _ => Err(Error::UnrecognizedDatabaseDriver),
    }
}

/// Connect to a database.
///
/// # Errors
///
/// This function will return an `Error::UnrecognizedDatabaseDriver` if unable to match database type.
pub async fn connect(db_path: &str) -> Result<Box<dyn Database>, Error> {
    let db_driver = self::get_driver(db_path)?;

    Ok(match db_driver {
        self::Driver::Sqlite3 => Box::new(Sqlite::new(db_path).await),
        self::Driver::Mysql => Box::new(Mysql::new(db_path).await),
    })
}

/// Trait for database implementations.
#[async_trait]
pub trait Database: Sync + Send {
    /// Return current database driver.
    fn get_database_driver(&self) -> Driver;

    async fn new(db_path: &str) -> Self
    where
        Self: Sized;

    /// Add new user and return the newly inserted `user_id`.
    async fn insert_user_and_get_id(&self, username: &str, email: &str, password: &str) -> Result<UserId, Error>;

    /// Get `User` from `user_id`.
    async fn get_user_from_id(&self, user_id: i64) -> Result<User, Error>;

    /// Get `UserAuthentication` from `user_id`.
    async fn get_user_authentication_from_id(&self, user_id: UserId) -> Result<UserAuthentication, Error>;

    /// Get `UserProfile` from `username`.
    async fn get_user_profile_from_username(&self, username: &str) -> Result<UserProfile, Error>;

    /// Get `UserCompact` from `user_id`.
    async fn get_user_compact_from_id(&self, user_id: i64) -> Result<UserCompact, Error>;

    /// Get a user's `TrackerKey`.
    async fn get_user_tracker_key(&self, user_id: i64) -> Option<TrackerKey>;

    /// Get total user count.
    async fn count_users(&self) -> Result<i64, Error>;

    /// Ban user with `user_id`, `reason` and `date_expiry`.
    async fn ban_user(&self, user_id: i64, reason: &str, date_expiry: NaiveDateTime) -> Result<(), Error>;

    /// Grant a user the administrator role.
    async fn grant_admin_role(&self, user_id: i64) -> Result<(), Error>;

    /// Verify a user's email with `user_id`.
    async fn verify_email(&self, user_id: i64) -> Result<(), Error>;

    /// Link a `TrackerKey` to a certain user with `user_id`.
    async fn add_tracker_key(&self, user_id: i64, tracker_key: &TrackerKey) -> Result<(), Error>;

    /// Delete user and all related user data with `user_id`.
    async fn delete_user(&self, user_id: i64) -> Result<(), Error>;

    /// Add a new category and return `category_id`.
    async fn insert_category_and_get_id(&self, category_name: &str) -> Result<i64, Error>;

    /// Get `Category` from `category_id`.
    async fn get_category_from_id(&self, category_id: i64) -> Result<Category, Error>;

    /// Get `Category` from `category_name`.
    async fn get_category_from_name(&self, category_name: &str) -> Result<Category, Error>;

    /// Get all categories as `Vec<Category>`.
    async fn get_categories(&self) -> Result<Vec<Category>, Error>;

    /// Delete category with `category_name`.
    async fn delete_category(&self, category_name: &str) -> Result<(), Error>;

    /// Get results of a torrent search in a paginated and sorted form as `TorrentsResponse` from `search`, `categories`, `sort`, `offset` and `page_size`.
    async fn get_torrents_search_sorted_paginated(
        &self,
        search: &Option<String>,
        categories: &Option<Vec<String>>,
        tags: &Option<Vec<String>>,
        sort: &Sorting,
        offset: u64,
        page_size: u8,
    ) -> Result<TorrentsResponse, Error>;

    /// Add new torrent and return the newly inserted `torrent_id` with `torrent`, `uploader_id`, `category_id`, `title` and `description`.
    async fn insert_torrent_and_get_id(
        &self,
        original_info_hash: &InfoHash,
        torrent: &Torrent,
        uploader_id: UserId,
        category_id: i64,
        title: &str,
        description: &str,
    ) -> Result<i64, Error>;

    /// Get `Torrent` from `InfoHash`.
    async fn get_torrent_from_info_hash(&self, info_hash: &InfoHash) -> Result<Torrent, Error> {
        let torrent_info = self.get_torrent_info_from_info_hash(info_hash).await?;

        let torrent_files = self.get_torrent_files_from_id(torrent_info.torrent_id).await?;

        let torrent_announce_urls = self.get_torrent_announce_urls_from_id(torrent_info.torrent_id).await?;

        Ok(Torrent::from_db_info_files_and_announce_urls(
            torrent_info,
            torrent_files,
            torrent_announce_urls,
        ))
    }

    /// Get `Torrent` from `torrent_id`.
    async fn get_torrent_from_id(&self, torrent_id: i64) -> Result<Torrent, Error> {
        let torrent_info = self.get_torrent_info_from_id(torrent_id).await?;

        let torrent_files = self.get_torrent_files_from_id(torrent_id).await?;

        let torrent_announce_urls = self.get_torrent_announce_urls_from_id(torrent_id).await?;

        Ok(Torrent::from_db_info_files_and_announce_urls(
            torrent_info,
            torrent_files,
            torrent_announce_urls,
        ))
    }

    /// Returns the list of original infohashes ofr a canonical infohash.
    ///
    /// When you upload a torrent the infohash migth change because the Index
    /// remove the non-standard fields in the `info` dictionary. That makes the
    /// infohash change. The canonical infohash is the resulting infohash.
    /// This function returns the original infohashes of a canonical infohash.
    /// The relationship is 1 canonical infohash -> N original infohashes.
    async fn get_torrent_original_info_hashes(&self, canonical: &InfoHash) -> Result<OriginalInfoHashes, Error>;

    async fn insert_torrent_info_hash(&self, original: &InfoHash, canonical: &InfoHash) -> Result<(), Error>;

    /// Get torrent's info as `DbTorrentInfo` from `torrent_id`.
    async fn get_torrent_info_from_id(&self, torrent_id: i64) -> Result<DbTorrentInfo, Error>;

    /// Get torrent's info as `DbTorrentInfo` from torrent `InfoHash`.
    async fn get_torrent_info_from_info_hash(&self, info_hash: &InfoHash) -> Result<DbTorrentInfo, Error>;

    /// Get all torrent's files as `Vec<TorrentFile>` from `torrent_id`.
    async fn get_torrent_files_from_id(&self, torrent_id: i64) -> Result<Vec<TorrentFile>, Error>;

    /// Get all torrent's announce urls as `Vec<Vec<String>>` from `torrent_id`.
    async fn get_torrent_announce_urls_from_id(&self, torrent_id: i64) -> Result<Vec<Vec<String>>, Error>;

    /// Get `TorrentListing` from `torrent_id`.
    async fn get_torrent_listing_from_id(&self, torrent_id: i64) -> Result<TorrentListing, Error>;

    /// Get `TorrentListing` from `InfoHash`.
    async fn get_torrent_listing_from_info_hash(&self, info_hash: &InfoHash) -> Result<TorrentListing, Error>;

    /// Get all torrents as `Vec<TorrentCompact>`.
    async fn get_all_torrents_compact(&self) -> Result<Vec<TorrentCompact>, Error>;

    /// Update a torrent's title with `torrent_id` and `title`.
    async fn update_torrent_title(&self, torrent_id: i64, title: &str) -> Result<(), Error>;

    /// Update a torrent's description with `torrent_id` and `description`.
    async fn update_torrent_description(&self, torrent_id: i64, description: &str) -> Result<(), Error>;

    /// Update a torrent's category with `torrent_id` and `category_id`.
    async fn update_torrent_category(&self, torrent_id: i64, category_id: CategoryId) -> Result<(), Error>;

    /// Add a new tag.
    async fn add_tag(&self, name: &str) -> Result<(), Error>;

    /// Delete a tag.
    async fn delete_tag(&self, tag_id: TagId) -> Result<(), Error>;

    /// Add a tag to torrent.
    async fn add_torrent_tag_link(&self, torrent_id: i64, tag_id: TagId) -> Result<(), Error>;

    /// Add multiple tags to a torrent at once.
    async fn add_torrent_tag_links(&self, torrent_id: i64, tag_ids: &[TagId]) -> Result<(), Error>;

    /// Remove a tag from torrent.
    async fn delete_torrent_tag_link(&self, torrent_id: i64, tag_id: TagId) -> Result<(), Error>;

    /// Remove all tags from torrent.
    async fn delete_all_torrent_tag_links(&self, torrent_id: i64) -> Result<(), Error>;

    /// Get tag from name.
    async fn get_tag_from_name(&self, name: &str) -> Result<TorrentTag, Error>;

    /// Get all tags as `Vec<TorrentTag>`.
    async fn get_tags(&self) -> Result<Vec<TorrentTag>, Error>;

    /// Get tags for `torrent_id`.
    async fn get_tags_for_torrent_id(&self, torrent_id: i64) -> Result<Vec<TorrentTag>, Error>;

    /// Update the seeders and leechers info for a torrent with `torrent_id`, `tracker_url`, `seeders` and `leechers`.
    async fn update_tracker_info(&self, torrent_id: i64, tracker_url: &str, seeders: i64, leechers: i64) -> Result<(), Error>;

    /// Delete a torrent with `torrent_id`.
    async fn delete_torrent(&self, torrent_id: i64) -> Result<(), Error>;

    /// DELETES ALL DATABASE ROWS, ONLY CALL THIS IF YOU KNOW WHAT YOU'RE DOING!
    async fn delete_all_database_rows(&self) -> Result<(), Error>;
}
