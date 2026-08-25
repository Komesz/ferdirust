use cef::*;
use std::sync::{Arc, Mutex};

use crate::app::SharedServiceManager;
use crate::client::FerdiClient;
use crate::service::config::{AppConfig, ServiceConfig};
use crate::service::partition;
use crate::sidebar::badge::BadgeState;

const SIDEBAR_WIDTH: i32 = 56;

// Window delegate for popup windows (calls, OAuth).
// Does NOT quit the message loop when closed — only the main window does that.
wrap_window_delegate! {
    struct PopupWindowDelegate {}

    impl ViewDelegate {}

    impl PanelDelegate {}

    impl WindowDelegate {
        fn window_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }

        fn on_window_destroyed(&self, _window: Option<&mut Window>) {
            // Intentionally empty — closing a popup must not quit the app
        }

        fn initial_bounds(&self, _window: Option<&mut Window>) -> Rect {
            Rect {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            }
        }

        fn can_resize(&self, _window: Option<&mut Window>) -> ::std::os::raw::c_int {
            1
        }

        fn can_close(&self, _window: Option<&mut Window>) -> ::std::os::raw::c_int {
            1
        }
    }
}

// BrowserViewDelegate that forces Alloy runtime style and handles popup windows.
wrap_browser_view_delegate! {
    pub struct AlloyBrowserViewDelegate {}

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }

        fn on_popup_browser_view_created(
            &self,
            _browser_view: Option<&mut BrowserView>,
            popup_browser_view: Option<&mut BrowserView>,
            _is_devtools: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            let Some(popup_view) = popup_browser_view else {
                return 0;
            };

            eprintln!("[popup] on_popup_browser_view_created: creating window");

            let mut delegate = PopupWindowDelegate::new();
            let Some(window) = window_create_top_level(Some(&mut delegate)) else {
                eprintln!("[popup] Failed to create popup window");
                return 0;
            };

            window.set_title(Some(&CefString::from("Ferdirust")));
            window.add_child_view(Some(&mut View::from(&*popup_view)));
            window.show();
            window.center_window(Some(&Size {
                width: 800,
                height: 600,
            }));

            1 // We handled creating the window
        }
    }
}

// A service's browser context (profile) initializes asynchronously; CEF
// refuses to create a BrowserView until it's ready. This handler waits for
// initialization and then creates + attaches the service's browser view.
wrap_request_context_handler! {
    struct PartitionReadyHandler {
        service_id: String,
        service_manager: SharedServiceManager,
    }

    impl RequestContextHandler {
        fn on_request_context_initialized(
            &self,
            request_context: Option<&mut RequestContext>,
        ) {
            let Some(request_context) = request_context else { return };
            if let Ok(mut guard) = self.service_manager.lock() {
                if let Some(mgr) = guard.as_mut() {
                    mgr.attach_service_browser(&self.service_id, request_context);
                }
            }
        }
    }
}

pub struct ServiceInstance {
    pub config: ServiceConfig,
    // None until the service's request context finishes initializing
    // (and briefly during a reset)
    pub browser_view: Option<BrowserView>,
    pub resetting: bool,
}

pub struct ServiceManager {
    pub services: Vec<ServiceInstance>,
    pub sidebar_view: Option<BrowserView>,
    pub active_index: usize,
    pub badge_state: Arc<Mutex<BadgeState>>,
    window: Option<Window>,
    download_dir: String,
    shared_self: SharedServiceManager,
}

impl ServiceManager {
    pub fn create(config: &AppConfig, shared_manager: &SharedServiceManager) -> Self {
        let enabled = config.enabled_services();
        let badge_state = Arc::new(Mutex::new(BadgeState::new()));
        let download_dir = config.global.download_dir.clone();

        let services: Vec<ServiceInstance> = enabled
            .iter()
            .map(|svc| ServiceInstance {
                config: (*svc).clone(),
                browser_view: None,
                resetting: false,
            })
            .collect();

        // Kick off the (async) creation of each service's storage partition;
        // browser views are created in attach_service_browser once each
        // request context reports initialized.
        for svc in &enabled {
            Self::request_partition_context(svc, shared_manager);
        }

        Self {
            services,
            sidebar_view: None,
            active_index: 0,
            badge_state,
            window: None,
            download_dir,
            shared_self: shared_manager.clone(),
        }
    }

