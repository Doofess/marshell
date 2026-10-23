//! The rules for when a banner, a sound or a taskbar flash is wanted. Pure: it is handed snapshots, the
//! open permission holds, the settings, the time of day and an `Instant`, and returns what to tell the
//! user. Nothing here reads a clock or sleeps, so the tests drive time by hand.

use super::copy::{self, Kind, Who};
use super::{Banner, NotifyRequest, SoundKind};
use marshell_protocol::api::{
    LiveState, NotifyChannel, NotifySettings, QuietHours, SessionInfo, SessionState, SessionsSnapshot, UiView,
};
use marshell_protocol::hook::PermissionView;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// A Done is only worth a banner for a tab that has been out of sight for longer than this.
const DONE_MIN_BACKGROUND: Duration = Duration::from_secs(30);
/// Events of one category that follow the first within this window fold into one banner.
const BATCH_WINDOW: Duration = Duration::from_secs(3);
/// A category plays at most one sound in this time.
const SOUND_GAP: Duration = Duration::from_secs(3);
/// A wait that nobody has looked at for this long gets one reminder.
const ESCALATE_AFTER: Duration = Duration::from_secs(60);
/// The status folds before the hold of a permission request opens; a wait is told once its hold is
/// there (so a risky command is known to be risky) or, with no hold, after this.
const HOLD_WAIT: Duration = Duration::from_millis(500);
/// The most subagent events one look can make, so a counter that jumped cannot flood.
const MAX_SUBAGENT_EVENTS: u32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Cat {
    NeedsYou,
    Done,
    Error,
    Subagent,
}

impl Cat {
    fn sound(self) -> SoundKind {
        match self {
            Cat::NeedsYou => SoundKind::NeedsYou,
            Cat::Done => SoundKind::Done,
            Cat::Error => SoundKind::Error,
            Cat::Subagent => SoundKind::SubagentDone,
        }
    }

    /// The kinds that fold into one banner; a finished subagent never has one.
    fn kind(self) -> Option<Kind> {
        match self {
            Cat::NeedsYou => Some(Kind::NeedsYou),
            Cat::Done => Some(Kind::Done),
            Cat::Error => Some(Kind::Error),
            Cat::Subagent => None,
        }
    }
}

/// What a tab was last seen to be doing, for telling a new event from the same one seen again.
struct Live {
    state: LiveState,
    phrase: String,
    since_ms: u64,
    finished: u32,
}

/// One wait for the user: a permission request, a question or terminal attention.
struct Wait {
    permission: bool,
    terminal: bool,
    since_ms: u64,
    /// The holds seen for this wait. A hold not in here is another request.
    rids: HashSet<String>,
    risky: bool,
    told: bool,
    /// Its initial delivery was pending during a pause; only its reminder may still go out.
    silenced: bool,
    /// When a permission with no hold yet is told anyway.
    deadline: Instant,
}

struct Tab {
    who: Who,
    muted: bool,
    live: Option<Live>,
    wait: Option<Wait>,
    done_since: Option<u64>,
    error_since: Option<u64>,
    /// When the engine first saw the tab (or started): how long it has been out of sight, with no view.
    born: Instant,
    /// The last moment the tab was in view (window focused, tab visible).
    last_seen: Option<Instant>,
}

impl Tab {
    fn new(born: Instant) -> Tab {
        Tab {
            who: Who {
                tab_id: String::new(),
                number: 0,
                name: String::new(),
                project: String::new(),
            },
            muted: false,
            live: None,
            wait: None,
            done_since: None,
            error_since: None,
            born,
            last_seen: None,
        }
    }

    /// The row has no live status or attention to follow (an ended process, or none reported).
    fn clear(&mut self) {
        self.live = None;
        self.wait = None;
        self.done_since = None;
        self.error_since = None;
    }
}

/// What an event is about, kept so a batch can be worded and checked later.
#[derive(Clone)]
struct Entry {
    who: Who,
    /// The wait's kind and `since_ms`, which tell this event from another of the same tab.
    permission: bool,
    terminal: bool,
    since_ms: u64,
    phrase: String,
    recap: Option<String>,
    risky: bool,
    muted: bool,
}

struct Event {
    cat: Cat,
    entry: Entry,
}

/// Events of one category since the first went out, until the window ends.
#[derive(Default)]
struct Window {
    until: Option<Instant>,
    entries: Vec<Entry>,
}

struct Escalation {
    armed: Instant,
    who: Who,
    permission: bool,
    terminal: bool,
    since_ms: u64,
    risky: bool,
}

pub struct Engine {
    started: Instant,
    seeded: bool,
    view: UiView,
    tabs: HashMap<String, Tab>,
    windows: HashMap<Cat, Window>,
    last_sound: HashMap<Cat, Instant>,
    escalations: HashMap<String, Escalation>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Engine {
        Self::starting_at(Instant::now())
    }

    /// An engine that started at `started`. With no view ever reported, every tab has been out of sight since then.
    pub fn starting_at(started: Instant) -> Engine {
        Engine {
            started,
            seeded: false,
            view: UiView {
                window_focused: false,
                visible_tabs: Vec::new(),
            },
            tabs: HashMap::new(),
            windows: HashMap::new(),
            last_sound: HashMap::new(),
            escalations: HashMap::new(),
        }
    }

    /// The UI reported what it shows. The old view held until now, so tabs it showed count as seen up to now.
    pub fn set_view(&mut self, view: UiView, now: Instant) {
        self.note_seen(now);
        self.view = view;
        self.note_seen(now);
    }

    /// Looks at a snapshot of the sessions. The first look only records, so a restart replays nothing.
    pub fn observe(
        &mut self,
        snap: &SessionsSnapshot,
        holds: &[PermissionView],
        settings: &NotifySettings,
        minute_of_day: u16,
        now: Instant,
    ) -> Vec<NotifyRequest> {
        self.note_seen(now);
        let seeding = !self.seeded;
        self.seeded = true;
        let open: HashSet<&str> = snap.sessions.iter().map(|s| s.tab_id.as_str()).collect();
        self.tabs.retain(|id, _| open.contains(id.as_str()));
        let born = if seeding { self.started } else { now };
        let mut events = Vec::new();
        for info in &snap.sessions {
            let tab = self.tabs.entry(info.tab_id.clone()).or_insert_with(|| Tab::new(born));
            tab.who = Who::of(info);
            tab.muted = info.muted;
            follow(tab, info, holds, seeding, now, &mut events);
        }
        // A wait that ended, or was replaced by another, has nothing left to remind about.
        let tabs = &self.tabs;
        self.escalations.retain(|id, e| {
            tabs.get(id)
                .and_then(|t| t.wait.as_ref())
                .is_some_and(|w| w.permission == e.permission && w.terminal == e.terminal && w.since_ms == e.since_ms)
        });
        let mut out = Vec::new();
        // Windows that ended before these events started are over; these start new ones.
        self.flush(settings, minute_of_day, now, &mut out);
        for event in events {
            self.emit(event, settings, minute_of_day, now, &mut out);
        }
        out
    }

