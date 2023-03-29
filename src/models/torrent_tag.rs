use serde::{Deserialize, Serialize};
use sqlx::FromRow;

pub type TagId = i64;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct TorrentTag {
    pub tag_id: TagId,
    pub name: String,
}