    /// Create the service's own request context (storage partition). The
    /// browser view is created later, asynchronously, when the context's
    /// PartitionReadyHandler fires.
    fn request_partition_context(svc: &ServiceConfig, shared_manager: &SharedServiceManager) {
        // Each service gets its own storage partition so its data can be
        // wiped with a plain directory delete (see reset_service).
        let partition_dir = partition::partition_dir(&svc.id);
        partition::migrate_from_shared_profile(svc, &partition_dir);

        let mut context_settings = RequestContextSettings::default();
        context_settings.cache_path =
            CefString::from(partition_dir.to_str().unwrap_or_default());
        context_settings.persist_session_cookies = 1;
        let mut handler =
            PartitionReadyHandler::new(svc.id.clone(), shared_manager.clone());
        if request_context_create_context(Some(&context_settings), Some(&mut handler))
            .is_none()
        {
            eprintln!("[service] {}: failed to create request context", svc.id);
        }
    }

    /// Create the browser view for a service once its request context is
    /// initialized, and attach it to the window (visible if active, otherwise
    /// attached/detached once so CEF initializes the hidden browser).
    fn attach_service_browser(
        &mut self,
        service_id: &str,
        request_context: &mut RequestContext,
    ) {
        let Some(index) = self.services.iter().position(|s| s.config.id == service_id)
        else {
            return;
        };
        if self.services[index].browser_view.is_some() {
            return;
        }
        let svc = self.services[index].config.clone();

        let mut client = FerdiClient::create(
            svc.clone(),
            self.download_dir.clone(),
            self.badge_state.clone(),
            self.shared_self.clone(),
        );

        let url = CefString::from(svc.url.as_str());
        let browser_settings = BrowserSettings::default();
        let mut delegate = AlloyBrowserViewDelegate::new();
        let Some(view) = browser_view_create(
            Some(&mut client),
            Some(&url),
            Some(&browser_settings),
            None,
            Some(request_context),
            Some(&mut delegate),
        ) else {
            eprintln!("[service] {}: failed to create browser view", service_id);
            return;
        };

        eprintln!("[service] {}: browser view ready", service_id);
        self.services[index].browser_view = Some(view);
        self.services[index].resetting = false;

        let Some(window) = &self.window else { return };
        let view_ref = self.services[index].browser_view.as_ref().unwrap();
        let mut view = View::from(view_ref);
        window.add_child_view(Some(&mut view));
        if index == self.active_index {
            let bounds = window.bounds();
            self.layout_views(bounds.width, bounds.height);
        } else {
            // Attach/detach once so CEF initializes the browser; it stays
            // hidden until the user switches to it.
            window.remove_child_view(Some(&mut view));
        }
    }

