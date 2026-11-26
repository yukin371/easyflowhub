//! One table that decides which key does what.
//!
//! Before this module every shortcut was a constant next to the code that acted
//! on it: four combinations in the controller, six `VK_*` numbers in the note's
//! key table. That works until the first person wants a different key -- and the
//! shipped build has a settings screen for exactly that reason (`shortcut.ts`
//! ships `Ctrl+K` for the switcher as a *default*, not as the feature). Rebinding
//! was impossible not because the key table was wrong but because there was no
//! single place that both the keyboard handler and the settings screen could
//! read, so the two would have disagreed the moment one of them gained an entry.
//!
//! ## What lives where
//!
//! * [`Command`] -- the actions, with the id the settings file uses, the label a
//!   tooltip shows, the scope, and the default chord.
//! * [`Chord`] -- modifiers plus a virtual-key code, parsed from and formatted to
//!   the `"Ctrl+Alt+N"` spelling the settings file and the tooltips share.
//! * [`Table`] -- defaults, overridden by the user's file, with conflicts found
//!   once at startup instead of silently at press time.
//!
//! ## The two rules that are not obvious
//!
//! **A chord needs a modifier.** A bare `K` would be a shortcut that eats every
//! `k` typed into the note, and the parser has no way to know which window a
//! binding belongs to. `Esc` and the function keys are the exceptions: `Esc` is
//! the shipped "get me out" key and closes with any modifier held (parity Q12),
//! and `F5` is not a character, so it cannot collide with typing.
//!
//! **Conflicts are detected, not prevented.** The file is hand-editable, so two
//! commands can end up on one chord. Refusing to start would be worse than
//! starting with the first command owning the key and the log saying which one
//! lost: the same audible outcome either way, but one of them leaves a window
//! the user can fix. [`Table::shadowed`] is that list, and the caller uses it to
//! skip registering the loser.

use std::collections::BTreeMap;

const VK_ESCAPE: u16 = 0x1B;

/// Which key delivery the command belongs to.
///
/// A *global* command is registered with the OS and fires while another program
/// has the focus. A *window* command is resolved from a keystroke that reached
/// one of our windows. The split matters twice: the two tables must not shadow
/// each other (a window chord that equals a global one can never fire), and the
/// settings screen groups them because a global combination can fail to register
/// for reasons outside our control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Global,
    Window,
}

/// One rebindable action.
///
/// The discriminants are stable because they are what a window stores in an
/// atomic slot between the keystroke and the next frame; renumbering would turn a
/// pending pin into a pending close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Command {
    NewNote = 1,
    CloseAll = 2,
    ToggleVisible = 3,
    OpenManager = 4,
    Switcher = 5,
    CloseNote = 6,
    TogglePin = 7,
    ToggleTools = 8,
    ToggleGhost = 9,
    Undo = 10,
    Redo = 11,
}

impl Command {
    /// Every command, in the order the table resolves them.
    ///
    /// The order is the tie-break for a conflict and the order the settings
    /// screen lists them, so it is data, not an implementation detail: `Switcher`
    /// comes before `TogglePin` because the switcher is the more specific
    /// gesture, and a user who binds both to `Ctrl+K` should lose the pin, not
    /// the switcher.
    pub const ALL: [Command; 11] = [
        Command::NewNote,
        Command::CloseAll,
        Command::ToggleVisible,
        Command::OpenManager,
        Command::Switcher,
        Command::CloseNote,
        Command::TogglePin,
        Command::ToggleTools,
        Command::ToggleGhost,
        Command::Undo,
        Command::Redo,
    ];