    /// Time passing: ends batch windows, tells permissions that waited for a hold that never came, and
    /// sends an escalation that is due.
    pub fn tick(&mut self, settings: &NotifySettings, minute_of_day: u16, now: Instant) -> Vec<NotifyRequest> {
        self.note_seen(now);
        let mut out = Vec::new();
        self.flush(settings, minute_of_day, now, &mut out);
        let mut due = Vec::new();
        for tab in self.tabs.values_mut() {
            let (Some(wait), Some(live)) = (tab.wait.as_mut(), tab.live.as_ref()) else {
                continue;
            };
            if !wait.told && now >= wait.deadline {
                wait.told = true;
                due.push(needs_you_event(&tab.who, tab.muted, wait, live, wait.risky));
            }
        }
        for event in due {
            self.emit(event, settings, minute_of_day, now, &mut out);
        }
        self.escalate(settings, minute_of_day, now, &mut out);
        out
    }

    /// A pause drops deferred deliveries too, without losing the wait that can arm a reminder.
    pub(super) fn discard_pending(&mut self) {
        self.windows.clear();
        for tab in self.tabs.values_mut() {
            if let Some(wait) = tab.wait.as_mut()
                && !wait.told
            {
                wait.silenced = true;
            }
        }
    }

    /// Tabs in view now count as seen at `now`, and a reminder for one is not needed.
    fn note_seen(&mut self, now: Instant) {
        if !self.view.window_focused {
            return;
        }
        for id in &self.view.visible_tabs {
            if let Some(tab) = self.tabs.get_mut(id) {
                tab.last_seen = Some(now);
            }
            self.escalations.remove(id);
        }
    }

    /// Whether the tab has been out of sight for more than `DONE_MIN_BACKGROUND`.
    fn in_background_long(&self, tab: &str, now: Instant) -> bool {
        let since = self
            .tabs
            .get(tab)
            .map_or(self.started, |t| t.last_seen.unwrap_or(t.born));
        now.saturating_duration_since(since) > DONE_MIN_BACKGROUND
    }

    /// One sound per category per `SOUND_GAP`. Records the sound when it allows one.
    fn sound_ok(&mut self, cat: Cat, now: Instant) -> bool {
        if let Some(last) = self.last_sound.get(&cat)
            && now.saturating_duration_since(*last) < SOUND_GAP
        {
            return false;
        }
        self.last_sound.insert(cat, now);
        true
    }

    /// The request for one thing to tell: the banner if its switch is on, the sound if its switch is
    /// on and neither quiet hours nor the limit hold it back, the flash for needs-you unless quiet.
    fn request(
        &mut self,
        cat: Cat,
        sound: SoundKind,
        channel: NotifyChannel,
        banner: Option<Banner>,
        quiet: bool,
        now: Instant,
    ) -> Option<NotifyRequest> {
        let banner = banner.filter(|_| channel.banner);
        let sound = (channel.sound && !quiet && self.sound_ok(cat, now)).then_some(sound);
        let flash = cat == Cat::NeedsYou && !quiet;
        (banner.is_some() || sound.is_some() || flash).then_some(NotifyRequest { banner, sound, flash })
    }

    fn emit(
        &mut self,
        event: Event,
        settings: &NotifySettings,
        minute: u16,
        now: Instant,
        out: &mut Vec<NotifyRequest>,
    ) {
        let Event { cat, entry } = event;
        let channel = channel_of(settings, cat);
        if entry.muted || (entry.terminal && !settings.terminal_alerts) || !(channel.banner || channel.sound) {
            return;
        }
        // Armed even while the window is in focus, so a wait in a tab nobody looked at is reminded about
        // once the window is left. The reminder waits while the window is in focus, and a tab in view drops it.
        if cat == Cat::NeedsYou {
            // Reset by a newer wait, and dropped when the tab is seen or the wait ends.
            self.escalations.insert(
                entry.who.tab_id.clone(),
                Escalation {
                    armed: now,
                    who: entry.who.clone(),
                    permission: entry.permission,
                    terminal: entry.terminal,
                    since_ms: entry.since_ms,
                    risky: entry.risky,
                },
            );
            if self
                .tabs
                .get(&entry.who.tab_id)
                .and_then(|t| t.wait.as_ref())
                .is_some_and(|w| w.silenced)
            {
                return;
            }
        }
        if self.view.window_focused {
            return;
        }
        let quiet = in_quiet_hours(&settings.quiet_hours, minute);
        if cat == Cat::Done && !self.in_background_long(&entry.who.tab_id, now) {
            return;
        }
        let Some(kind) = cat.kind() else {
            // A finished subagent has a sound if the switch is on, and no banner or window.
            out.extend(self.request(cat, cat.sound(), channel, None, quiet, now));
            return;
        };
        let window = self.windows.entry(cat).or_default();
        if window.until.is_some_and(|until| now < until) {
            window.entries.push(entry);
            return;
        }
        window.until = Some(now + BATCH_WINDOW);
        window.entries = vec![entry.clone()];
        out.extend(self.request(cat, cat.sound(), channel, Some(banner_of(kind, &entry)), quiet, now));
    }

    /// Ends the windows that are over. The collected events whose tab is still in that state go out as one banner.
    fn flush(&mut self, settings: &NotifySettings, minute: u16, now: Instant, out: &mut Vec<NotifyRequest>) {
        for cat in [Cat::NeedsYou, Cat::Done, Cat::Error] {
            let Some(window) = self.windows.get_mut(&cat) else {
                continue;
            };
            if window.until.is_none_or(|until| now < until) {
                continue;
            }
            window.until = None;
            let entries = std::mem::take(&mut window.entries);
            let channel = channel_of(settings, cat);
            if self.view.window_focused || !(channel.banner || channel.sound) {
                continue;
            }
            let ok: Vec<bool> = entries
                .iter()
                .map(|e| (!e.terminal || settings.terminal_alerts) && self.still(cat, e))
                .collect();
            // The first went out when it happened: nothing goes out unless a later one is still waiting.
            if !ok.iter().skip(1).any(|ok| *ok) {
                continue;
            }
            let mut live: Vec<&Entry> = Vec::new();
            for (entry, _) in entries.iter().zip(&ok).filter(|(_, ok)| **ok) {
                match live.iter_mut().find(|e| e.who.tab_id == entry.who.tab_id) {
                    Some(slot) => *slot = entry,
                    None => live.push(entry),
                }
            }
            let Some(kind) = cat.kind() else { continue };
            let banner = match live.as_slice() {
                [one] => banner_of(kind, one),
                many => copy::batch(kind, &many.iter().map(|e| (e.who.clone(), e.risky)).collect::<Vec<_>>()),
            };
            let quiet = in_quiet_hours(&settings.quiet_hours, minute);
            out.extend(self.request(cat, cat.sound(), channel, Some(banner), quiet, now));
        }
    }