    /// Set up horizontal BoxLayout with sidebar + service, then override bounds.
    /// Must be called inside on_window_created.
    pub fn setup_window(&mut self, window: &mut Window, shared_manager: &SharedServiceManager) {
        self.window = Some(window.clone());

        // Use BoxLayout to get side-by-side rendering (required for BrowserViews)
        let layout_settings = BoxLayoutSettings {
            horizontal: 1,
            cross_axis_alignment: AxisAlignment::STRETCH,
            ..Default::default()
        };
        window.set_to_box_layout(Some(&layout_settings));

        // Build sidebar HTML with service list injected
        let sidebar_template = include_str!("../../resources/sidebar/sidebar.html");
        let service_json: Vec<String> = self
            .services
            .iter()
            .map(|s| {
                format!(
                    r#"{{"id":"{}","name":"{}","url":"{}"}}"#,
                    s.config.id, s.config.name, s.config.url
                )
            })
            .collect();
        let init_script = format!(
            "<script>initServices([{}]);</script>",
            service_json.join(",")
        );
        // Insert init script before closing </body>
        let sidebar_html = sidebar_template.replace("</body>", &format!("{}</body>", init_script));
        let sidebar_url = format!(
            "data:text/html;charset=utf-8,{}",
            urlencoding(&sidebar_html)
        );
        let mut sidebar_client =
            crate::sidebar::bridge::SidebarClient::create(shared_manager.clone());
        let sidebar_cef_url = CefString::from(sidebar_url.as_str());
        let browser_settings = BrowserSettings::default();
        let mut sidebar_delegate = AlloyBrowserViewDelegate::new();
        let sidebar = browser_view_create(
            Some(&mut sidebar_client),
            Some(&sidebar_cef_url),
            Some(&browser_settings),
            None,
            None,
            Some(&mut sidebar_delegate),
        );

        // Add sidebar as first child
        if let Some(sidebar_view) = sidebar {
            window.add_child_view(Some(&mut View::from(&sidebar_view)));
            self.sidebar_view = Some(sidebar_view);
        }

        // Service browser views normally don't exist yet at this point —
        // they are created and attached by attach_service_browser once each
        // request context finishes initializing. Attach any that already do.
        for (i, instance) in self.services.iter().enumerate() {
            let Some(view) = &instance.browser_view else { continue };
            let mut view = View::from(view);
            window.add_child_view(Some(&mut view));
            if i != self.active_index {
                window.remove_child_view(Some(&mut view));
            }
        }

        // Mark the first service as active so badge updates don't set unread on it
        if let Some(first) = self.services.first() {
            self.badge_state.lock().unwrap().set_active_service(&first.config.id);
        }
    }

    /// Reposition sidebar and active service view for the given window size.
    pub fn layout_views(&self, width: i32, height: i32) {
        // Sidebar: left strip, fixed width
        if let Some(sidebar) = &self.sidebar_view {
            View::from(sidebar).set_bounds(Some(&Rect {
                x: 0,
                y: 0,
                width: SIDEBAR_WIDTH,
                height,
            }));
        }

        // Active service: fills remaining space to the right
        if let Some(view) = self
            .services
            .get(self.active_index)
            .and_then(|i| i.browser_view.as_ref())
        {
            View::from(view).set_bounds(Some(&Rect {
                x: SIDEBAR_WIDTH,
                y: 0,
                width: width - SIDEBAR_WIDTH,
                height,
            }));
        }
    }

    pub fn switch_to_index(&mut self, index: usize) {
        if index >= self.services.len() || index == self.active_index {
            return;
        }
        if self.services[index].resetting || self.services[index].browser_view.is_none() {
            return;
        }

        let Some(window) = &self.window else { return };

        // Remove current service view
        if let Some(old) = &self.services[self.active_index].browser_view {
            window.remove_child_view(Some(&mut View::from(old)));
        }

        // Add new service view
        let new = self.services[index].browser_view.as_ref().unwrap();
        window.add_child_view(Some(&mut View::from(new)));

        self.active_index = index;

        // Override bounds after BoxLayout places the new view
        let bounds = window.bounds();
        self.layout_views(bounds.width, bounds.height);

        // Clear unread dot and suppress future unread marking for the active tab
        let service_id = &self.services[index].config.id;
        let json = {
            let mut state = self.badge_state.lock().unwrap();
            state.set_active_service(service_id);
            state.to_json()
        };
        self.update_sidebar_badges_json(&json);

        // Update sidebar active state
        self.update_sidebar_active();
    }

    pub fn switch_to_id(&mut self, id: &str) {
        if let Some(index) = self.services.iter().position(|s| s.config.id == id) {
            self.switch_to_index(index);
        }
    }

    fn active_browser(&self) -> Option<Browser> {
        self.services
            .get(self.active_index)
            .and_then(|i| i.browser_view.as_ref())
            .and_then(|v| v.browser())
    }

    pub fn reload_active(&self) {
        if let Some(browser) = self.active_browser() {
            browser.reload();
        }
    }

    pub fn hard_reload_active(&self) {
        if let Some(browser) = self.active_browser() {
            browser.reload_ignore_cache();
        }
    }

