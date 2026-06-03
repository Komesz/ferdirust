// Proton Mail badge script
// Only shows unread count for messages arriving after app start.
// All counts are direct. Emits ferdirust:badge:{direct,indirect} via console.log.
(function() {
    if (window.__ferdirust_badge_injected) return;
    window.__ferdirust_badge_injected = true;

    var baseline = null;
    var lastCount = -1;

    function getRawCount() {
        // Try DOM counter first
        var counter = document.querySelector('.navigation-counter-item');
        if (counter) {
            var n = parseInt(counter.textContent, 10);
            if (!isNaN(n)) return n;
        }
        // Fallback: parse from title "Inbox (N) - Proton Mail"
        var match = document.title.match(/\((\d+)\)/);
        if (match) {
            var n = parseInt(match[1], 10);
            if (!isNaN(n)) return n;
        }
        return 0;
    }

    function checkBadge() {
        try {
            var raw = getRawCount();

            // Capture baseline on first nonzero read (after page finishes loading)
            if (baseline === null) {
                if (raw > 0) baseline = raw;
                return; // don't emit until baseline is set
            }

            var newCount = Math.max(0, raw - baseline);

            if (newCount !== lastCount) {
                lastCount = newCount;

                // Best-effort: extract sender names from unread inbox items
                var senders = [];
                var unreadItems = document.querySelectorAll('.item-container--unread .item-senders');
                for (var i = 0; i < unreadItems.length && senders.length < 5; i++) {
                    var name = unreadItems[i].textContent.trim();
                    if (name && senders.indexOf(name) === -1) {
                        senders.push(name);
                    }
                }

                var payload = {
                    direct: newCount,
                    indirect: 0
                };
                if (senders.length > 0) {
                    payload.senders = senders;
                }
                console.log('ferdirust:badge:' + JSON.stringify(payload));
            }
        } catch (e) {}
    }

    var observer = new MutationObserver(checkBadge);
    observer.observe(document.body, { childList: true, subtree: true, characterData: true });
    checkBadge();
})();