    /// Whether the tab of a collected event is still in the state that made it.
    fn still(&self, cat: Cat, entry: &Entry) -> bool {
        let Some(tab) = self.tabs.get(&entry.who.tab_id).filter(|t| !t.muted) else {
            return false;
        };
        let Some(live) = tab.live.as_ref() else {
            return false;
        };
        match cat {
            Cat::NeedsYou => tab.wait.as_ref().is_some_and(|w| {
                w.permission == entry.permission && w.terminal == entry.terminal && w.since_ms == entry.since_ms
            }),
            Cat::Done => live.state == LiveState::Done && live.since_ms == entry.since_ms,
            Cat::Error => live.state == LiveState::Error && live.since_ms == entry.since_ms,
            Cat::Subagent => false,
        }
    }

    /// Sends the reminders that are due. They wait while the window is in focus.
    fn escalate(&mut self, settings: &NotifySettings, minute: u16, now: Instant, out: &mut Vec<NotifyRequest>) {
        if self.view.window_focused {
            return;
        }
        let mut due: Vec<String> = self
            .escalations
            .iter()
            .filter(|(_, e)| now.saturating_duration_since(e.armed) >= ESCALATE_AFTER)
            .map(|(id, _)| id.clone())
            .collect();
        due.sort();
        for id in due {
            let Some(e) = self.escalations.remove(&id) else {
                continue;
            };
            let channel = settings.needs_you;
            if self.tabs.get(&id).is_none_or(|t| t.muted)
                || (e.terminal && !settings.terminal_alerts)
                || !(channel.banner || channel.sound)
            {
                continue;
            }
            let minutes = (now.saturating_duration_since(e.armed).as_secs_f64() / 60.0)
                .round()
                .max(1.0) as u64;
            let banner = copy::escalation(&e.who, minutes, e.permission, e.risky);
            let quiet = in_quiet_hours(&settings.quiet_hours, minute);
            out.extend(self.request(Cat::NeedsYou, SoundKind::Escalation, channel, Some(banner), quiet, now));
        }
    }
}

fn channel_of(settings: &NotifySettings, cat: Cat) -> NotifyChannel {
    match cat {
        Cat::NeedsYou => settings.needs_you,
        Cat::Done => settings.done,
        Cat::Error => settings.error,
        Cat::Subagent => NotifyChannel {
            banner: false,
            sound: settings.subagent_sound,
        },
    }
}

fn banner_of(kind: Kind, entry: &Entry) -> Banner {
    match kind {
        Kind::NeedsYou => copy::needs_you(&entry.who, &entry.phrase, entry.risky),
        Kind::Done => copy::done(&entry.who, entry.recap.as_deref()),
        Kind::Error => copy::error(&entry.who, &entry.phrase),
    }
}

fn needs_you_event(who: &Who, muted: bool, wait: &Wait, live: &Live, risky: bool) -> Event {
    Event {
        cat: Cat::NeedsYou,
        entry: Entry {
            who: who.clone(),
            permission: wait.permission,
            terminal: wait.terminal,
            since_ms: wait.since_ms,
            phrase: live.phrase.clone(),
            recap: None,
            risky,
            muted,
        },
    }
}

/// Updates one tab from its row and pushes the events the difference makes.
fn follow(
    tab: &mut Tab,
    info: &SessionInfo,
    holds: &[PermissionView],
    seeding: bool,
    now: Instant,
    events: &mut Vec<Event>,
) {
    let (status, terminal) = match (&info.state, &info.live, &info.attention) {
        (SessionState::Running, Some(status), _) => (
            Live {
                state: status.state,
                phrase: status.phrase.clone(),
                since_ms: status.since_ms,
                finished: status.subagents_finished,
            },
            false,
        ),
        (SessionState::Running, None, Some(attention)) => (
            Live {
                state: LiveState::NeedsQuestion,
                phrase: attention
                    .message
                    .clone()
                    .unwrap_or_else(|| "Wants your attention".into()),
                since_ms: attention.since_ms,
                finished: 0,
            },
            true,
        ),
        _ => {
            tab.clear();
            return;
        }
    };
    if terminal {
        tab.done_since = None;
        tab.error_since = None;
    }
    let (who, muted) = (tab.who.clone(), tab.muted);
    let entry = |permission: bool, risky: bool| Entry {
        who: who.clone(),
        permission,
        terminal,
        since_ms: status.since_ms,
        phrase: status.phrase.clone(),
        recap: info
            .live
            .as_ref()
            .and_then(|s| s.recap.as_ref())
            .map(|r| r.text.clone()),
        risky,
        muted,
    };
    // A subagent finished: the core's counter rose while the tab kept working. The counter only
    // rises on a SubagentStop, so the main Stop and SessionEnd clearing the set never look like one,
    // and a restart starts it again from 0 (it falls, and is taken as the new reading).
    if let Some(prev) = &tab.live
        && prev.state == LiveState::Working
        && status.finished > prev.finished
    {
        for _ in 0..(status.finished - prev.finished).min(MAX_SUBAGENT_EVENTS) {
            events.push(Event {
                cat: Cat::Subagent,
                entry: entry(false, false),
            });
        }
    }
    match status.state {
        LiveState::NeedsPermission | LiveState::NeedsQuestion => {
            let permission = status.state == LiveState::NeedsPermission;
            let mine: Vec<&PermissionView> = if permission {
                holds.iter().filter(|h| h.tab == info.tab_id).collect()
            } else {
                Vec::new()
            };
            let same = tab
                .wait
                .as_ref()
                .is_some_and(|w| w.permission == permission && w.terminal == terminal && w.since_ms == status.since_ms);
            if !same {
                tab.wait = Some(Wait {
                    permission,
                    terminal,
                    since_ms: status.since_ms,
                    rids: mine.iter().map(|h| h.rid.clone()).collect(),
                    risky: mine.iter().any(|h| h.risk.is_some()),
                    // A wait found at the first look was told in a run before this one.
                    told: seeding,
                    silenced: false,
                    deadline: now + HOLD_WAIT,
                });
            }
            let wait = tab.wait.as_mut().expect("the wait was just set");
            let fresh: Vec<&&PermissionView> = mine.iter().filter(|h| !wait.rids.contains(&h.rid)).collect();
            let fresh_risky = fresh.iter().any(|h| h.risk.is_some());
            wait.rids.extend(fresh.iter().map(|h| h.rid.clone()));
            wait.risky |= fresh_risky;
            if !wait.told {
                // A permission is told once its hold is open, so its risk is known; a question needs none.
                if !permission || !wait.rids.is_empty() || now >= wait.deadline {
                    wait.told = true;
                    let risky = wait.risky;
                    events.push(Event {
                        cat: Cat::NeedsYou,
                        entry: entry(permission, risky),
                    });
                }
            } else if same && !fresh.is_empty() {
                // A second request in the same wait.
                events.push(Event {
                    cat: Cat::NeedsYou,
                    entry: entry(permission, fresh_risky),
                });
            }
        }
        _ => tab.wait = None,
    }
    match status.state {
        LiveState::Done if tab.done_since != Some(status.since_ms) => {
            tab.done_since = Some(status.since_ms);
            if !seeding {
                events.push(Event {
                    cat: Cat::Done,
                    entry: entry(false, false),
                });
            }
        }
        LiveState::Error if tab.error_since != Some(status.since_ms) => {
            tab.error_since = Some(status.since_ms);
            if !seeding {
                events.push(Event {
                    cat: Cat::Error,
                    entry: entry(false, false),
                });
            }
        }
        _ => {}
    }
    tab.live = Some(status);
}

