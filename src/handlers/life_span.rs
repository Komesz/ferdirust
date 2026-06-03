use cef::*;

use crate::handlers::permission::FerdiPermissionHandler;

// Lightweight client for popup windows (calls, OAuth).
// Provides permission handling so media access works in call popups.
wrap_client! {
    pub struct PopupClient {
        pub auto_grant_media: bool,
        pub allowed_origins: Vec<String>,
    }

    impl Client {
        fn permission_handler(&self) -> Option<PermissionHandler> {
            Some(FerdiPermissionHandler::create(
                self.auto_grant_media,
                self.allowed_origins.clone(),
            ))
        }
    }
}

wrap_life_span_handler! {
    pub struct FerdiLifeSpanHandler {
        pub allowed_origins: Vec<String>,
        pub auto_grant_media: bool,
    }

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: ::std::os::raw::c_int,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: ::std::os::raw::c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            if let Some(url) = target_url {
                let url_str = url.to_string();
                eprintln!("[popup] on_before_popup: {}", url_str);

                // Allow popup for URLs matching allowed origins, or about:blank
                // (Messenger opens calls as window.open('about:blank') then navigates via JS)
                let is_allowed = url_str == "about:blank"
                    || self
                        .allowed_origins
                        .iter()
                        .any(|origin| url_str.contains(origin));

                if is_allowed {
                    eprintln!("[popup] Allowing popup: {}", url_str);
                    // Provide a client with permission handling so media works in popups
                    if let Some(client_slot) = client {
                        *client_slot = Some(PopupClient::new(
                            self.auto_grant_media,
                            self.allowed_origins.clone(),
                        ));
                    }
                    return 0; // Allow CEF to create the popup BrowserView
                }

                // For external URLs, open in system browser
                eprintln!("[popup] Opening in external browser: {}", url_str);
                let _ = std::process::Command::new("xdg-open")
                    .arg(&url_str)
                    .spawn();
            }
            1 // Cancel popup
        }
    }
}

impl FerdiLifeSpanHandler {
    pub fn create(allowed_origins: Vec<String>, auto_grant_media: bool) -> LifeSpanHandler {
        FerdiLifeSpanHandler::new(allowed_origins, auto_grant_media)
    }
}
