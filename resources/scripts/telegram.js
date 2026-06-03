// Telegram badge script — supports Web K, Web Z, and Web A
// Counts number of chats with unread badges (dot, not totals).
// Falls back to document.title parsing if DOM selectors return nothing.
// Emits ferdirust:badge:{direct,indirect} via console.log.
(function() {
    if (window.__ferdirust_badge_injected) return;
    window.__ferdirust_badge_injected = true;

    var lastDirect = -1;
    var lastIndirect = -1;

    function detectVersion() {
        var href = window.location.href || '';
        if (href.indexOf('/k/') !== -1) return 'k';
        if (href.indexOf('/a/') !== -1) return 'a';
        return 'z';
    }

    function parseTitleBadge() {
        var title = document.title || '';
        var match = title.match(/\((\d+)\)/);
        if (match) {
            return parseInt(match[1], 10) || 0;
        }
        return 0;
    }

    function countBadgesWebZ() {
        var direct = 0, indirect = 0, senders = [];

        // Private chats with unread badges
        var privates = document.querySelectorAll(
            '.chat-list .ListItem.Chat.private:not(.chat-item-archive)'
        );
        for (var i = 0; i < privates.length; i++) {
            var badge = privates[i].querySelector('.chat-badge-transition.shown');
            if (!badge) continue;
            var countEl = badge.querySelector('span');
            var count = countEl ? parseInt(countEl.textContent, 10) || 1 : 1;
            direct += count;
            var nameEl = privates[i].querySelector('.fullName');
            if (nameEl && nameEl.textContent && senders.length < 5) {
                senders.push(nameEl.textContent.trim());
            }
        }

        // Group chats with unread badges
        var groups = document.querySelectorAll(
            '.chat-list .ListItem.Chat.group:not(.chat-item-archive)'
        );
        for (var i = 0; i < groups.length; i++) {
            var badge = groups[i].querySelector('.chat-badge-transition.shown');
            if (!badge) continue;
            var countEl = badge.querySelector('span');
            var count = countEl ? parseInt(countEl.textContent, 10) || 1 : 1;
            indirect += count;
            var nameEl = groups[i].querySelector('.fullName');
            if (nameEl && nameEl.textContent && senders.length < 5) {
                senders.push(nameEl.textContent.trim());
            }
        }

        return { direct: direct, indirect: indirect, senders: senders };
    }

    function countBadgesWebA() {
        var direct = 0, indirect = 0, senders = [];

        var chats = document.querySelectorAll('.chat-list .Chat.chat-item-clickable');
        for (var i = 0; i < chats.length; i++) {
            var chat = chats[i];

            // Skip muted chats
            if (chat.querySelector('.info-row .icon-muted')) continue;

            // Find visible badge in subtitle row
            var badge = chat.querySelector('.subtitle .chat-badge-transition.shown');
            if (!badge) continue;

            // Get inner div (the actual badge element)
            var badgeInner = badge.querySelector('div');
            if (!badgeInner) continue;

            // Skip pinned badges (they contain an icon, not a count)
            if (badgeInner.querySelector('i')) continue;

            var countText = (badgeInner.textContent || '').trim();
            var count = 1;
            if (countText) {
                if (countText.indexOf('K') !== -1) {
                    count = Math.round(parseFloat(countText) * 1000) || 1;
                } else {
                    count = parseInt(countText, 10) || 1;
                }
            }

            if (chat.classList.contains('private')) {
                direct += count;
                var nameEl = chat.querySelector('h3.fullName');
                if (nameEl && nameEl.textContent && senders.length < 5) {
                    senders.push(nameEl.textContent.trim());
                }
            } else {
                indirect += count;
                var nameEl = chat.querySelector('h3.fullName');
                if (nameEl && nameEl.textContent && senders.length < 5) {
                    senders.push(nameEl.textContent.trim());
                }
            }
        }

        return { direct: direct, indirect: indirect, senders: senders };
    }

    function countBadgesWebK() {
        var direct = 0, indirect = 0, senders = [];

        var elements = document.querySelectorAll('.rp:not(.is-muted)');
        for (var i = 0; i < elements.length; i++) {
            var badge = elements[i].querySelector('.dialog-subtitle-badge');
            if (!badge) continue;

            var peerId = elements[i].dataset.peerId || elements[i].dataset.peerid || '';
            if (parseInt(peerId, 10) > 0) {
                direct++;
            } else {
                indirect++;
            }
            var nameEl = elements[i].querySelector('.peer-title');
            if (nameEl && nameEl.textContent && senders.length < 5) {
                senders.push(nameEl.textContent.trim());
            }
        }

        return { direct: direct, indirect: indirect, senders: senders };
    }

    function checkBadges() {
        try {
            var version = detectVersion();
            var counts;
            if (version === 'k') {
                counts = countBadgesWebK();
            } else if (version === 'a') {
                counts = countBadgesWebA();
            } else {
                counts = countBadgesWebZ();
            }

            // Title fallback: if DOM selectors found nothing, parse document.title
            if (counts.direct === 0 && counts.indirect === 0) {
                var titleCount = parseTitleBadge();
                if (titleCount > 0) {
                    counts.direct = titleCount;
                    counts.senders = [];
                }
            }

            if (counts.direct !== lastDirect || counts.indirect !== lastIndirect) {
                lastDirect = counts.direct;
                lastIndirect = counts.indirect;
                var payload = {
                    direct: counts.direct,
                    indirect: counts.indirect
                };
                if (counts.senders && counts.senders.length > 0) {
                    payload.senders = counts.senders;
                }
                console.log('ferdirust:badge:' + JSON.stringify(payload));
            }
        } catch (e) {}
    }

    // Poll every 2 seconds (reliable for React re-renders)
    setInterval(checkBadges, 2000);

    // Also observe DOM for immediate updates
    var observer = new MutationObserver(checkBadges);
    observer.observe(document.body, { childList: true, subtree: true, characterData: true });
    checkBadges();
})();
