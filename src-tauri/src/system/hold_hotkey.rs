use std::collections::BTreeSet;

pub fn normalize(key: u32) -> u32 {
    match key {
        160 | 161 => 16,
        162 | 163 => 17,
        164 | 165 => 18,
        91 | 92 => 91,
        _ => key,
    }
}
pub fn modifier(key: u32) -> bool {
    matches!(normalize(key), 16 | 17 | 18 | 91)
}
// Mouse buttons use private codes outside the Windows virtual-key range.
pub const MIDDLE: u32 = 256;
pub const X1: u32 = 257;
pub const X2: u32 = 258;
fn token(key: u32) -> String {
    match key {
        MIDDLE => "mouse_middle".into(),
        X1 => "mouse_x1".into(),
        X2 => "mouse_x2".into(),
        _ => key.to_string(),
    }
}
pub fn encode(keys: &BTreeSet<u32>) -> String {
    if keys.len() == 1 {
        let key = *keys.first().unwrap();
        return if key >= MIDDLE {
            token(key)
        } else {
            format!("key_{key}")
        };
    }
    format!(
        "chord_{}",
        keys.iter()
            .map(|key| token(*key))
            .collect::<Vec<_>>()
            .join("+")
    )
}
pub fn parse(value: &str) -> BTreeSet<u32> {
    let value = match value {
        "keyboard_f10" => "key_121",
        "keyboard_caps" => "key_20",
        "keyboard_scroll" => "key_145",
        "keyboard_insert" => "key_45",
        _ => value,
    };
    let raw = value
        .strip_prefix("chord_")
        .or_else(|| value.strip_prefix("key_"))
        .unwrap_or(value);
    raw.split('+')
        .filter_map(|part| match part {
            "mouse_middle" => Some(MIDDLE),
            "mouse_x1" => Some(X1),
            "mouse_x2" => Some(X2),
            _ => part
                .parse::<u32>()
                .ok()
                .filter(|key| (1..=255).contains(key))
                .map(normalize),
        })
        .collect()
}
pub fn valid(value: &str) -> bool {
    if value == "unassigned" {
        return true;
    }
    if let Some(code) = value
        .strip_prefix("key_")
        .and_then(|code| code.parse::<u32>().ok())
    {
        return (1..=255).contains(&code);
    }
    let keys = parse(value);
    !keys.is_empty() && keys.len() <= 4 && encode(&keys) == value
        || matches!(
            value,
            "keyboard_f10" | "keyboard_caps" | "keyboard_scroll" | "keyboard_insert"
        )
}

