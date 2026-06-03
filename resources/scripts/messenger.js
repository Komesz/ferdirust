// Messenger badge script
// Uses Ferdium's proven selectors for unread message detection.
// All Messenger counts are direct (DM platform).
// Emits ferdirust:badge:{direct,indirect} via console.log.
(function() {
    if (window.__ferdirust_badge_injected) return;
    window.__ferdirust_badge_injected = true;

    var lastCount = -1;

    function safeParseInt(text) {
        var n = parseInt(String(text), 10);
        if (isNaN(n) || n < 0) return 0;
        return n;
    }

    function parseTitleBadge() {
        var title = document.title || '';
        var match = title.match(/\((\d+)\)/);
        if (match) {
            return parseInt(match[1], 10) || 0;
        }
        return 0;
    }

    function checkBadge() {
        try {
            var count = 0;
            var newUI = false;
            var senders = [];

            // New Messenger UI: count from aria-label on sidebar links
            var hrefs = ['/', '/requests/', '/marketplace/'];
            for (var i = 0; i < hrefs.length; i++) {
                var elem = document.querySelector(
                    "a[href^='" + hrefs[i] + "t/'][role='link'][tabindex='0']"
                );
                if (elem && elem.ariaLabel) {
                    newUI = true;
                    var match = elem.ariaLabel.match(/(\d+)/g);
                    if (match) {
                        count += safeParseInt(match[0]);
                    }
                }
            }

            // Best-effort: extract names from unread chat rows (bold = unread)
            if (newUI) {
                var rows = document.querySelectorAll('a[role="link"][tabindex="0"][href*="/t/"]');
                for (var k = 0; k < rows.length && senders.length < 5; k++) {
                    // Unread chats typically have a bold/semibold font-weight on the name span
                    var nameSpan = rows[k].querySelector('span[dir="auto"] span');
                    if (!nameSpan) continue;
                    var weight = getComputedStyle(nameSpan).fontWeight;
                    if (weight === 'bold' || weight === '600' || weight === '700' || weight === '800') {
                        var name = nameSpan.textContent.trim();
                        if (name && senders.indexOf(name) === -1) {
                            senders.push(name);
                        }
                    }
                }
            }

            // Old Messenger UI fallback
            if (!newUI) {
                var items = document.querySelectorAll(
                    '.bp9cbjyn.j83agx80.owycx6da:not(.btwxx1t3)'
                );
                for (var j = 0; j < items.length; j++) {
                    var hasPing = !!items[j].querySelector(
                        '.pq6dq46d.is6700om.qu0x051f.esr5mh6w.e9989ue4.r7d6kgcz.s45kfl79.emlxlaya.bkmhp75w.spb7xbtv.cyypbtt7.fwizqjfa'
                    );
                    var isMuted = !!items[j].querySelector(
                        '.a8c37x1j.ms05siws.l3qrxjdp.b7h9ocf4.trssfv1o'
                    );
                    if (hasPing && !isMuted) count++;
                }

                // Message requests count
                var requestsEl = document.querySelector('._5nxf');
                if (requestsEl) {
                    count += safeParseInt(requestsEl.textContent);
                }
            }

            // Title fallback: if DOM selectors found nothing, parse document.title
            if (count === 0) {
                count = parseTitleBadge();
            }

            if (count !== lastCount) {
                lastCount = count;
                var payload = {
                    direct: count,
                    indirect: 0
                };
                if (senders.length > 0) {
                    payload.senders = senders;
                }
                console.log('ferdirust:badge:' + JSON.stringify(payload));
            }
        } catch (e) {}
    }

    // Poll every 2 seconds (reliable for React re-renders)
    setInterval(checkBadge, 2000);

    // Also observe DOM for immediate updates
    var observer = new MutationObserver(checkBadge);
    observer.observe(document.body, { childList: true, subtree: true, characterData: true });
    checkBadge();
})();
