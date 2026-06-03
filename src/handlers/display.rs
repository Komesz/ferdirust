use cef::*;
use std::process::Command;
use std::sync::{Arc, Mutex};

use crate::app::SharedServiceManager;
use crate::handlers::load::has_badge_script;
use crate::sidebar::badge::BadgeState;

wrap_display_handler! {
    pub struct FerdiDisplayHandler {
        pub service_id: String,
        pub badge_state: Arc<Mutex<BadgeState>>,
        pub service_manager: SharedServiceManager,
    }

    impl DisplayHandler {
        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let Some(title) = title {
                let title_str = title.to_string();

                let count = parse_badge_from_title(&title_str);

                // Services with custom badge scripts use console.log IPC for
                // full direct/indirect badge counts. But if their DOM selectors
                // break, we'd get zero badge detection. As a fallback, still
                // parse the title — if count > 0, mark unread (red dot) without
                // triggering notifications (to avoid conflicts with the script).
                if has_badge_script(&self.service_id) {
                    if count > 0 {
                        let json = {
                            let mut state = match self.badge_state.lock() {
                                Ok(s) => s,
                                Err(_) => return,
                            };
                            state.mark_unread(&self.service_id);
                            state.to_json()
                        };
                        if let Ok(guard) = self.service_manager.lock() {
                            if let Some(mgr) = guard.as_ref() {
                                mgr.update_sidebar_badges_json(&json);
                            }
                        }
                    }
                    return;
                }

                // Update badge state (release lock before accessing service_manager)
                let (should_notify, json) = {
                    let mut state = match self.badge_state.lock() {
                        Ok(s) => s,
                        Err(_) => return,
                    };
                    let (_prev, notify) = state.set_count(&self.service_id, count);
                    (notify, state.to_json())
                };

                // Push updated badges to the sidebar
                if let Ok(guard) = self.service_manager.lock() {
                    if let Some(mgr) = guard.as_ref() {
                        mgr.update_sidebar_badges_json(&json);
                    }
                }

                // Send desktop notification (debounced by BadgeState)
                if should_notify {
                    let service_id = self.service_id.clone();
                    let title_clean = title_str
                        .trim_start_matches(|c: char| c == '(' || c.is_ascii_digit() || c == ')' || c == ' ')
                        .to_string();
                    std::thread::spawn(move || {
                        let summary = format!("[{}] New message", service_id);
                        let body = format!("New notification from {}", title_clean);
                        let _ = Command::new("notify-send")
                            .arg("--app-name=Ferdirust")
                            .arg(&summary)
                            .arg(&body)
                            .spawn();
                        play_notification_sound();
                    });
                }
            }
        }

        fn on_console_message(
            &self,
            _browser: Option<&mut Browser>,
            _level: LogSeverity,
            message: Option<&CefString>,
            _source: Option<&CefString>,
            _line: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            if let Some(msg) = message {
                let msg_str = msg.to_string();

                // Handle badge updates from service scripts
                if let Some(json) = msg_str.strip_prefix("ferdirust:badge:") {
                    println!("[{}] badge raw: {}", self.service_id, json);
                    if let Some((direct, indirect, senders)) = parse_badge_json(json) {
                        let (should_notify, badge_json) = {
                            let mut state = match self.badge_state.lock() {
                                Ok(s) => s,
                                Err(_) => return 1,
                            };
                            let (prev, notify) = state.set_badge(&self.service_id, direct, indirect);
                            println!(
                                "[{}] badge: {} | direct={} indirect={} senders={:?} | prev_direct={} prev_indirect={} | notify={} | peak={:?} | active={:?}",
                                self.service_id, json, direct, indirect, senders,
                                prev.direct, prev.indirect, notify,
                                state.notified_peak(&self.service_id),
                                state.active_service_id()
                            );
                            (notify, state.to_json())
                        };
                        if let Ok(guard) = self.service_manager.lock() {
                            if let Some(mgr) = guard.as_ref() {
                                mgr.update_sidebar_badges_json(&badge_json);
                            }
                        }
                        if should_notify {
                            let service_id = self.service_id.clone();
                            std::thread::spawn(move || {
                                let summary = format!("[{}] New message", service_id);
                                let body = if let Some(name) = senders.first() {
                                    format!("From: {}", name)
                                } else {
                                    format!("{} new direct message(s)", direct)
                                };
                                let _ = Command::new("notify-send")
                                    .arg("--app-name=Ferdirust")
                                    .arg(&summary)
                                    .arg(&body)
                                    .spawn();
                                play_notification_sound();
                            });
                        }
                    }
                    return 1;
                }

                // Handle notification messages
                if let Some(json) = msg_str.strip_prefix("ferdirust:notify:") {
                    send_notification(json);

                    // Also bump the badge count for this service
                    let badge_json = {
                        let mut state = match self.badge_state.lock() {
                            Ok(s) => s,
                            Err(_) => return 1,
                        };
                        state.increment(&self.service_id);
                        state.to_json()
                    };
                    if let Ok(guard) = self.service_manager.lock() {
                        if let Some(mgr) = guard.as_ref() {
                            mgr.update_sidebar_badges_json(&badge_json);
                        }
                    }

                    return 1; // handled, suppress from console
                }
            }
            0
        }

        fn on_favicon_urlchange(
            &self,
            _browser: Option<&mut Browser>,
            icon_urls: Option<&mut CefStringList>,
        ) {
            if let Some(urls) = icon_urls {
                // Get first favicon URL from the list
                let url_list: Vec<String> = std::mem::take(urls).into_iter().collect();
                if let Some(favicon_url) = url_list.first() {
                    if let Ok(guard) = self.service_manager.lock() {
                        if let Some(mgr) = guard.as_ref() {
                            mgr.update_sidebar_favicon(&self.service_id, favicon_url);
                        }
                    }
                }
            }
        }
    }
}