    /// The key in the settings file. snake_case, and never renamed once shipped:
    /// a rename silently drops the user's binding for that command.
    pub fn id(self) -> &'static str {
        match self {
            Command::NewNote => "new_note",
            Command::CloseAll => "close_all",
            Command::ToggleVisible => "toggle_visible",
            Command::OpenManager => "open_manager",
            Command::Switcher => "switcher",
            Command::CloseNote => "close_note",
            Command::TogglePin => "toggle_pin",
            Command::ToggleTools => "toggle_tools",
            Command::ToggleGhost => "toggle_ghost",
            Command::Undo => "undo",
            Command::Redo => "redo",
        }
    }

    /// What a tooltip calls it. Chinese, like the rest of the visible chrome.
    pub fn label(self) -> &'static str {
        match self {
            Command::NewNote => "新建笔记",
            Command::CloseAll => "关闭全部笔记",
            Command::ToggleVisible => "最小化 / 恢复全部笔记",
            Command::OpenManager => "打开工作台",
            Command::Switcher => "笔记切换器",
            Command::CloseNote => "关闭当前笔记",
            Command::TogglePin => "置顶窗口",
            Command::ToggleTools => "工具面板",
            Command::ToggleGhost => "幽灵模式",
            Command::Undo => "撤销",
            Command::Redo => "重做",
        }
    }

    pub fn scope(self) -> Scope {
        match self {
            Command::NewNote
            | Command::CloseAll
            | Command::ToggleVisible
            | Command::OpenManager => Scope::Global,
            _ => Scope::Window,
        }
    }

    /// The shipped default. Kept identical to the old constants on purpose: a
    /// default that moved would feel like a bug to anyone who learned the keys.
    pub fn default_chord(self) -> Chord {
        let chord = match self {
            Command::NewNote => "Ctrl+Alt+N",
            Command::CloseAll => "Ctrl+Alt+D",
            Command::ToggleVisible => "Ctrl+Alt+H",
            Command::OpenManager => "Ctrl+Alt+M",
            Command::Switcher => "Ctrl+K",
            Command::CloseNote => "Ctrl+D",
            Command::TogglePin => "Ctrl+T",
            Command::ToggleTools => "Ctrl+.",
            Command::ToggleGhost => "Ctrl+Shift+H",
            Command::Undo => "Ctrl+Z",
            Command::Redo => "Ctrl+Shift+Z",
        };
        // The defaults are unit-tested through `parse`; a typo here must fail the
        // test run rather than produce a command with no key.
        Chord::parse(chord).unwrap_or(Chord {
            ctrl: true,
            alt: false,
            shift: false,
            vk: b'N' as u16,
        })
    }

    pub fn code(self) -> u8 {
        self as u8
    }

    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.iter().copied().find(|command| command.code() == code)
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|command| command.id() == id)
    }
}

/// Modifiers plus a virtual-key code.
///
/// The virtual-key numbers are the platform's, but nothing here calls the
/// platform: the table is compared against codes the callers already carry, and
/// keeping the numbers as data is what lets this crate stay free of OS bindings.
/// The Win key is deliberately unrepresentable -- it is reserved by the shell for
/// its own combinations, so a chord that claimed it would be accepted by the
/// settings file and then never arrive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub vk: u16,
}

impl Chord {
    pub fn new(ctrl: bool, alt: bool, shift: bool, vk: u16) -> Self {
        Self {
            ctrl,
            alt,
            shift,
            vk,
        }
    }

    /// `"Ctrl+Alt+N"` → chord.
    ///
    /// Accepted spellings are case-insensitive and tolerate spaces, because the
    /// settings file is hand-edited: `"ctrl + alt + n"` is the same binding.
    /// `"Cmd"`/`"Meta"`/`"Win"` are rejected rather than ignored -- a chord that
    /// quietly loses its modifier is a different chord, and the user would only
    /// find out by pressing it.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        let mut vk: Option<u16> = None;

        for part in text.split('+') {
            let part = part.trim();
            if part.is_empty() {
                return Err(format!("{text}: empty component"));
            }
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => ctrl = true,
                "alt" => alt = true,
                "shift" => shift = true,
                "win" | "super" | "meta" | "cmd" | "command" => {
                    return Err(format!("{text}: the system key is reserved"));
                }
                other => {
                    if vk.is_some() {
                        return Err(format!("{text}: more than one non-modifier key"));
                    }
                    vk = Some(
                        key_code(other)
                            .ok_or_else(|| format!("{text}: unsupported key '{other}'"))?,
                    );
                }
            }
        }

        let vk = vk.ok_or_else(|| format!("{text}: no non-modifier key"))?;
        // A bare letter would eat typing; a function key is not a character, so it
        // stands alone the way it does everywhere else. `Esc` is the other
        // exception, and it is the reason the rule cannot simply be "modifier
        // required": `Esc` closes a note today and has to keep doing that.
        let standalone = vk == VK_ESCAPE || (0x70..=0x87).contains(&vk);
        if !ctrl && !alt && !shift && !standalone {
            return Err(format!(
                "{text}: a shortcut needs Ctrl, Alt or Shift (a bare key would eat typing)"
            ));
        }
        Ok(Self {
            ctrl,
            alt,
            shift,
            vk,
        })
    }

    /// The spelling shared by the settings file and every tooltip.
    ///
    /// One spelling for both directions is the whole point: a tooltip that says
    /// something the file cannot express is how "I changed it and nothing
    /// happened" starts. Round-tripping is unit-tested for every command.
    pub fn display(&self) -> String {
        let mut out = String::new();
        if self.ctrl {
            out.push_str("Ctrl+");
        }
        if self.alt {
            out.push_str("Alt+");
        }
        if self.shift {
            out.push_str("Shift+");
        }
        out.push_str(key_name(self.vk).as_str());
        out
    }
}