    fn browser_for(&self, service_id: &str) -> Option<Browser> {
        self.services
            .iter()
            .find(|s| s.config.id == service_id)
            .and_then(|s| s.browser_view.as_ref())
            .and_then(|v| v.browser())
    }

    pub fn reload_service(&self, service_id: &str) {
        if let Some(browser) = self.browser_for(service_id) {
            browser.reload();
        }
    }

    /// Reload bypassing the HTTP cache and the service worker.
    pub fn hard_reload_service(&self, service_id: &str) {
        if let Some(browser) = self.browser_for(service_id) {
            browser.reload_ignore_cache();
        }
    }

    /// Unregister the service's service workers, clear its Cache Storage and
    /// delete its IndexedDB databases, then reload. Fixes stale-SW white
    /// pages and corrupted-IndexedDB boot crashes. Logins live in cookies
    /// and localStorage, which are left untouched; local caches re-sync.
    pub fn repair_service(&self, service_id: &str) {
        let Some(browser) = self.browser_for(service_id) else { return };
        let Some(frame) = browser.main_frame() else { return };
        eprintln!(
            "[service] {}: repair — clearing service workers, caches and IndexedDB",
            service_id
        );
        let js = r#"(async function() {
            try {
                if (navigator.serviceWorker) {
                    var regs = await navigator.serviceWorker.getRegistrations();
                    await Promise.all(regs.map(function(r) { return r.unregister(); }));
                }
            } catch (e) {}
            try {
                if (window.caches) {
                    var keys = await caches.keys();
                    await Promise.all(keys.map(function(k) { return caches.delete(k); }));
                }
            } catch (e) {}
            try {
                if (window.indexedDB && indexedDB.databases) {
                    var dbs = await indexedDB.databases();
                    await Promise.all(dbs.map(function(db) {
                        return new Promise(function(resolve) {
                            var req = indexedDB.deleteDatabase(db.name);
                            req.onsuccess = req.onerror = req.onblocked = function() { resolve(); };
                        });
                    }));
                }
            } catch (e) {}
            location.reload();
        })();"#;
        let url = CefString::from("ferdirust://repair");
        frame.execute_java_script(Some(&CefString::from(js)), Some(&url), 0);
    }

    /// Wipe the service: delete all cookies, clear every storage API the page
    /// can reach (service workers, Cache Storage, IndexedDB, local/session
    /// storage), reload logged-out, and flag the partition directory for a
    /// full delete at next app start. The browser itself is never destroyed —
    /// tearing down a Views-hosted browser inside a shared window crashes CEF.
    pub fn reset_service(&mut self, service_id: &str) {
        let Some(instance) = self.services.iter().find(|s| s.config.id == service_id)
        else {
            return;
        };
        eprintln!(
            "[service] {}: reset — wiping storage; partition deletes fully on next launch",
            service_id
        );

        partition::mark_for_wipe(&partition::partition_dir(service_id));

        let Some(browser) = instance
            .browser_view
            .as_ref()
            .and_then(|v| v.browser())
        else {
            return;
        };

        // All cookies in this partition belong to this service
        if let Some(cookie_manager) = browser
            .host()
            .and_then(|h| h.request_context())
            .and_then(|ctx| ctx.cookie_manager(None))
        {
            cookie_manager.delete_cookies(None, None, None);
        }

        let Some(frame) = browser.main_frame() else { return };
        let js = format!(
            r#"(async function() {{
            try {{
                if (navigator.serviceWorker) {{
                    var regs = await navigator.serviceWorker.getRegistrations();
                    await Promise.all(regs.map(function(r) {{ return r.unregister(); }}));
                }}
            }} catch (e) {{}}
            try {{
                if (window.caches) {{
                    var keys = await caches.keys();
                    await Promise.all(keys.map(function(k) {{ return caches.delete(k); }}));
                }}
            }} catch (e) {{}}
            try {{ localStorage.clear(); }} catch (e) {{}}
            try {{ sessionStorage.clear(); }} catch (e) {{}}
            try {{
                if (window.indexedDB && indexedDB.databases) {{
                    var dbs = await indexedDB.databases();
                    await Promise.all(dbs.map(function(db) {{
                        return new Promise(function(resolve) {{
                            var req = indexedDB.deleteDatabase(db.name);
                            req.onsuccess = req.onerror = req.onblocked = function() {{ resolve(); }};
                        }});
                    }}));
                }}
            }} catch (e) {{}}
            location.replace('{url}');
        }})();"#,
            url = instance.config.url
        );
        let script_url = CefString::from("ferdirust://reset");
        frame.execute_java_script(Some(&CefString::from(js.as_str())), Some(&script_url), 0);
    }

    fn update_sidebar_active(&self) {
        if let Some(sidebar) = &self.sidebar_view {
            if let Some(browser) = sidebar.browser() {
                if let Some(frame) = browser.main_frame() {
                    let js = format!(
                        "if (typeof setActiveService === 'function') setActiveService('{}');",
                        self.services[self.active_index].config.id
                    );
                    let url = CefString::from("ferdirust://internal");
                    frame.execute_java_script(
                        Some(&CefString::from(js.as_str())),
                        Some(&url),
                        0,
                    );
                }
            }
        }
    }

    /// Push badge counts to the sidebar.
    pub fn update_sidebar_badges_json(&self, json: &str) {
        if let Some(sidebar) = &self.sidebar_view {
            if let Some(browser) = sidebar.browser() {
                if let Some(frame) = browser.main_frame() {
                    let js = format!(
                        "if (typeof updateBadges === 'function') updateBadges({});",
                        json
                    );
                    let url = CefString::from("ferdirust://internal");
                    frame.execute_java_script(
                        Some(&CefString::from(js.as_str())),
                        Some(&url),
                        0,
                    );
                }
            }
        }
    }

    /// Update the sidebar icon for a service with a new favicon URL.
    pub fn update_sidebar_favicon(&self, service_id: &str, favicon_url: &str) {
        if let Some(sidebar) = &self.sidebar_view {
            if let Some(browser) = sidebar.browser() {
                if let Some(frame) = browser.main_frame() {
                    let js = format!(
                        "if (typeof updateFavicon === 'function') updateFavicon('{}','{}');",
                        service_id,
                        favicon_url.replace('\'', "\\'")
                    );
                    let url = CefString::from("ferdirust://internal");
                    frame.execute_java_script(
                        Some(&CefString::from(js.as_str())),
                        Some(&url),
                        0,
                    );
                }
            }
        }
    }

    pub fn service_count(&self) -> usize {
        self.services.len()
    }
}

