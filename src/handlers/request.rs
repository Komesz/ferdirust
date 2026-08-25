use cef::*;

use crate::service::manager::create_popup_for_url;

wrap_request_handler! {
    pub struct FerdiRequestHandler {
        pub service_url: String,
        pub allowed_origins: Vec<String>,
        pub auto_grant_media: bool,
    }

    impl RequestHandler {
        fn on_before_browse(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: ::std::os::raw::c_int,
            _is_redirect: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            // Only intercept main frame navigation
            let Some(frame) = frame else { return 0 };
            if frame.is_main() == 0 {
                return 0;
            }

            let Some(request) = request else { return 0 };
            let url_userfree = request.url();
            let url_cef = CefString::from(&url_userfree);
            let url_str = url_cef.to_string();

            // Allow navigation within the service's own domain(s).
            // e.g. app.slack.com → edgeapi.slack.com is still "slack.com"
            if self
                .allowed_origins
                .iter()
                .any(|origin| self.service_url.contains(origin) && url_str.contains(origin))
            {
                return 0;
            }

            // Check if target is an *external* allowed origin (different domain from service).
            // e.g. messenger.com service navigating to facebook.com/call → intercept
            let is_external_allowed = self.allowed_origins.iter().any(|origin| {
                url_str.contains(origin) && !self.service_url.contains(origin)
            });

            if is_external_allowed {
                eprintln!(
                    "[request] Intercepting main-frame navigation to external allowed origin: {}",
                    url_str
                );
                // Open in popup window instead of navigating the main frame,
                // inside the same storage partition as the service
                let request_context = browser
                    .and_then(|b| b.host())
                    .and_then(|h| h.request_context());
                create_popup_for_url(
                    &url_str,
                    self.allowed_origins.clone(),
                    self.auto_grant_media,
                    request_context,
                );
                return 1; // Cancel the main-frame navigation
            }

            // External URL not in allowed origins — open in system browser
            if !self.allowed_origins.iter().any(|origin| url_str.contains(origin)) {
                eprintln!("[request] Opening in external browser: {}", url_str);
                let _ = std::process::Command::new("xdg-open")
                    .arg(&url_str)
                    .spawn();
                return 1; // Cancel navigation
            }

            0 // Allow other navigation
        }
    }
}

impl FerdiRequestHandler {
    pub fn create(
        service_url: String,
        allowed_origins: Vec<String>,
        auto_grant_media: bool,
    ) -> RequestHandler {
        FerdiRequestHandler::new(service_url, allowed_origins, auto_grant_media)
    }
}