/// "HH:MM" as minutes since midnight.
fn parse_clock(text: &str) -> Option<u16> {
    let (h, m) = text.trim().split_once(':')?;
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// Whether `minute` (since local midnight) is inside the quiet hours: from start up to, not including,
/// end, wrapping past midnight when start is after end. Equal times, or a time that cannot be read, mean none.
pub fn in_quiet_hours(quiet: &QuietHours, minute: u16) -> bool {
    if !quiet.enabled {
        return false;
    }
    let (Some(start), Some(end)) = (parse_clock(&quiet.start), parse_clock(&quiet.end)) else {
        return false;
    };
    match start.cmp(&end) {
        std::cmp::Ordering::Equal => false,
        std::cmp::Ordering::Less => start <= minute && minute < end,
        std::cmp::Ordering::Greater => minute >= start || minute < end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notify::{BannerAction, NotifyRequest, SoundKind};
    use marshell_protocol::api::{
        Attention, CommandState, LiveState, LiveStatus, NotifyChannel, NotifySettings, QuietHours, SessionInfo,
        SessionState, SessionsSnapshot, UiView,
    };
    use marshell_protocol::hook::{PermissionDetail, PermissionView};
    use std::time::{Duration, Instant};

    const NOON: u16 = 12 * 60;

    fn live(state: LiveState, phrase: &str, since_ms: u64) -> LiveStatus {
        LiveStatus {
            state,
            phrase: phrase.into(),
            since_ms,
            recap: None,
            model: None,
            effort: None,
            context_pct: None,
            five_hour_pct: None,
            five_hour_resets_at: None,
            seven_day_pct: None,
            subagents: 0,
            subagents_finished: 0,
        }
    }

    /// An open Claude row; tab `t<number>`, named `s<number>`.
    fn row(number: u32, live: Option<LiveStatus>) -> SessionInfo {
        SessionInfo {
            tab_id: format!("t{number}"),
            number,
            name: format!("s{number}"),
            agent: Some("claude".into()),
            profile_id: "p".into(),
            cwd: String::new(),
            project: "C:/work/app".into(),
            mode: None,
            state: SessionState::Running,
            command: CommandState::Unknown,
            paused: false,
            foreground: None,
            live,
            activity: None,
            status_note: None,
            muted: false,
            attention: None,
            stuck: None,
        }
    }

    fn idle(number: u32) -> SessionInfo {
        row(number, Some(live(LiveState::Idle, "", 1)))
    }

    fn perm(number: u32, since: u64) -> SessionInfo {
        row(
            number,
            Some(live(LiveState::NeedsPermission, "Wants to run `npm test`", since)),
        )
    }

    fn question(number: u32, since: u64) -> SessionInfo {
        row(number, Some(live(LiveState::NeedsQuestion, "Has a question", since)))
    }

    fn attention(number: u32, since_ms: u64, message: Option<&str>) -> SessionInfo {
        let mut info = row(number, None);
        info.attention = Some(Attention {
            message: message.map(String::from),
            since_ms,
        });
        info
    }

    fn done(number: u32, since: u64) -> SessionInfo {
        row(number, Some(live(LiveState::Done, "Done", since)))
    }

    fn working(number: u32, since: u64) -> SessionInfo {
        row(number, Some(live(LiveState::Working, "Thinking", since)))
    }

    fn hold(number: u32, rid: &str, risk: Option<&str>) -> PermissionView {
        PermissionView {
            rid: rid.into(),
            tab: format!("t{number}"),
            agent: "claude".into(),
            at_ms: 0,
            detail: PermissionDetail::Bash {
                command: "npm test".into(),
                cwd: None,
            },
            risk: risk.map(String::from),
            always_rule: None,
        }
    }

    fn snap(rows: Vec<SessionInfo>) -> SessionsSnapshot {
        SessionsSnapshot {
            sessions: rows,
            closed: Vec::new(),
        }
    }

    struct H {
        e: Engine,
        base: Instant,
        settings: NotifySettings,
        minute: u16,
    }

    impl H {
        fn new() -> H {
            let base = Instant::now();
            H {
                e: Engine::starting_at(base),
                base,
                settings: NotifySettings::default(),
                minute: NOON,
            }
        }

        fn at(&self, secs: f64) -> Instant {
            self.base + Duration::from_secs_f64(secs)
        }

        fn obs(&mut self, secs: f64, rows: Vec<SessionInfo>, holds: &[PermissionView]) -> Vec<NotifyRequest> {
            let now = self.at(secs);
            self.e.observe(&snap(rows), holds, &self.settings, self.minute, now)
        }

        fn tick(&mut self, secs: f64) -> Vec<NotifyRequest> {
            let now = self.at(secs);
            self.e.tick(&self.settings, self.minute, now)
        }

        fn view(&mut self, secs: f64, focused: bool, tabs: &[&str]) {
            let now = self.at(secs);
            self.e.set_view(
                UiView {
                    window_focused: focused,
                    visible_tabs: tabs.iter().map(|t| t.to_string()).collect(),
                },
                now,
            );
        }
    }

    fn only(mut got: Vec<NotifyRequest>) -> NotifyRequest {
        assert_eq!(got.len(), 1, "expected one request, got {got:?}");
        got.remove(0)
    }

    fn banner(req: &NotifyRequest) -> &crate::notify::Banner {
        req.banner.as_ref().expect("a banner")
    }

    #[test]
    fn the_first_look_replays_nothing() {
        let mut h = H::new();
        assert!(
            h.obs(
                0.0,
                vec![perm(1, 10), question(2, 10), done(3, 10)],
                &[hold(1, "r1", None)]
            )
            .is_empty()
        );
        assert!(
            h.obs(
                1.0,
                vec![perm(1, 10), question(2, 10), done(3, 10)],
                &[hold(1, "r1", None)]
            )
            .is_empty()
        );
        assert!(h.tick(70.0).is_empty());
        // Only a new wait is an event.
        assert!(
            h.obs(71.0, vec![working(1, 11), question(2, 10), done(3, 10)], &[])
                .is_empty()
        );
        assert_eq!(
            h.obs(
                72.0,
                vec![perm(1, 12), question(2, 10), done(3, 10)],
                &[hold(1, "r2", None)]
            )
            .len(),
            1
        );
    }

    #[test]
    fn a_permission_request_asks_for_a_banner_a_sound_and_a_flash() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1)], &[]);
        let req = only(h.obs(1.0, vec![perm(1, 5)], &[hold(1, "r1", None)]));
        let b = banner(&req);
        assert_eq!(b.title, "s1 needs you");
        assert_eq!(b.body, "Wants to run `npm test`");
        assert_eq!(b.meta, "Session 1 · app");
        assert_eq!(b.action, BannerAction::Open);
        assert_eq!(b.tab_id.as_deref(), Some("t1"));
        assert_eq!(req.sound, Some(SoundKind::NeedsYou));
        assert!(req.flash);
        // The same wait seen again says nothing more.
        assert!(h.obs(2.0, vec![perm(1, 5)], &[hold(1, "r1", None)]).is_empty());
    }

    #[test]
    fn a_question_is_an_event_too() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1)], &[]);
        let req = only(h.obs(1.0, vec![question(1, 5)], &[]));
        assert_eq!(banner(&req).body, "Has a question");
        assert_eq!(req.sound, Some(SoundKind::NeedsYou));
    }

    #[test]
    fn attention_raises_needs_you_with_its_message() {
        let mut h = H::new();
        h.obs(0.0, vec![row(1, None)], &[]);
        let req = only(h.obs(1.0, vec![attention(1, 5, Some("Build needs input"))], &[]));
        let b = banner(&req);
        assert_eq!(b.title, "s1 needs you");
        assert_eq!(b.body, "Build needs input");
        assert_eq!(b.meta, "Session 1 · app");
        assert_eq!(b.action, BannerAction::Open);
        assert_eq!(b.tab_id.as_deref(), Some("t1"));
        assert_eq!(req.sound, Some(SoundKind::NeedsYou));
        assert!(req.flash);
        let reminder = only(h.tick(61.0));
        assert_eq!(banner(&reminder).body, "s1 has waited 1 min for an answer");
        assert_eq!(reminder.sound, Some(SoundKind::Escalation));
        assert!(h.tick(200.0).is_empty());
    }

    #[test]
    fn attention_without_message_says_wants_your_attention() {
        let mut h = H::new();
        h.obs(0.0, vec![row(1, None)], &[]);
        let req = only(h.obs(1.0, vec![attention(1, 5, None)], &[]));
        assert_eq!(banner(&req).body, "Wants your attention");
        assert_eq!(req.sound, Some(SoundKind::NeedsYou));
    }

    #[test]
    fn attention_is_silent_when_terminal_alerts_off() {
        let mut h = H::new();
        h.settings.terminal_alerts = false;
        h.obs(0.0, vec![row(1, None), idle(2)], &[]);
        assert!(h.obs(1.0, vec![attention(1, 5, None), idle(2)], &[]).is_empty());
        h.settings.terminal_alerts = true;
        assert!(h.obs(2.0, vec![attention(1, 5, None), idle(2)], &[]).is_empty());
        assert!(h.tick(70.0).is_empty());
        // The wait was tracked while off: only a new since_ms is a new event.
        assert_eq!(h.obs(71.0, vec![attention(1, 6, None), idle(2)], &[]).len(), 1);
        h.settings.terminal_alerts = false;
        let req = only(h.obs(75.0, vec![attention(1, 6, None), question(2, 7)], &[]));
        assert_eq!(banner(&req).title, "s2 needs you");
        assert_eq!(req.sound, Some(SoundKind::NeedsYou));
        assert!(h.tick(131.0).is_empty());
        // A switch turned off before a batch flush removes only terminal entries.
        for hook_first in [false, true] {
            let mut h = H::new();
            h.obs(0.0, vec![row(1, None), idle(2)], &[]);
            let first = if hook_first {
                vec![row(1, None), question(2, 7)]
            } else {
                vec![attention(1, 5, None), idle(2)]
            };
            assert_eq!(h.obs(1.0, first, &[]).len(), 1);
            assert!(h.obs(2.0, vec![attention(1, 5, None), question(2, 7)], &[]).is_empty());
            h.settings.terminal_alerts = false;
            let batch = h.tick(4.0);
            if hook_first {
                assert!(batch.is_empty());
            } else {
                let req = only(batch);
                assert_eq!(banner(&req).title, "s2 needs you");
                assert_eq!(req.sound, Some(SoundKind::NeedsYou));
            }
        }
    }

    #[test]
    fn attention_flood_is_one_banner() {
        let mut h = H::new();
        h.obs(0.0, vec![row(1, None)], &[]);
        assert_eq!(h.obs(1.0, vec![attention(1, 5, None)], &[]).len(), 1);
        for secs in [1.1, 2.0, 3.0, 4.0, 10.0, 30.0] {
            assert!(h.obs(secs, vec![attention(1, 5, Some("More bells"))], &[]).is_empty());
            assert!(h.tick(secs).is_empty());
        }
    }

    #[test]
    fn new_attention_after_clear_is_a_new_wait() {
        let mut h = H::new();
        h.obs(0.0, vec![row(1, None)], &[]);
        assert_eq!(h.obs(1.0, vec![attention(1, 5, None)], &[]).len(), 1);
        assert!(h.obs(2.0, vec![row(1, None)], &[]).is_empty());
        assert!(h.tick(61.0).is_empty());
        let req = only(h.obs(70.0, vec![attention(1, 6, Some("Next question"))], &[]));
        assert_eq!(banner(&req).body, "Next question");
        assert_eq!(req.sound, Some(SoundKind::NeedsYou));
    }

    #[test]
    fn attention_on_visible_tab_in_focused_window_is_not_announced() {
        let mut h = H::new();
        h.obs(0.0, vec![row(1, None)], &[]);
        h.view(0.5, true, &["t1"]);
        assert!(h.obs(1.0, vec![attention(1, 5, None)], &[]).is_empty());
        h.view(2.0, false, &["t1"]);
        assert!(h.obs(3.0, vec![attention(1, 5, None)], &[]).is_empty());
        assert!(h.tick(70.0).is_empty());
    }

    #[test]
    fn attention_is_ignored_on_a_hooked_row() {
        let mut h = H::new();
        h.obs(0.0, vec![working(1, 1)], &[]);
        let mut info = attention(1, 5, Some("Build needs input"));
        info.live = working(1, 1).live;
        assert!(h.obs(1.0, vec![info], &[]).is_empty());
        assert!(h.tick(70.0).is_empty());
    }

    #[test]
    fn a_risky_request_gets_review_and_never_shows_the_command() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1), idle(2)], &[]);
        let req = only(h.obs(1.0, vec![perm(1, 5), idle(2)], &[hold(1, "r1", Some("Deletes files"))]));
        let b = banner(&req);
        assert_eq!(b.action, BannerAction::Review);
        assert_eq!(b.body, "Needs a careful look.");
        assert!(!b.body.contains("npm test"));
    }

    #[test]
    fn a_second_request_on_one_tab_inside_one_interval_is_a_second_event() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1)], &[]);
        assert_eq!(h.obs(1.0, vec![perm(1, 5)], &[hold(1, "r1", None)]).len(), 1);
        // Same state, same since: only the open hold set tells them apart. Inside the batch window it is collected.
        assert!(
            h.obs(2.0, vec![perm(1, 5)], &[hold(1, "r1", None), hold(1, "r2", None)])
                .is_empty()
        );
        let req = only(h.tick(4.5));
        assert_eq!(banner(&req).title, "s1 needs you");
    }

    #[test]
    fn a_permission_that_becomes_a_question_is_a_second_event() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1)], &[]);
        assert_eq!(h.obs(1.0, vec![perm(1, 5)], &[hold(1, "r1", None)]).len(), 1);
        let req = only(h.obs(10.0, vec![question(1, 6)], &[]));
        assert_eq!(banner(&req).body, "Has a question");
    }

    #[test]
    fn idle_working_and_ended_are_not_events() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1)], &[]);
        for (i, state) in [LiveState::Working, LiveState::Idle, LiveState::Ended]
            .into_iter()
            .enumerate()
        {
            let rows = vec![row(1, Some(live(state, "x", 10 + i as u64)))];
            assert!(h.obs(10.0 + i as f64 * 10.0, rows, &[]).is_empty());
        }
        let mut stopped = idle(1);
        stopped.state = SessionState::Exited { code: 0 };
        assert!(h.obs(60.0, vec![stopped], &[]).is_empty());
    }

    #[test]
    fn a_muted_tab_makes_no_event() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1)], &[]);
        let mut muted = perm(1, 5);
        muted.muted = true;
        assert!(h.obs(1.0, vec![muted], &[hold(1, "r1", None)]).is_empty());
        assert!(h.tick(70.0).is_empty());
    }

    #[test]
    fn a_category_with_both_switches_off_makes_no_event() {
        let mut h = H::new();
        let off = NotifyChannel {
            banner: false,
            sound: false,
        };
        h.settings.needs_you = off;
        h.obs(0.0, vec![idle(1)], &[]);
        assert!(h.obs(1.0, vec![question(1, 5)], &[]).is_empty());
        // Nor an escalation for it later.
        assert!(h.tick(70.0).is_empty());
    }

    #[test]
    fn each_switch_gates_its_own_part() {
        let mut h = H::new();
        h.settings.needs_you = NotifyChannel {
            banner: false,
            sound: true,
        };
        h.obs(0.0, vec![idle(1), idle(2)], &[]);
        let req = only(h.obs(1.0, vec![question(1, 5), idle(2)], &[]));
        assert_eq!(
            (req.banner, req.sound, req.flash),
            (None, Some(SoundKind::NeedsYou), true)
        );
        h.settings.needs_you = NotifyChannel {
            banner: true,
            sound: false,
        };
        let req = only(h.obs(10.0, vec![question(1, 5), question(2, 6)], &[]));
        assert_eq!(banner(&req).title, "s2 needs you");
        assert_eq!(req.sound, None);
    }

    #[test]
    fn a_window_in_focus_notifies_nothing_and_replays_nothing_later() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1), idle(2)], &[]);
        h.view(0.5, true, &["t1"]);
        assert!(h.obs(1.0, vec![question(1, 5), idle(2)], &[]).is_empty());
        // A tab in the window but not in view is still the window in focus.
        assert!(h.obs(2.0, vec![question(1, 5), question(2, 6)], &[]).is_empty());
        assert!(h.tick(70.0).is_empty());
        h.view(80.0, false, &["t1"]);
        // The wait nobody looked at is reminded about once the window is left; the one in view was seen.
        let req = only(h.tick(81.0));
        assert_eq!(banner(&req).tab_id.as_deref(), Some("t2"));
        assert_eq!(req.sound, Some(SoundKind::Escalation));
        assert!(h.tick(82.0).is_empty());
        assert!(h.obs(83.0, vec![question(1, 5), question(2, 6)], &[]).is_empty());
    }

    #[test]
    fn an_unfocused_window_notifies_even_for_a_visible_tab() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1)], &[]);
        h.view(1.0, false, &["t1"]);
        assert_eq!(h.obs(2.0, vec![question(1, 5)], &[]).len(), 1);
    }

    #[test]
    fn done_waits_for_thirty_seconds_in_the_background() {
        let mut h = H::new();
        h.obs(0.0, vec![working(1, 1), working(2, 1)], &[]);
        // No view has ever been reported: everything is in the background since the engine started.
        assert!(h.obs(29.0, vec![done(1, 5), working(2, 1)], &[]).is_empty());
        let req = only(h.obs(31.0, vec![done(1, 5), done(2, 6)], &[]));
        assert_eq!(banner(&req).title, "s2 is done");
        assert_eq!(banner(&req).body, "Done");
        assert_eq!(req.sound, Some(SoundKind::Done));
        assert!(!req.flash);
        // The Done that was too early is not replayed once the thirty seconds pass.
        assert!(h.tick(50.0).is_empty());
    }

    #[test]
    fn done_counts_the_time_since_the_tab_was_last_in_view() {
        let mut h = H::new();
        h.obs(0.0, vec![working(1, 1)], &[]);
        h.view(1.0, true, &["t1"]);
        h.view(100.0, false, &["t1"]);
        assert!(h.obs(120.0, vec![done(1, 5)], &[]).is_empty());
        h.obs(121.0, vec![working(1, 6)], &[]);
        let req = only(h.obs(131.0, vec![done(1, 7)], &[]));
        assert_eq!(banner(&req).title, "s1 is done");
    }

    #[test]
    fn done_shows_the_recap_and_fires_once_per_finish() {
        let mut h = H::new();
        h.obs(0.0, vec![working(1, 1)], &[]);
        let mut finished = done(1, 5);
        finished.live.as_mut().unwrap().recap = Some(marshell_protocol::api::Recap {
            text: "Fixed the build.".into(),
            at_ms: 5,
        });
        let req = only(h.obs(40.0, vec![finished.clone()], &[]));
        assert_eq!(banner(&req).body, "Fixed the build.");
        assert!(h.obs(41.0, vec![finished], &[]).is_empty());
        h.obs(50.0, vec![working(1, 6)], &[]);
        assert_eq!(h.obs(90.0, vec![done(1, 7)], &[]).len(), 1);
    }

    #[test]
    fn an_error_is_an_event_with_its_own_sound() {
        let mut h = H::new();
        h.obs(0.0, vec![working(1, 1)], &[]);
        let failed = row(1, Some(live(LiveState::Error, "rate limit reached", 5)));
        let req = only(h.obs(1.0, vec![failed.clone()], &[]));
        assert_eq!(banner(&req).title, "s1 stopped");
        assert_eq!(banner(&req).body, "Stopped: rate limit reached.");
        assert_eq!(req.sound, Some(SoundKind::Error));
        assert!(!req.flash);
        assert!(h.obs(2.0, vec![failed], &[]).is_empty());
    }

    fn subagents(number: u32, running: u32, finished: u32, state: LiveState) -> SessionInfo {
        let mut l = live(state, "Thinking", 1);
        l.subagents = running;
        l.subagents_finished = finished;
        row(number, Some(l))
    }

    #[test]
    fn a_finished_subagent_plays_its_sound_only_when_the_switch_is_on() {
        let mut h = H::new();
        h.obs(0.0, vec![subagents(1, 2, 0, LiveState::Working)], &[]);
        assert!(h.obs(1.0, vec![subagents(1, 1, 1, LiveState::Working)], &[]).is_empty());
        h.settings.subagent_sound = true;
        let req = only(h.obs(10.0, vec![subagents(1, 0, 2, LiveState::Working)], &[]));
        assert_eq!(
            (req.banner, req.sound, req.flash),
            (None, Some(SoundKind::SubagentDone), false)
        );
    }

    #[test]
    fn a_subagent_event_is_one_per_increment_and_the_sound_is_limited() {
        let mut h = H::new();
        h.settings.subagent_sound = true;
        h.obs(0.0, vec![subagents(1, 3, 0, LiveState::Working)], &[]);
        // Two finish between two looks: two events, one sound.
        let got = h.obs(10.0, vec![subagents(1, 1, 2, LiveState::Working)], &[]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].sound, Some(SoundKind::SubagentDone));
        // Three seconds on, the next one sounds again.
        assert_eq!(h.obs(13.0, vec![subagents(1, 0, 3, LiveState::Working)], &[]).len(), 1);
    }

    #[test]
    fn the_main_stop_clearing_the_count_is_not_a_finished_subagent() {
        let mut h = H::new();
        h.settings.subagent_sound = true;
        h.obs(0.0, vec![subagents(1, 2, 0, LiveState::Working)], &[]);
        assert!(h.obs(1.0, vec![subagents(1, 0, 0, LiveState::Working)], &[]).is_empty());
    }

    #[test]
    fn session_end_clearing_the_count_is_not_a_finished_subagent() {
        let mut h = H::new();
        h.settings.subagent_sound = true;
        h.obs(0.0, vec![subagents(1, 2, 0, LiveState::Working)], &[]);
        assert!(h.obs(1.0, vec![subagents(1, 0, 0, LiveState::Ended)], &[]).is_empty());
    }

    #[test]
    fn a_restart_resetting_the_counter_is_not_a_finished_subagent() {
        let mut h = H::new();
        h.settings.subagent_sound = true;
        h.obs(0.0, vec![subagents(1, 1, 3, LiveState::Working)], &[]);
        assert!(h.obs(1.0, vec![subagents(1, 0, 0, LiveState::Working)], &[]).is_empty());
        // The counter starts again from the new process.
        assert_eq!(h.obs(10.0, vec![subagents(1, 0, 1, LiveState::Working)], &[]).len(), 1);
    }

    #[test]
    fn a_subagent_that_stops_while_the_tab_is_not_working_is_not_announced() {
        let mut h = H::new();
        h.settings.subagent_sound = true;
        h.obs(0.0, vec![done(1, 1)], &[]);
        assert!(h.obs(1.0, vec![subagents(1, 0, 1, LiveState::Done)], &[]).is_empty());
    }

    #[test]
    fn a_muted_tab_or_a_focused_window_hears_no_subagent() {
        let mut h = H::new();
        h.settings.subagent_sound = true;
        h.obs(
            0.0,
            vec![
                subagents(1, 2, 0, LiveState::Working),
                subagents(2, 2, 0, LiveState::Working),
            ],
            &[],
        );
        let mut muted = subagents(1, 1, 1, LiveState::Working);
        muted.muted = true;
        assert!(
            h.obs(1.0, vec![muted, subagents(2, 2, 0, LiveState::Working)], &[])
                .is_empty()
        );
        h.view(2.0, true, &[]);
        assert!(
            h.obs(
                3.0,
                vec![
                    subagents(1, 1, 1, LiveState::Working),
                    subagents(2, 1, 1, LiveState::Working)
                ],
                &[]
            )
            .is_empty()
        );
    }

    fn three_idle() -> Vec<SessionInfo> {
        vec![idle(3), idle(5), idle(7)]
    }

    #[test]
    fn the_first_event_goes_out_at_once_and_the_next_ones_become_one_banner() {
        let mut h = H::new();
        h.obs(0.0, three_idle(), &[]);
        assert_eq!(h.obs(1.0, vec![question(3, 5), idle(5), idle(7)], &[]).len(), 1);
        assert!(
            h.obs(1.5, vec![question(3, 5), question(5, 6), idle(7)], &[])
                .is_empty()
        );
        assert!(
            h.obs(2.5, vec![question(3, 5), question(5, 6), question(7, 7)], &[])
                .is_empty()
        );
        assert!(h.tick(3.9).is_empty());
        let req = only(h.tick(4.0));
        let b = banner(&req);
        assert_eq!(b.title, "3 sessions need you");
        assert_eq!(b.body, "Sessions 3 s3, 5 s5 and 7 s7");
        assert_eq!(b.meta, "3 sessions");
        assert_eq!(b.action, BannerAction::Open);
        assert_eq!(b.tab_id, None);
        assert!(req.flash);
        // The window is over: nothing more, and the next event is a first one again.
        assert!(h.tick(5.0).is_empty());
    }

    #[test]
    fn a_batch_drops_the_sessions_that_stopped_needing_you() {
        let mut h = H::new();
        h.obs(0.0, three_idle(), &[]);
        h.obs(1.0, vec![question(3, 5), idle(5), idle(7)], &[]);
        h.obs(1.5, vec![question(3, 5), question(5, 6), idle(7)], &[]);
        h.obs(2.0, vec![question(3, 5), question(5, 6), question(7, 7)], &[]);
        h.obs(3.0, vec![question(3, 5), working(5, 8), question(7, 7)], &[]);
        let req = only(h.tick(4.0));
        assert_eq!(banner(&req).title, "2 sessions need you");
        assert_eq!(banner(&req).body, "Sessions 3 s3 and 7 s7");
    }

    #[test]
    fn a_batch_with_nobody_left_sends_nothing() {
        let mut h = H::new();
        h.obs(0.0, three_idle(), &[]);
        h.obs(1.0, vec![question(3, 5), idle(5), idle(7)], &[]);
        h.obs(2.0, vec![question(3, 5), question(5, 6), idle(7)], &[]);
        h.obs(3.0, vec![question(3, 5), working(5, 8), idle(7)], &[]);
        assert!(h.tick(4.0).is_empty());
    }

    #[test]
    fn a_batch_with_a_risky_request_says_review() {
        let mut h = H::new();
        h.obs(0.0, three_idle(), &[]);
        h.obs(1.0, vec![question(3, 5), idle(5), idle(7)], &[]);
        h.obs(
            2.0,
            vec![question(3, 5), perm(5, 6), idle(7)],
            &[hold(5, "r", Some("Deletes files"))],
        );
        let req = only(h.tick(4.0));
        assert_eq!(banner(&req).action, BannerAction::Review);
    }

    #[test]
    fn categories_batch_apart() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1), working(2, 1)], &[]);
        assert_eq!(h.obs(40.0, vec![question(1, 5), working(2, 1)], &[]).len(), 1);
        // A Done inside the needs-you window is a first event of its own category.
        assert_eq!(h.obs(41.0, vec![question(1, 5), done(2, 6)], &[]).len(), 1);
    }

    #[test]
    fn sounds_follow_a_limit_of_one_per_three_seconds_per_category() {
        let mut h = H::new();
        h.obs(0.0, three_idle(), &[]);
        h.obs(1.0, vec![question(3, 5), idle(5), idle(7)], &[]);
        h.obs(2.0, vec![question(3, 5), question(5, 6), idle(7)], &[]);
        // The batch at 4.0 sounds (3 s since the first) ...
        assert_eq!(only(h.tick(4.0)).sound, Some(SoundKind::NeedsYou));
        // ... so a new first event at 5.0 gets its banner and no sound.
        let req = only(h.obs(5.0, vec![question(3, 5), question(5, 6), question(7, 7)], &[]));
        assert_eq!(banner(&req).title, "s7 needs you");
        assert_eq!(req.sound, None);
        assert!(req.flash);
    }

    #[test]
    fn quiet_hours_hold_back_the_sound_and_the_flash_but_not_the_banner() {
        let mut h = H::new();
        h.settings.quiet_hours = QuietHours {
            enabled: true,
            start: "22:00".into(),
            end: "08:00".into(),
        };
        h.minute = 23 * 60;
        h.obs(0.0, vec![idle(1)], &[]);
        let req = only(h.obs(1.0, vec![question(1, 5)], &[]));
        assert!(req.banner.is_some());
        assert_eq!((req.sound, req.flash), (None, false));
    }

    #[test]
    fn quiet_hours_table() {
        let q = |start: &str, end: &str| QuietHours {
            enabled: true,
            start: start.into(),
            end: end.into(),
        };
        let at = |h: u16, m: u16| h * 60 + m;
        let night = q("22:00", "08:00");
        assert!(!in_quiet_hours(&night, at(21, 59)));
        assert!(in_quiet_hours(&night, at(22, 0)));
        assert!(in_quiet_hours(&night, at(3, 0)));
        assert!(in_quiet_hours(&night, at(7, 59)));
        assert!(!in_quiet_hours(&night, at(8, 0)));
        let day = q("09:00", "17:00");
        assert!(!in_quiet_hours(&day, at(8, 59)));
        assert!(in_quiet_hours(&day, at(9, 0)));
        assert!(in_quiet_hours(&day, at(16, 59)));
        assert!(!in_quiet_hours(&day, at(17, 0)));
        let same = q("10:00", "10:00");
        for m in [at(0, 0), at(10, 0), at(15, 0)] {
            assert!(!in_quiet_hours(&same, m));
        }
        let off = QuietHours {
            enabled: false,
            ..night.clone()
        };
        assert!(!in_quiet_hours(&off, at(23, 0)));
        assert!(!in_quiet_hours(&q("late", "08:00"), at(23, 0)));
    }

    #[test]
    fn a_wait_that_lasts_a_minute_escalates_once() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1), idle(2)], &[]);
        h.obs(1.0, vec![perm(1, 5), idle(2)], &[hold(1, "r1", None)]);
        assert!(h.tick(60.9).is_empty());
        let req = only(h.tick(61.0));
        let b = banner(&req);
        assert_eq!(b.title, "s1 needs you");
        assert_eq!(b.body, "s1 has waited 1 min to run a command");
        assert_eq!(b.tab_id.as_deref(), Some("t1"));
        assert_eq!(req.sound, Some(SoundKind::Escalation));
        assert!(req.flash);
        assert!(h.tick(200.0).is_empty());
        // A question reads "for an answer".
        h.obs(300.0, vec![question(1, 9), idle(2)], &[]);
        assert_eq!(banner(&only(h.tick(361.0))).body, "s1 has waited 1 min for an answer");
    }

    #[test]
    fn an_escalation_for_a_risky_request_says_review_and_hides_the_command() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1)], &[]);
        h.obs(1.0, vec![perm(1, 5)], &[hold(1, "r1", Some("Deletes files"))]);
        let req = only(h.tick(61.0));
        assert_eq!(banner(&req).action, BannerAction::Review);
        assert!(!banner(&req).body.contains("npm test"));
    }

    #[test]
    fn a_tab_seen_in_between_does_not_escalate() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1)], &[]);
        h.obs(1.0, vec![question(1, 5)], &[]);
        h.view(30.0, true, &["t1"]);
        h.view(31.0, false, &["t1"]);
        assert!(h.tick(61.0).is_empty());
        assert!(h.tick(300.0).is_empty());
    }

    #[test]
    fn a_focused_window_with_another_tab_in_view_has_not_seen_this_one() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1), idle(2)], &[]);
        h.obs(1.0, vec![question(1, 5), idle(2)], &[]);
        h.view(30.0, true, &["t2"]);
        h.view(31.0, false, &["t2"]);
        assert_eq!(h.tick(61.0).len(), 1);
    }

    #[test]
    fn an_escalation_waits_while_the_window_is_in_focus() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1), idle(2)], &[]);
        h.obs(1.0, vec![question(1, 5), idle(2)], &[]);
        h.view(30.0, true, &["t2"]);
        assert!(h.tick(61.0).is_empty());
    }

    #[test]
    fn a_wait_that_ended_does_not_escalate() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1)], &[]);
        h.obs(1.0, vec![question(1, 5)], &[]);
        h.obs(30.0, vec![working(1, 6)], &[]);
        assert!(h.tick(61.0).is_empty());
    }

    #[test]
    fn a_new_wait_resets_the_escalation() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1)], &[]);
        h.obs(1.0, vec![perm(1, 5)], &[hold(1, "r1", None)]);
        h.obs(40.0, vec![perm(1, 5)], &[hold(1, "r2", None)]);
        assert!(h.tick(61.0).is_empty());
        let req = only(h.tick(100.0));
        assert_eq!(banner(&req).body, "s1 has waited 1 min to run a command");
    }

    #[test]
    fn an_escalation_in_quiet_hours_keeps_its_banner_only() {
        let mut h = H::new();
        h.settings.quiet_hours = QuietHours {
            enabled: true,
            start: "22:00".into(),
            end: "08:00".into(),
        };
        h.minute = 3 * 60;
        h.obs(0.0, vec![idle(1)], &[]);
        h.obs(1.0, vec![question(1, 5)], &[]);
        let req = only(h.tick(61.0));
        assert!(req.banner.is_some());
        assert_eq!((req.sound, req.flash), (None, false));
    }

    #[test]
    fn a_permission_waits_a_moment_for_its_hold_so_a_risky_one_is_never_told_apart_late() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1)], &[]);
        // The status folds before the hold opens.
        assert!(h.obs(1.0, vec![perm(1, 5)], &[]).is_empty());
        let req = only(h.obs(1.2, vec![perm(1, 5)], &[hold(1, "r1", Some("Deletes files"))]));
        assert_eq!(banner(&req).action, BannerAction::Review);
        assert_eq!(banner(&req).body, "Needs a careful look.");
        assert!(
            h.obs(1.4, vec![perm(1, 5)], &[hold(1, "r1", Some("Deletes files"))])
                .is_empty()
        );
    }

    #[test]
    fn a_permission_with_no_hold_is_told_after_the_wait() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1)], &[]);
        assert!(h.obs(1.0, vec![perm(1, 5)], &[]).is_empty());
        assert!(h.tick(1.4).is_empty());
        let req = only(h.tick(1.6));
        assert_eq!(banner(&req).body, "Wants to run `npm test`");
        assert!(h.tick(2.0).is_empty());
    }

    #[test]
    fn a_tab_that_goes_away_is_forgotten() {
        let mut h = H::new();
        h.obs(0.0, vec![idle(1), idle(2)], &[]);
        h.obs(1.0, vec![question(1, 5), idle(2)], &[]);
        assert!(h.obs(2.0, vec![idle(2)], &[]).is_empty());
        assert!(h.tick(61.0).is_empty());
    }
}
