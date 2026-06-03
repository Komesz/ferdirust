use cef::*;
use std::sync::{Arc, Mutex};

use crate::app::SharedServiceManager;
use crate::client::FerdiClient;
use crate::service::config::{AppConfig, ServiceConfig};
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

pub struct ServiceInstance {
    pub config: ServiceConfig,
    pub browser_view: BrowserView,
}

pub struct ServiceManager {
    pub services: Vec<ServiceInstance>,
    pub sidebar_view: Option<BrowserView>,
    pub active_index: usize,
    pub badge_state: Arc<Mutex<BadgeState>>,
    window: Option<Window>,
}

impl ServiceManager {
    pub fn create(config: &AppConfig, shared_manager: &SharedServiceManager) -> Self {
        let enabled = config.enabled_services();
        let badge_state = Arc::new(Mutex::new(BadgeState::new()));

        let services: Vec<ServiceInstance> = enabled
            .iter()
            .map(|svc| {
                let browser_view =
                    Self::create_service_view(svc, config, &badge_state, shared_manager);
                ServiceInstance {
                    config: (*svc).clone(),
                    browser_view,
                }
            })
            .collect();

        Self {
            services,
            sidebar_view: None,
            active_index: 0,
            badge_state,
            window: None,
        }
    }

    fn create_service_view(
        svc: &ServiceConfig,
        config: &AppConfig,
        badge_state: &Arc<Mutex<BadgeState>>,
        shared_manager: &SharedServiceManager,
    ) -> BrowserView {
        let mut client = FerdiClient::create(
            svc.clone(),
            config.global.download_dir.clone(),
            badge_state.clone(),
            shared_manager.clone(),
        );

        let url = CefString::from(svc.url.as_str());
        let browser_settings = BrowserSettings::default();

        let mut delegate = AlloyBrowserViewDelegate::new();
        let view = browser_view_create(
            Some(&mut client),
            Some(&url),
            Some(&browser_settings),
            None,
            None,
            Some(&mut delegate),
        );

        view.expect(&format!("Failed to create BrowserView for {}", svc.id))
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

        // Add ALL service views to force CEF to initialize their browsers,
        // then remove non-active ones. Without this, unattached BrowserViews
        // don't load content until first displayed.
        for instance in &self.services {
            window.add_child_view(Some(&mut View::from(&instance.browser_view)));
        }
        // Remove all except the first (active) service
        for instance in self.services.iter().skip(1) {
            window.remove_child_view(Some(&mut View::from(&instance.browser_view)));
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
        if let Some(instance) = self.services.get(self.active_index) {
            View::from(&instance.browser_view).set_bounds(Some(&Rect {
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

        let Some(window) = &self.window else { return };

        // Remove current service view
        let mut old_view = View::from(&self.services[self.active_index].browser_view);
        window.remove_child_view(Some(&mut old_view));

        // Add new service view
        let mut new_view = View::from(&self.services[index].browser_view);
        window.add_child_view(Some(&mut new_view));

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

    pub fn reload_active(&self) {
        if let Some(instance) = self.services.get(self.active_index) {
            if let Some(browser) = instance.browser_view.browser() {
                browser.reload();
            }
        }
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
pub fn create_popup_for_url(url: &str, allowed_origins: Vec<String>, auto_grant_media: bool) {
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
        None,
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