/// Name → virtual-key code. Letters and digits are handled arithmetically so the
/// table below only has to carry what cannot be derived.
fn key_code(name: &str) -> Option<u16> {
    let upper = name.to_ascii_uppercase();
    if upper.len() == 1 {
        let byte = upper.as_bytes()[0];
        if byte.is_ascii_alphabetic() {
            return Some(byte as u16);
        }
        if byte.is_ascii_digit() {
            // Not contiguous with the letters: `1` is 0x31, not `A` + 1.
            return Some(byte as u16);
        }
    }
    if let Some(rest) = upper.strip_prefix('F') {
        if let Ok(number) = rest.parse::<u16>() {
            if (1..=24).contains(&number) {
                return Some(0x70 + number - 1);
            }
            return None;
        }
    }
    let code = match upper.as_str() {
        "ESC" | "ESCAPE" => VK_ESCAPE,
        "SPACE" => 0x20,
        "ENTER" | "RETURN" => 0x0D,
        "TAB" => 0x09,
        "BACKSPACE" => 0x08,
        "DELETE" | "DEL" => 0x2E,
        "INSERT" => 0x2D,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" | "PGUP" => 0x21,
        "PAGEDOWN" | "PGDN" => 0x22,
        "UP" => 0x26,
        "DOWN" => 0x28,
        "LEFT" => 0x25,
        "RIGHT" => 0x27,
        "." | "PERIOD" => 0xBE,
        "," | "COMMA" => 0xBC,
        ";" | "SEMICOLON" => 0xBA,
        "'" | "QUOTE" => 0xDE,
        "/" | "SLASH" => 0xBF,
        "`" | "GRAVE" | "BACKTICK" => 0xC0,
        "-" | "MINUS" => 0xBD,
        "=" | "EQUALS" => 0xBB,
        "[" | "BRACKETLEFT" | "OPENBRACKET" => 0xDB,
        "]" | "BRACKETRIGHT" | "CLOSEBRACKET" => 0xDD,
        "\\" | "BACKSLASH" => 0xDC,
        _ => return None,
    };
    Some(code)
}

/// The inverse of [`key_code`], for the settings file and the tooltips.
///
/// Unknown codes come back as `VK_…` rather than a panic: the table can hold a
/// chord that came from a file, and the display path must never be the thing that
/// takes the window down.
fn key_name(vk: u16) -> String {
    match vk {
        VK_ESCAPE => return "Esc".to_string(),
        0x20 => return "Space".to_string(),
        0x0D => return "Enter".to_string(),
        0x09 => return "Tab".to_string(),
        0x08 => return "Backspace".to_string(),
        0x2E => return "Delete".to_string(),
        0x2D => return "Insert".to_string(),
        0x24 => return "Home".to_string(),
        0x23 => return "End".to_string(),
        0x21 => return "PageUp".to_string(),
        0x22 => return "PageDown".to_string(),
        0x26 => return "Up".to_string(),
        0x28 => return "Down".to_string(),
        0x25 => return "Left".to_string(),
        0x27 => return "Right".to_string(),
        0xBE => return ".".to_string(),
        0xBC => return ",".to_string(),
        0xBA => return ";".to_string(),
        0xDE => return "'".to_string(),
        0xBF => return "/".to_string(),
        0xC0 => return "`".to_string(),
        0xBD => return "-".to_string(),
        0xBB => return "=".to_string(),
        0xDB => return "[".to_string(),
        0xDD => return "]".to_string(),
        0xDC => return "\\".to_string(),
        0x70..=0x87 => return format!("F{}", vk - 0x70 + 1),
        _ => {}
    }
    if (b'A' as u16..=b'Z' as u16).contains(&vk) {
        return char::from_u32(vk as u32).unwrap_or('?').to_string();
    }
    if (b'0' as u16..=b'9' as u16).contains(&vk) {
        return char::from_u32(vk as u32).unwrap_or('?').to_string();
    }
    format!("VK_{vk:#04X}")
}

