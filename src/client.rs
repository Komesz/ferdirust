use cef::*;
use std::sync::{Arc, Mutex};

use crate::app::SharedServiceManager;
use crate::handlers::display::FerdiDisplayHandler;
use crate::handlers::download::FerdiDownloadHandler;
use crate::handlers::life_span::FerdiLifeSpanHandler;
use crate::handlers::load::FerdiLoadHandler;
use crate::handlers::permission::FerdiPermissionHandler;
use crate::handlers::request::FerdiRequestHandler;
use crate::service::config::ServiceConfig;
use crate::sidebar::badge::BadgeState;

wrap_client! {
    pub struct FerdiClient {
        pub service_config: ServiceConfig,
        pub download_dir: String,
        pub badge_state: Arc<Mutex<BadgeState>>,
        pub service_manager: SharedServiceManager,
    }

    impl Client {
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(FerdiDisplayHandler::create(
                self.service_config.id.clone(),
                self.badge_state.clone(),
                self.service_manager.clone(),
            ))
        }

        fn download_handler(&self) -> Option<DownloadHandler> {
            Some(FerdiDownloadHandler::create(self.download_dir.clone()))
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(FerdiLifeSpanHandler::create(
                self.service_config.allowed_origins.clone(),
                self.service_config.auto_grant_media,
            ))
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(FerdiLoadHandler::create(
                self.service_config.id.clone(),
                self.badge_state.clone(),
            ))
        }

        fn permission_handler(&self) -> Option<PermissionHandler> {
            Some(FerdiPermissionHandler::create(
                self.service_config.auto_grant_media,
                self.service_config.allowed_origins.clone(),
            ))
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(FerdiRequestHandler::create(
                self.service_config.url.clone(),
                self.service_config.allowed_origins.clone(),
                self.service_config.auto_grant_media,
            ))
        }
    }
}

impl FerdiClient {
    pub fn create(
        service_config: ServiceConfig,
        download_dir: String,
        badge_state: Arc<Mutex<BadgeState>>,
        service_manager: SharedServiceManager,
    ) -> Client {
        FerdiClient::new(service_config, download_dir, badge_state, service_manager)
    }
}
