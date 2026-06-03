use cef::*;
use std::sync::Arc;

use crate::app::SharedServiceManager;
use crate::service::config::AppConfig;
use crate::service::manager::ServiceManager;
use crate::window_delegate::FerdiWindowDelegate;

wrap_browser_process_handler! {
    pub struct FerdiBrowserProcessHandler {
        pub service_manager: SharedServiceManager,
        pub config: Arc<AppConfig>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            // Create the service manager (creates all BrowserViews)
            let manager = ServiceManager::create(&self.config, &self.service_manager);

            // Store manager BEFORE creating window — on_window_created needs it
            if let Ok(mut guard) = self.service_manager.lock() {
                *guard = Some(manager);
            }

            // Create window — on_window_created will set up layout via SharedServiceManager
            let mut delegate = FerdiWindowDelegate::create(self.service_manager.clone());
            let window = window_create_top_level(Some(&mut delegate));

            if window.is_none() {
                eprintln!("[ferdirust] Failed to create top-level window");
                quit_message_loop();
            }
        }
    }
}

impl FerdiBrowserProcessHandler {
    pub fn create(
        service_manager: SharedServiceManager,
        config: Arc<AppConfig>,
    ) -> BrowserProcessHandler {
        FerdiBrowserProcessHandler::new(service_manager, config)
    }
}