/// The resolved table.
#[derive(Debug, Clone)]
pub struct Table {
    /// Indexed by [`Command::code`] minus one, so a lookup is an array read
    /// rather than a scan on the keystroke path.
    chords: [Chord; Command::ALL.len()],
    /// Human-readable reasons the file did not fully apply: unknown ids, bad
    /// chords, conflicts. The controller logs them once at startup.
    notes: Vec<String>,
}

impl Default for Table {
    fn default() -> Self {
        Self::defaults()
    }
}

impl Table {
    pub fn defaults() -> Self {
        let mut chords = [Command::NewNote.default_chord(); Command::ALL.len()];
        for command in Command::ALL {
            chords[command.code() as usize - 1] = command.default_chord();
        }
        Self {
            chords,
            notes: Vec::new(),
        }
    }

    /// Defaults, then the user's overrides.
    ///
    /// Nothing in here can fail: a file with a typo produces defaults plus a
    /// note, because refusing to start over a key binding would be the worst of
    /// the available outcomes.
    pub fn build(overrides: &BTreeMap<String, String>) -> Self {
        let mut table = Self::defaults();
        for (id, text) in overrides {
            let Some(command) = Command::from_id(id) else {
                table
                    .notes
                    .push(format!("unknown command '{id}' (kept out of the table)"));
                continue;
            };
            match Chord::parse(text) {
                Ok(chord) => table.chords[command.code() as usize - 1] = chord,
                Err(error) => table
                    .notes
                    .push(format!("{}: {error} (default kept)", command.id())),
            }
        }
        for command in Command::ALL {
            if table.shadowed_by_esc(command) {
                table.notes.push(format!(
                    "{}: Esc is fixed and cannot be rebound (default kept)",
                    command.id()
                ));
            }
        }
        for command in Command::ALL {
            if let Some(winner) = table.shadowing(command) {
                table.notes.push(format!(
                    "conflict: {} and {} both claim {}; {} wins ({} is unreachable)",
                    command.id(),
                    winner.id(),
                    table.chord(command).display(),
                    winner.id(),
                    command.id()
                ));
            }
        }
        table
    }

    pub fn chord(&self, command: Command) -> Chord {
        self.chords[command.code() as usize - 1]
    }

    /// The tooltip spelling of a command's key.
    pub fn hint(&self, command: Command) -> String {
        self.chord(command).display()
    }

    /// Which command a keystroke means, or `None` to let the control have it.
    ///
    /// `None` is the important answer: a typed letter must reach the text, and
    /// every caller relies on "not mine" being distinguishable from "mine".
    pub fn find(&self, scope: Scope, chord: Chord) -> Option<Command> {
        // Esc closes regardless of modifiers, and it is not rebindable (parity
        // Q12: the shipped handler never reads the modifier flags). Handled
        // before the scan so a user who binds Esc elsewhere cannot break the one
        // key that has to always work.
        if scope == Scope::Window && chord.vk == VK_ESCAPE {
            return Some(Command::CloseNote);
        }
        Command::ALL.into_iter().find(|command| {
            command.scope() == scope && self.chord(*command) == chord
        })
    }

    /// Every command that lost a chord to an earlier command in its scope.
    ///
    /// Two commands on one key cannot both fire; the caller uses this to skip
    /// registering the loser rather than to refuse the whole set.
    pub fn shadowed(&self) -> Vec<Command> {
        Command::ALL
            .into_iter()
            .filter(|command| self.shadowing(*command).is_some())
            .collect()
    }

    fn shadowing(&self, command: Command) -> Option<Command> {
        let chord = self.chord(command);
        Command::ALL
            .into_iter()
            .take_while(|other| *other != command)
            .find(|other| other.scope() == command.scope() && self.chord(*other) == chord)
    }

    fn shadowed_by_esc(&self, command: Command) -> bool {
        command.scope() == Scope::Window
            && command != Command::CloseNote
            && self.chord(command).vk == VK_ESCAPE
    }