/// Create a popup window for a URL (used by RequestHandler for intercepted navigations).
/// Pass the opener's request context so the popup shares the service's storage partition.
pub fn create_popup_for_url(
    url: &str,
    allowed_origins: Vec<String>,
    auto_grant_media: bool,
    mut request_context: Option<RequestContext>,
) {
    use crate::handlers::life_span::PopupClient;

    eprintln!("[popup] create_popup_for_url: {}", url);

    let mut client = PopupClient::new(auto_grant_media, allowed_origins);
    let cef_url = CefString::from(url);
    let settings = BrowserSettings::default();
    let mut delegate = AlloyBrowserViewDelegate::new();

    let Some(view) = browser_view_create(
        Some(&mut client),
        Some(&cef_url),
        Some(&settings),
        None,
        request_context.as_mut(),
        Some(&mut delegate),
    ) else {
        eprintln!("[popup] Failed to create BrowserView for popup");
        return;
    };

    let mut win_delegate = PopupWindowDelegate::new();
    let Some(window) = window_create_top_level(Some(&mut win_delegate)) else {
        eprintln!("[popup] Failed to create popup window");
        return;
    };

    window.set_title(Some(&CefString::from("Ferdirust")));
    window.add_child_view(Some(&mut View::from(&view)));
    window.show();
    window.center_window(Some(&Size {
        width: 800,
        height: 600,
    }));
}

/// Simple URL encoding for data: URLs
fn urlencoding(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => {
                result.push('%');
                result.push_str(&format!("{:02X}", byte));
            }
        }
    }
    result
}
