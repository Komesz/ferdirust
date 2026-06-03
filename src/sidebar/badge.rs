use std::collections::HashMap;
use std::time::Instant;

const NOTIFY_COOLDOWN_SECS: u64 = 0;

#[derive(Debug, Clone, Copy, Default)]
pub struct BadgeCounts {
    pub direct: u32,
    pub indirect: u32,
}

impl BadgeCounts {
    pub fn total(&self) -> u32 {
        self.direct + self.indirect
    }
}

pub struct BadgeState {
    counts: HashMap<String, BadgeCounts>,
    /// Tracks the direct count we last notified for, plus when.
    notified: HashMap<String, (u32, Instant)>,
    /// Whether a service has unread activity since the user last viewed it.
    unread: HashMap<String, bool>,
    /// The service the user is currently viewing — suppresses unread marking.
    active_service: Option<String>,
}

impl BadgeState {
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
            notified: HashMap::new(),
            unread: HashMap::new(),
            active_service: None,
        }
    }

    /// Sets direct/indirect counts. Returns (counts, should_notify).
    /// Only signals to notify when direct count increases above the last
    /// notified peak, with a cooldown to handle title blinking.
    pub fn set_badge(&mut self, service_id: &str, direct: u32, indirect: u32) -> (BadgeCounts, bool) {
        let prev = self.counts
            .insert(service_id.to_string(), BadgeCounts { direct, indirect })
            .unwrap_or_default();

        // Mark as unread if any count increased (but not for the active tab)
        if (direct > prev.direct || indirect > prev.indirect)
            && self.active_service.as_deref() != Some(service_id)
        {
            self.unread.insert(service_id.to_string(), true);
        }

        let should_notify = if direct > 0 && direct > prev.direct {
            let now = Instant::now();
            match self.notified.get(service_id) {
                Some(&(peak, at)) => {
                    if direct > peak {
                        true
                    } else if now.duration_since(at).as_secs() >= NOTIFY_COOLDOWN_SECS {
                        direct > prev.direct
                    } else {
                        false
                    }
                }
                None => true,
            }
        } else {
            false
        };

        if should_notify {
            self.notified.insert(service_id.to_string(), (direct, Instant::now()));
        }

        (prev, should_notify)
    }

    /// Convenience wrapper: sets count as direct, indirect=0. For title-change fallback.
    pub fn set_count(&mut self, service_id: &str, count: u32) -> (BadgeCounts, bool) {
        self.set_badge(service_id, count, 0)
    }

    /// Increment the direct count by 1 (for notification-based badge bumps).
    /// Returns the new counts.
    pub fn increment(&mut self, service_id: &str) -> BadgeCounts {
        let counts = self.counts.entry(service_id.to_string()).or_default();
        counts.direct += 1;
        if self.active_service.as_deref() != Some(service_id) {
            self.unread.insert(service_id.to_string(), true);
        }
        *counts
    }

    /// Mark a service as having unread activity (without changing counts).
    /// Used as a fallback when title shows a badge but the service has a custom badge script.
    pub fn mark_unread(&mut self, service_id: &str) {
        if self.active_service.as_deref() != Some(service_id) {
            self.unread.insert(service_id.to_string(), true);
        }
    }

    /// Mark a service as viewed — clears the red dot.
    pub fn mark_viewed(&mut self, service_id: &str) {
        self.unread.insert(service_id.to_string(), false);
    }

    /// Set the active (currently viewed) service.
    /// Clears the unread dot and suppresses future unread marking for this service.
    pub fn set_active_service(&mut self, service_id: &str) {
        self.active_service = Some(service_id.to_string());
        self.unread.insert(service_id.to_string(), false);
    }

    /// Returns the currently active service ID (for debug logging).
    pub fn active_service_id(&self) -> Option<&str> {
        self.active_service.as_deref()
    }

    /// Returns the notified peak direct count for a service (for debug logging).
    pub fn notified_peak(&self, service_id: &str) -> Option<u32> {
        self.notified.get(service_id).map(|&(peak, _)| peak)
    }

    #[allow(dead_code)]
    pub fn get_counts(&self, service_id: &str) -> BadgeCounts {
        self.counts.get(service_id).copied().unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn total_count(&self) -> u32 {
        self.counts.values().map(|c| c.total()).sum()
    }

    /// Returns JSON with boolean unread state per service (for sidebar red dot).
    pub fn to_json(&self) -> String {
        let entries: Vec<String> = self
            .unread
            .iter()
            .map(|(k, v)| format!("\"{}\":{}", k, v))
            .collect();
        format!("{{{}}}", entries.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_badge_counts_default() {
        let c = BadgeCounts::default();
        assert_eq!(c.direct, 0);
        assert_eq!(c.indirect, 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn test_set_badge_direct_notifies() {
        let mut state = BadgeState::new();
        let (prev, notify) = state.set_badge("test", 3, 0);
        assert_eq!(prev.direct, 0);
        assert!(notify);
    }

    #[test]
    fn test_set_badge_indirect_only_no_notify() {
        let mut state = BadgeState::new();
        let (_prev, notify) = state.set_badge("test", 0, 5);
        assert!(!notify);
    }

    #[test]
    fn test_set_count_wrapper() {
        let mut state = BadgeState::new();
        let (prev, notify) = state.set_count("test", 3);
        assert_eq!(prev.direct, 0);
        assert!(notify);
        assert_eq!(state.get_counts("test").direct, 3);
        assert_eq!(state.get_counts("test").indirect, 0);
    }

    #[test]
    fn test_increment() {
        let mut state = BadgeState::new();
        let counts = state.increment("test");
        assert_eq!(counts.direct, 1);
        assert_eq!(counts.indirect, 0);
        let counts = state.increment("test");
        assert_eq!(counts.direct, 2);
    }

    #[test]
    fn test_to_json() {
        let mut state = BadgeState::new();
        state.set_badge("svc1", 3, 2);
        let json = state.to_json();
        assert!(json.contains("\"svc1\":true"));
    }

    #[test]
    fn test_mark_viewed_clears_unread() {
        let mut state = BadgeState::new();
        state.set_badge("test", 3, 0);
        assert!(state.to_json().contains("\"test\":true"));
        state.mark_viewed("test");
        assert!(state.to_json().contains("\"test\":false"));
    }

    #[test]
    fn test_mark_unread() {
        let mut state = BadgeState::new();
        // Initially no unread
        assert!(!state.to_json().contains("\"test\":true"));
        // mark_unread sets the red dot without changing counts
        state.mark_unread("test");
        assert!(state.to_json().contains("\"test\":true"));
        assert_eq!(state.get_counts("test").direct, 0);
        assert_eq!(state.get_counts("test").indirect, 0);
        // mark_viewed clears it
        state.mark_viewed("test");
        assert!(state.to_json().contains("\"test\":false"));
    }

    #[test]
    fn test_set_active_service_suppresses_unread() {
        let mut state = BadgeState::new();
        state.set_active_service("slack");

        // Badge updates for the active service should NOT set unread
        state.set_badge("slack", 5, 0);
        assert!(state.to_json().contains("\"slack\":false"));

        state.increment("slack");
        assert!(state.to_json().contains("\"slack\":false"));

        state.mark_unread("slack");
        assert!(state.to_json().contains("\"slack\":false"));

        // But a different service should still get unread
        state.set_badge("telegram", 1, 0);
        assert!(state.to_json().contains("\"telegram\":true"));
    }

    #[test]
    fn test_active_service_still_notifies() {
        let mut state = BadgeState::new();
        state.set_active_service("slack");

        // should_notify should still be true even for the active service
        let (_prev, notify) = state.set_badge("slack", 3, 0);
        assert!(notify);
    }

    #[test]
    fn test_switching_active_service() {
        let mut state = BadgeState::new();
        state.set_active_service("slack");
        state.set_badge("slack", 3, 0);
        assert!(state.to_json().contains("\"slack\":false"));

        // Switch to telegram — now slack should accumulate unread
        state.set_active_service("telegram");
        state.set_badge("slack", 5, 0);
        assert!(state.to_json().contains("\"slack\":true"));
        assert!(state.to_json().contains("\"telegram\":false"));
    }

    #[test]
    fn test_no_notify_on_same_count() {
        let mut state = BadgeState::new();
        state.set_badge("test", 3, 0);
        let (_prev, notify) = state.set_badge("test", 3, 0);
        assert!(!notify);
    }

    #[test]
    fn test_no_notify_on_decrease() {
        let mut state = BadgeState::new();
        state.set_badge("test", 5, 0);
        let (_prev, notify) = state.set_badge("test", 2, 0);
        assert!(!notify);
    }
}
