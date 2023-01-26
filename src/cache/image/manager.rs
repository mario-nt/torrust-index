use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use bytes::Bytes;
use tokio::sync::RwLock;

use crate::cache::cache::BytesCache;
use crate::models::user::UserId;

pub enum Error {
    UrlIsUnreachable,
    UrlIsNotAnImage,
    ImageTooBig,
    UserQuotaMet
}

type UserQuotas = HashMap<UserId, ImageCacheQuota>;

pub fn now_in_secs() -> u64 {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(n) => n.as_secs(),
        Err(_) => panic!("SystemTime before UNIX EPOCH!"),
    }
}

#[derive(Clone)]
pub struct ImageCacheQuota {
    pub user_id: UserId,
    pub usage: usize,
    pub max_usage: usize,
    pub date_start_secs: u64,
    pub period_secs: u64
}

impl ImageCacheQuota {
    pub fn new(user_id: UserId, max_usage: usize, period_secs: u64) -> Self {
        Self {
            user_id,
            usage: 0,
            max_usage,
            date_start_secs: now_in_secs(),
            period_secs
        }
    }

    pub fn add_usage(&mut self, amount: usize) -> Result<(), ()> {
        // Check if quota needs to be reset.
        if now_in_secs() - self.date_start_secs > self.period_secs {
            self.reset();
        }

        if self.met() {
            return Err(())
        }

        self.usage = self.usage.saturating_add(amount);

        Ok(())
    }

    pub fn reset(&mut self) {
        self.usage = 0;
        self.date_start_secs = now_in_secs();
    }

    pub fn met(&self) -> bool {
        self.usage >= self.max_usage
    }
}

pub struct ImageCacheManagerConfig {
    pub max_image_request_timeout_ms: u64,
    pub max_image_size: usize
}

pub struct ImageCacheManager {
    image_cache: RwLock<BytesCache>,
    user_quotas: RwLock<UserQuotas>,
    reqwest_client: reqwest::Client,
    config: ImageCacheManagerConfig
}

impl ImageCacheManager {
    pub fn new(bytes_cache: BytesCache, config: ImageCacheManagerConfig) -> Self {
        let reqwest_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.max_image_request_timeout_ms))
            .build()
            .unwrap();

        Self {
            image_cache: RwLock::new(bytes_cache),
            user_quotas: RwLock::new(HashMap::new()),
            reqwest_client,
            config
        }
    }

    pub async fn get_image_by_url(&self, user_id: &UserId, url: &str) -> Result<Bytes, Error> {
        // Check if image is already in our cache and send it if so.
        if let Some(entry) = self.image_cache.read().await.get(&url).await {
            return Ok(entry.bytes)
        }

        if let Some(quota) = self.user_quotas.read().await.get(&user_id) {
            if quota.met() {
                return Err(Error::UserQuotaMet)
            }
        }

        let res = self
            .reqwest_client
            .clone()
            .get(url)
            .send()
            .await
            .map_err(|_| Error::UrlIsUnreachable)?;

        // Verify the content-type of the response.
        if let Some(content_type) = res.headers().get("Content-Type") {
            if content_type != "image/jpeg" && content_type != "image/png" {
                return Err(Error::UrlIsNotAnImage)
            }
        } else {
            return Err(Error::UrlIsNotAnImage)
        }

        let image_bytes = res
            .bytes()
            .await
            .map_err(|_| Error::UrlIsNotAnImage)?;

        // Verify that the response size does not exceed the defined max image size.
        if image_bytes.len() > self.config.max_image_size {
            return Err(Error::ImageTooBig)
        }

        // TODO: Update the cache on a separate thread, so that the client does not have to wait.
        // Update image cache.
        if self.image_cache.write().await.set(url.to_string(), image_bytes.clone()).await.is_err() {
            return Err(Error::ImageTooBig)
        }

        if let Some(mut quota) = self.user_quotas.read().await.get(&user_id).cloned() {
            let _ = quota.add_usage(image_bytes.len());
        }

        Ok(image_bytes)
    }
}