    /// Resolutions the file asked for but did not get, in the order found.
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// Rebind one command, keeping everything else. Used by the settings screen
    /// to show the result before it is written to disk.
    pub fn rebind(&self, command: Command, chord: Chord) -> Self {
        let mut next = self.clone();
        next.chords[command.code() as usize - 1] = chord;
        next.notes.clear();
        next
    }

    /// `(command, chord)` for the global scope, in table order.
    pub fn globals(&self) -> Vec<(Command, Chord)> {
        Command::ALL
            .into_iter()
            .filter(|command| command.scope() == Scope::Global)
            .map(|command| (command, self.chord(command)))
            .collect()
    }

    /// The same for a window, which is what the note's keyboard handler walks.
    pub fn window(&self) -> Vec<(Command, Chord)> {
        Command::ALL
            .into_iter()
            .filter(|command| command.scope() == Scope::Window)
            .map(|command| (command, self.chord(command)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_round_trips_through_parse_and_display() {
        // The two directions share one spelling. If they ever disagree, a tooltip
        // would show a key the settings file cannot express.
        for command in Command::ALL {
            let text = command.default_chord().display();
            let parsed = Chord::parse(&text).unwrap_or_else(|error| {
                panic!("{}: default {text} does not parse: {error}", command.id())
            });
            assert_eq!(parsed, command.default_chord(), "{}", command.id());
        }
    }

    #[test]
    fn the_defaults_are_the_shipped_combinations() {
        assert_eq!(Command::NewNote.default_chord().display(), "Ctrl+Alt+N");
        assert_eq!(Command::Switcher.default_chord().display(), "Ctrl+K");
        assert_eq!(Command::ToggleGhost.default_chord().display(), "Ctrl+Shift+H");
        assert_eq!(Command::ToggleTools.default_chord().display(), "Ctrl+.");
    }

    #[test]
    fn a_global_and_a_window_command_can_share_a_chord_without_a_conflict() {
        // They are delivered by different paths, so neither shadows the other.
        let table = Table::defaults();
        assert!(table.shadowed().is_empty());
        assert_eq!(
            table.find(Scope::Global, Chord::parse("Ctrl+Alt+N").unwrap()),
            Some(Command::NewNote)
        );
        assert_eq!(
            table.find(Scope::Window, Chord::parse("Ctrl+Alt+N").unwrap()),
            None,
            "a global combination is not a window shortcut"
        );
    }

    #[test]
    fn a_typed_letter_is_not_a_shortcut() {
        let table = Table::defaults();
        let plain = Chord::new(false, false, false, b'N' as u16);
        assert_eq!(table.find(Scope::Window, plain), None);
        assert_eq!(table.find(Scope::Global, plain), None);
    }

    #[test]
    fn escape_always_closes_the_window_scope_and_never_the_global_one() {
        let table = Table::defaults();
        let esc = Chord::new(false, false, false, VK_ESCAPE);
        assert_eq!(table.find(Scope::Window, esc), Some(Command::CloseNote));
        // The shipped handler ignores modifiers, so Ctrl+Esc closes too.
        let ctrl_esc = Chord::new(true, false, true, VK_ESCAPE);
        assert_eq!(table.find(Scope::Window, ctrl_esc), Some(Command::CloseNote));
        assert_eq!(
            table.find(Scope::Global, esc),
            None,
            "Esc must not be registered globally: it would eat the key system-wide"
        );
    }

    #[test]
    fn an_override_moves_one_command_and_leaves_the_rest_alone() {
        let mut overrides = BTreeMap::new();
        overrides.insert("switcher".to_string(), "Ctrl+Shift+K".to_string());
        let table = Table::build(&overrides);

        assert_eq!(
            table.find(Scope::Window, Chord::parse("Ctrl+Shift+K").unwrap()),
            Some(Command::Switcher)
        );
        assert_eq!(
            table.find(Scope::Window, Chord::parse("Ctrl+K").unwrap()),
            None,
            "the old chord must stop working"
        );
        assert_eq!(table.chord(Command::TogglePin).display(), "Ctrl+T");
        assert!(table.notes().is_empty(), "{:?}", table.notes());
    }

    #[test]
    fn an_unknown_id_is_reported_and_not_invented() {
        let mut overrides = BTreeMap::new();
        overrides.insert("open_the_pod_bay".to_string(), "Ctrl+P".to_string());
        let table = Table::build(&overrides);
        assert_eq!(table.notes().len(), 1);
        assert!(table.notes()[0].contains("open_the_pod_bay"));
        assert_eq!(table.chord(Command::Switcher).display(), "Ctrl+K");
    }

    #[test]
    fn a_bad_chord_keeps_the_default_and_says_why() {
        let mut overrides = BTreeMap::new();
        overrides.insert("switcher".to_string(), "K".to_string());
        let table = Table::build(&overrides);
        assert_eq!(table.chord(Command::Switcher).display(), "Ctrl+K");
        assert!(
            table.notes().iter().any(|note| note.contains("needs Ctrl")),
            "{:?}",
            table.notes()
        );
    }

    #[test]
    fn a_conflict_names_both_sides_and_keeps_the_earlier_command() {
        let mut overrides = BTreeMap::new();
        overrides.insert("toggle_pin".to_string(), "Ctrl+K".to_string());
        let table = Table::build(&overrides);

        assert_eq!(table.shadowed(), vec![Command::TogglePin]);
        assert_eq!(
            table.find(Scope::Window, Chord::parse("Ctrl+K").unwrap()),
            Some(Command::Switcher),
            "the switcher comes earlier in the table and wins"
        );
        let note = table
            .notes()
            .iter()
            .find(|note| note.starts_with("conflict"))
            .unwrap_or_else(|| panic!("no conflict note: {:?}", table.notes()));
        assert!(note.contains("switcher") && note.contains("toggle_pin"), "{note}");
    }

    #[test]
    fn rebinding_to_escape_is_refused_because_escape_is_fixed() {
        let mut overrides = BTreeMap::new();
        overrides.insert("toggle_pin".to_string(), "Esc".to_string());
        let table = Table::build(&overrides);
        assert!(
            table.notes().iter().any(|note| note.contains("Esc is fixed")),
            "{:?}",
            table.notes()
        );
        // And the fixed key still closes rather than pinning.
        assert_eq!(
            table.find(
                Scope::Window,
                Chord::new(false, false, false, VK_ESCAPE)
            ),
            Some(Command::CloseNote)
        );
    }

    #[test]
    fn modifiers_are_case_insensitive_and_spaces_are_tolerated() {
        let a = Chord::parse("ctrl + alt + n").unwrap();
        let b = Chord::parse("Ctrl+Alt+N").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.display(), "Ctrl+Alt+N");
    }

    #[test]
    fn the_system_key_is_rejected_instead_of_dropped() {
        // Silently losing the modifier would bind `Ctrl+K` when the user wrote
        // `Win+K`, and they would only find out by pressing it.
        assert!(Chord::parse("Win+K").is_err());
        assert!(Chord::parse("Cmd+K").is_err());
    }

    #[test]
    fn punctuation_and_function_keys_have_stable_names() {
        for text in ["Ctrl+.", "Ctrl+,", "Ctrl+/", "Ctrl+`", "Ctrl+[", "F5", "Alt+F4"] {
            let chord = Chord::parse(text).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(chord.display(), text);
        }
    }

    #[test]
    fn digits_are_not_letters() {
        // `A` is 0x41 and `1` is 0x31; deriving one from the other is the classic
        // off-by-16 that makes Ctrl+1 fire on Ctrl+Q.
        let one = Chord::parse("Ctrl+1").unwrap();
        assert_eq!(one.vk, 0x31);
        assert_eq!(one.display(), "Ctrl+1");
    }

    #[test]
    fn the_escape_chord_is_representable_but_not_assignable() {
        // The parser accepts it so the file can spell it; `build` rejects it for
        // any command but the fixed close.
        assert!(Chord::parse("Esc").is_ok());
        assert!(Chord::parse("Ctrl+Shift+Esc").is_ok());
    }

    #[test]
    fn shadowed_is_empty_on_a_table_with_no_duplicates() {
        let table = Table::defaults();
        assert_eq!(table.shadowed().len(), 0);
        assert_eq!(table.globals().len(), 4);
        assert_eq!(table.window().len(), 7);
    }

    #[test]
    fn an_unknown_key_code_still_renders_something() {
        // A chord read from a file can outlive the code that knew it. The display
        // path must not be the thing that panics.
        assert_eq!(Chord::new(true, false, false, 0x0201).display(), "Ctrl+VK_0x201");
    }
}
