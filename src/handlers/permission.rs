use cef::*;

wrap_permission_handler! {
    pub struct FerdiPermissionHandler {
        pub auto_grant_media: bool,
        pub allowed_origins: Vec<String>,
    }

    impl PermissionHandler {
        fn on_request_media_access_permission(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut MediaAccessCallback>,
        ) -> ::std::os::raw::c_int {
            if self.auto_grant_media {
                if let Some(url) = requesting_origin {
                    let url_str = url.to_string();
                    let should_grant = self.allowed_origins.iter().any(|origin| {
                        url_str.contains(origin)
                    });

                    if should_grant {
                        if let Some(cb) = callback {
                            cb.cont(requested_permissions);
                            return 1;
                        }
                    }
                }
            }
            0
        }

        fn on_show_permission_prompt(
            &self,
            _browser: Option<&mut Browser>,
            _prompt_id: u64,
            requesting_origin: Option<&CefString>,
            _requested_permissions: u32,
            callback: Option<&mut PermissionPromptCallback>,
        ) -> ::std::os::raw::c_int {
            // Auto-accept permission prompts for allowed origins
            if let Some(url) = requesting_origin {
                let url_str = url.to_string();
                let should_grant = self.allowed_origins.iter().any(|origin| {
                    url_str.contains(origin)
                });

                if should_grant {
                    if let Some(cb) = callback {
                        cb.cont(
                            cef::sys::cef_permission_request_result_t::CEF_PERMISSION_RESULT_ACCEPT
                                .into(),
                        );
                        return 1;
                    }
                }
            }
            0
        }
    }
}

impl FerdiPermissionHandler {
    pub fn create(auto_grant_media: bool, allowed_origins: Vec<String>) -> PermissionHandler {
        FerdiPermissionHandler::new(auto_grant_media, allowed_origins)
    }
}
