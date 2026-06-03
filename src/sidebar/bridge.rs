use cef::*;

use crate::app::SharedServiceManager;

// DisplayHandler that intercepts "ferdirust:switch:{id}" title changes
wrap_display_handler! {
    struct SidebarDisplayHandler {
        service_manager: SharedServiceManager,
    }

    impl DisplayHandler {
        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let Some(title) = title {
                let title_str = title.to_string();
                if let Some(id) = title_str.strip_prefix("ferdirust:switch:") {
                    if let Ok(mut guard) = self.service_manager.lock() {
                        if let Some(mgr) = guard.as_mut() {
                            mgr.switch_to_id(id);
                        }
                    }
                }
            }
        }
    }
}

// Client for the sidebar BrowserView
wrap_client! {
    pub struct SidebarClient {
        pub service_manager: SharedServiceManager,
    }

    impl Client {
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(SidebarDisplayHandler::new(self.service_manager.clone()))
        }
    }
}

impl SidebarClient {
    pub fn create(service_manager: SharedServiceManager) -> Client {
        SidebarClient::new(service_manager)
    }
}
