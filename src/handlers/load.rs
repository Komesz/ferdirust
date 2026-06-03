use cef::*;
use std::sync::{Arc, Mutex};

use crate::sidebar::badge::BadgeState;

wrap_load_handler! {
    pub struct FerdiLoadHandler {
        pub service_id: String,
        pub badge_state: Arc<Mutex<BadgeState>>,
    }

    impl LoadHandler {
        fn on_load_start(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _transition_type: TransitionType,
        ) {
            // Only inject into main frame
            let Some(frame) = frame else { return };
            if frame.is_main() == 0 {
                return;
            }

            // Inject notification override EARLY — before the page JS runs,
            // so apps can't cache a reference to the original Notification.
            let notification_script = get_notification_override_script(&self.service_id);
            let url = CefString::from("ferdirust://inject");
            frame.execute_java_script(
                Some(&CefString::from(notification_script.as_str())),
                Some(&url),
                0,
            );
        }

        fn on_load_end(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _http_status_code: ::std::os::raw::c_int,
        ) {
            // Only inject into main frame
            let Some(frame) = frame else { return };
            if frame.is_main() == 0 {
                return;
            }

            // Inject service-specific badge script (needs DOM ready)
            let script = get_badge_script(&self.service_id);
            if !script.is_empty() {
                let url = CefString::from("ferdirust://inject");
                frame.execute_java_script(
                    Some(&CefString::from(script)),
                    Some(&url),
                    0,
                );
            }

            // Inject service-specific post-load setup (localStorage, etc.)
            let setup = get_post_load_script(&self.service_id);
            if !setup.is_empty() {
                let url = CefString::from("ferdirust://inject");
                frame.execute_java_script(
                    Some(&CefString::from(setup)),
                    Some(&url),
                    0,
                );
            }

            let _ = browser;
        }
    }
}

impl FerdiLoadHandler {
    pub fn create(service_id: String, badge_state: Arc<Mutex<BadgeState>>) -> LoadHandler {
        FerdiLoadHandler::new(service_id, badge_state)
    }
}

/// Returns true if this service has a custom badge script (uses console.log IPC).
pub fn has_badge_script(service_id: &str) -> bool {
    !get_badge_script(service_id).is_empty()
}

fn get_badge_script(service_id: &str) -> &'static str {
    match service_id {
        "messenger" => include_str!("../../resources/scripts/messenger.js"),
        s if s == "slack" || s.starts_with("slack-") => include_str!("../../resources/scripts/slack.js"),
        "protonmail" => include_str!("../../resources/scripts/protonmail.js"),
        "telegram" => include_str!("../../resources/scripts/telegram.js"),
        _ => "",
    }
}

fn get_notification_override_script(service_id: &str) -> String {
    format!(
        r#"
(function() {{
    if (window.__ferdirust_notif_injected) return;
    window.__ferdirust_notif_injected = true;

    var ready = false;
    setTimeout(function() {{ ready = true; }}, 15000);

    function ferdiNotify(title, body) {{
        if (!ready) return;
        console.log('ferdirust:notify:' + JSON.stringify({{
            service: '{service_id}',
            title: title || '',
            body: body || ''
        }}));
    }}

    // Override classic Notification constructor
    window.Notification = function(title, options) {{
        ferdiNotify(title, options && options.body);
    }};
    window.Notification.permission = 'granted';
    window.Notification.requestPermission = function() {{
        return Promise.resolve('granted');
    }};

    // Override ServiceWorker showNotification (used by modern web apps)
    if (navigator.serviceWorker) {{
        ServiceWorkerRegistration.prototype.showNotification = function(title, options) {{
            ferdiNotify(title, options && options.body);
            return Promise.resolve();
        }};
    }}
}})();
"#
    )
}

/// Per-service post-load setup scripts (localStorage, etc.)
fn get_post_load_script(service_id: &str) -> &'static str {
    match service_id {
        // Messenger requires this localStorage flag to enable desktop notifications
        "messenger" => r#"
(function() {
    if (window.__ferdirust_postload_done) return;
    window.__ferdirust_postload_done = true;
    try {
        localStorage.setItem('_cs_desktopNotifsEnabled', JSON.stringify({
            __t: Date.now(),
            __v: true
        }));
    } catch (e) {}
})();
"#,
        _ => "",
    }
}