#[derive(Debug, PartialEq)]
pub enum Action {
    Start,
    Stop,
    Captured(String),
    CancelCapture,
}
#[derive(Default)]
pub struct HoldState {
    pub configured: BTreeSet<u32>,
    pressed: BTreeSet<u32>, // Preserve physical sides: releasing RCtrl must not release LCtrl.
    consumed: BTreeSet<u32>,
    active: bool,
    armed: bool,
    capturing: bool,
    candidate: BTreeSet<u32>,
}
impl HoldState {
    pub fn configure(&mut self, value: &str) -> Vec<Action> {
        let configured = parse(value);
        if configured == self.configured {
            return vec![];
        }
        self.configured = configured;
        self.armed = self.pressed.is_empty();
        if std::mem::take(&mut self.active) {
            vec![Action::Stop]
        } else {
            vec![]
        }
    }
    pub fn capture(&mut self, enabled: bool) -> Vec<Action> {
        self.capturing = enabled;
        self.candidate.clear();
        self.armed = !enabled && self.pressed.is_empty();
        if std::mem::take(&mut self.active) {
            vec![Action::Stop]
        } else {
            vec![]
        }
    }
    pub fn event(&mut self, key: u32, down: bool) -> (bool, Vec<Action>) {
        let was_pressed = self.pressed.contains(&key);
        let was_consumed = self.consumed.contains(&key);
        if down {
            self.pressed.insert(key);
        } else {
            self.pressed.remove(&key);
            self.consumed.remove(&key);
        }
        let held: BTreeSet<_> = self.pressed.iter().copied().map(normalize).collect();
        let mut actions = vec![];
        if self.capturing {
            if key == 27 && down {
                self.capturing = false;
                self.candidate.clear();
                actions.push(Action::CancelCapture);
            } else {
                if held.len() > self.candidate.len() && held.len() <= 4 {
                    self.candidate = held.clone();
                }
                if !down && held.is_empty() && !self.candidate.is_empty() {
                    self.capturing = false;
                    actions.push(Action::Captured(encode(&self.candidate)));
                    self.candidate.clear();
                    self.armed = true;
                }
            }
            if down {
                self.consumed.insert(key);
            }
            return (true, actions);
        }
        if self.active && !self.configured.is_subset(&held) {
            self.active = false;
            self.armed = false;
            actions.push(Action::Stop);
        }
        if self.configured.is_disjoint(&held) {
            self.armed = true;
        }
        // Require a non-repeat edge and exactly the configured keys. Extra modifiers
        // must not turn Ctrl+Shift+Q into the configured Ctrl+Q shortcut.
        if down
            && !was_pressed
            && self.armed
            && !self.configured.is_empty()
            && held == self.configured
        {
            self.active = true;
            self.armed = false;
            actions.push(Action::Start);
            if !modifier(key) || self.configured.len() == 1 {
                self.consumed.insert(key);
            }
        }
        (was_consumed || self.consumed.contains(&key), actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(value: &str) -> HoldState {
        let mut state = HoldState::default();
        state.configure(value);
        state
    }
    #[test]
    fn release_either_key_stops_once() {
        for release in [162, 81] {
            let mut s = state("chord_17+81");
            assert!(s.event(162, true).1.is_empty());
            assert_eq!(s.event(81, true).1, vec![Action::Start]);
            assert_eq!(s.event(release, false).1, vec![Action::Stop]);
            assert!(s.event(release, false).1.is_empty());
        }
    }
    #[test]
    fn repeats_and_partial_repress_do_not_restart() {
        let mut s = state("chord_17+81");
        s.event(162, true);
        s.event(81, true);
        assert!(s.event(81, true).1.is_empty());
        s.event(81, false);
        assert!(s.event(81, true).1.is_empty());
        s.event(81, false);
        s.event(162, false);
        s.event(81, true);
        assert_eq!(s.event(163, true).1, vec![Action::Start]);
    }
    #[test]
    fn capture_waits_for_full_chord_release() {
        let mut s = state("key_121");
        s.capture(true);
        assert!(s.event(162, true).1.is_empty());
        s.event(81, true);
        assert!(s.event(162, false).1.is_empty());
        assert_eq!(
            s.event(81, false).1,
            vec![Action::Captured("chord_17+81".into())]
        );
    }
    #[test]
    fn modifier_sides_and_config_changes() {
        let mut s = state("key_17");
        s.event(162, true);
        s.event(163, true);
        assert!(s.event(163, false).1.is_empty());
        assert_eq!(s.configure("key_121"), vec![Action::Stop]);
        assert!(s.event(162, false).1.is_empty());
    }
    #[test]
    fn mouse_and_modifier_release() {
        let mut s = state("chord_17+mouse_x1");
        s.event(162, true);
        assert_eq!(s.event(X1, true).1, vec![Action::Start]);
        assert_eq!(s.event(162, false).1, vec![Action::Stop]);
        assert!(s.event(X1, false).0);
    }
    #[test]
    fn extra_modifier_and_single_key_compatibility() {
        let mut s = state("chord_17+81");
        s.event(162, true);
        s.event(160, true);
        assert!(s.event(81, true).1.is_empty());
        let mut s = state("keyboard_f10");
        assert_eq!(s.event(121, true).1, vec![Action::Start]);
        assert_eq!(s.event(121, false).1, vec![Action::Stop]);
    }
    #[test]
    fn capture_cancel_and_single_modifier() {
        let mut s = state("key_121");
        s.capture(true);
        s.event(163, true);
        assert_eq!(
            s.event(163, false).1,
            vec![Action::Captured("key_17".into())]
        );
        s.capture(true);
        assert_eq!(s.event(27, true).1, vec![Action::CancelCapture]);
    }
}
