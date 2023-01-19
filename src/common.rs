use std::sync::Arc;

use crate::auth::AuthorizationService;
use crate::cache::image::ByteCache;
use crate::config::Configuration;
use crate::databases::database::Database;
use crate::mailer::MailerService;
use crate::tracker::TrackerService;

pub type Username = String;

pub type WebAppData = actix_web::web::Data<Arc<AppData>>;

pub struct AppData {
    pub cfg: Arc<Configuration>,
    pub database: Arc<Box<dyn Database>>,
    pub auth: Arc<AuthorizationService>,
    pub tracker: Arc<TrackerService>,
    pub mailer: Arc<MailerService>,
    pub image_cache: Arc<ByteCache>
}

impl AppData {
    pub fn new(
        cfg: Arc<Configuration>,
        database: Arc<Box<dyn Database>>,
        auth: Arc<AuthorizationService>,
        tracker: Arc<TrackerService>,
        mailer: Arc<MailerService>,
        image_cache: Arc<ByteCache>
    ) -> AppData {
        AppData {
            cfg,
            database,
            auth,
            tracker,
            mailer,
            image_cache
        }
    }
}
