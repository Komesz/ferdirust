// Slack badge + team icon script
// Uses Ferdium's proven selectors for unread channel detection.
// Emits ferdirust:badge:{direct,indirect} via console.log.
(function() {
    if (window.__ferdirust_badge_injected) return;
    window.__ferdirust_badge_injected = true;

    var UNREAD = '.p-channel_sidebar__channel--unread:not(.p-channel_sidebar__channel--muted)';
    var lastDirect = -1;
    var lastIndirect = -1;

    function checkBadge() {
        try {
            // Direct: channels with mention badges + unread sidebar links (excl nav items)
            var direct = document.querySelectorAll(
                UNREAD + ' .p-channel_sidebar__badge, ' +
                '.p-channel_sidebar__link--unread' +
                ':not([data-sidebar-link-id="Punreads"])' +
                ':not([data-sidebar-link-id="Pdrafts"])' +
                ':not([data-sidebar-link-id="Pdms"])' +
                ':not([data-sidebar-link-id="Ppaid-benefits"])'
            ).length;

            // Indirect: remaining unread channels without mention badges
            var allUnread = document.querySelectorAll(UNREAD).length;
            var indirect = Math.max(0, allUnread - direct);

            // Extract channel/DM names from unread items with badges
            var senders = [];
            var badgeEls = document.querySelectorAll(UNREAD + ' .p-channel_sidebar__badge');
            for (var i = 0; i < badgeEls.length && senders.length < 5; i++) {
                var ch = badgeEls[i].closest('.p-channel_sidebar__channel');
                if (!ch) continue;
                var nameEl = ch.querySelector('.p-channel_sidebar__name');
                if (nameEl && nameEl.textContent) {
                    senders.push(nameEl.textContent.trim());
                }
            }

            if (direct !== lastDirect || indirect !== lastIndirect) {
                lastDirect = direct;
                lastIndirect = indirect;
                var payload = {
                    direct: direct,
                    indirect: indirect
                };
                if (senders.length > 0) {
                    payload.senders = senders;
                }
                console.log('ferdirust:badge:' + JSON.stringify(payload));
            }
        } catch (e) {}
    }

    // Extract Slack workspace team icon and set it as the page favicon
    function extractTeamIcon() {
        try {
            var el = document.querySelector('.c-team_icon');
            if (!el) return;
            var bg = getComputedStyle(el).backgroundImage;
            if (!bg || bg === 'none') return;
            var match = bg.match(/url\(["']?(.*?)["']?\)/);
            if (!match || !match[1]) return;
            var iconUrl = match[1];
            if (window.__ferdirust_last_team_icon === iconUrl) return;
            window.__ferdirust_last_team_icon = iconUrl;

            // Replace the page favicon with the team icon
            var link = document.querySelector('link[rel*="icon"][data-ferdirust]');
            if (!link) {
                link = document.createElement('link');
                link.rel = 'icon';
                link.setAttribute('data-ferdirust', '1');
                document.head.appendChild(link);
            }
            link.href = iconUrl;
        } catch (e) {}
    }

    // Poll every 2 seconds (reliable for Slack's React re-renders)
    setInterval(checkBadge, 2000);

    // Also observe DOM for immediate updates
    var observer = new MutationObserver(function() {
        checkBadge();
        extractTeamIcon();
    });
    observer.observe(document.body, { childList: true, subtree: true, characterData: true });

    setTimeout(extractTeamIcon, 2000);
    checkBadge();
})();