impl FerdiDisplayHandler {
    pub fn create(
        service_id: String,
        badge_state: Arc<Mutex<BadgeState>>,
        service_manager: SharedServiceManager,
    ) -> DisplayHandler {
        FerdiDisplayHandler::new(service_id, badge_state, service_manager)
    }
}

/// Parse badge JSON from console.log message: {"direct":N,"indirect":N,"senders":["Alice"]}
fn parse_badge_json(json: &str) -> Option<(u32, u32, Vec<String>)> {
    #[derive(serde::Deserialize)]
    struct BadgePayload {
        direct: Option<u32>,
        indirect: Option<u32>,
        senders: Option<Vec<String>>,
    }
    let payload: BadgePayload = serde_json::from_str(json).ok()?;
    Some((
        payload.direct.unwrap_or(0),
        payload.indirect.unwrap_or(0),
        payload.senders.unwrap_or_default(),
    ))
}

/// Send a desktop notification via notify-send.
/// Expects JSON like: {"service":"messenger","title":"New message","body":"Hello"}
fn send_notification(json: &str) {
    #[derive(serde::Deserialize)]
    struct NotifPayload {
        service: Option<String>,
        title: Option<String>,
        body: Option<String>,
    }

    let payload: NotifPayload = match serde_json::from_str(json) {
        Ok(p) => p,
        Err(_) => return,
    };

    let summary = match (&payload.service, &payload.title) {
        (Some(svc), Some(title)) => format!("[{}] {}", svc, title),
        (None, Some(title)) => title.clone(),
        (Some(svc), None) => format!("[{}] New notification", svc),
        (None, None) => "New notification".to_string(),
    };

    let body = payload.body.unwrap_or_default();

    std::thread::spawn(move || {
        let _ = Command::new("notify-send")
            .arg("--app-name=Ferdirust")
            .arg(&summary)
            .arg(&body)
            .spawn();
        play_notification_sound();
    });
}

/// Play the freedesktop notification sound.
fn play_notification_sound() {
    let _ = Command::new("paplay")
        .arg("/usr/share/sounds/freedesktop/stereo/message-new-instant.oga")
        .spawn();
}

/// Parse badge count from title strings like "(3) Messenger" or "Telegram Web (3)"
fn parse_badge_from_title(title: &str) -> u32 {
    // Try "(N) ..." at the start
    if title.starts_with('(') {
        if let Some(end) = title.find(')') {
            if let Ok(n) = title[1..end].parse::<u32>() {
                return n;
            }
        }
    }
    // Try "... (N)" at the end
    if title.ends_with(')') {
        if let Some(start) = title.rfind('(') {
            if let Ok(n) = title[start + 1..title.len() - 1].parse::<u32>() {
                return n;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_badge() {
        assert_eq!(parse_badge_from_title("(3) Messenger"), 3);
        assert_eq!(parse_badge_from_title("(12) Slack"), 12);
        assert_eq!(parse_badge_from_title("Messenger"), 0);
        assert_eq!(parse_badge_from_title("(0) Test"), 0);
        assert_eq!(parse_badge_from_title(""), 0);
        // Telegram-style: count at end
        assert_eq!(parse_badge_from_title("Telegram Web (5)"), 5);
        assert_eq!(parse_badge_from_title("Telegram Web (0)"), 0);
    }

    #[test]
    fn test_parse_badge_json() {
        assert_eq!(parse_badge_json(r#"{"direct":3,"indirect":5}"#), Some((3, 5, vec![])));
        assert_eq!(parse_badge_json(r#"{"direct":0,"indirect":0}"#), Some((0, 0, vec![])));
        assert_eq!(parse_badge_json(r#"{"direct":1}"#), Some((1, 0, vec![])));
        assert_eq!(parse_badge_json(r#"{}"#), Some((0, 0, vec![])));
        assert_eq!(parse_badge_json("invalid"), None);
    }

    #[test]
    fn test_parse_badge_json_with_senders() {
        assert_eq!(
            parse_badge_json(r#"{"direct":2,"indirect":0,"senders":["Alice","Bob"]}"#),
            Some((2, 0, vec!["Alice".to_string(), "Bob".to_string()]))
        );
        assert_eq!(
            parse_badge_json(r#"{"direct":1,"indirect":0,"senders":[]}"#),
            Some((1, 0, vec![]))
        );
        // senders field is optional — omitting it gives empty vec
        assert_eq!(
            parse_badge_json(r#"{"direct":1,"indirect":0}"#),
            Some((1, 0, vec![]))
        );
    }
}
