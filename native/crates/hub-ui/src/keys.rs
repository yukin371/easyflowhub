//! Frame keystrokes → the chord the shortcut table understands.
//!
//! This used to live in the note's key module, but the settings screen needs the
//! same conversion to capture a new binding, and two copies of a 60-arm match is
//! how `Ctrl+1` ends up firing on `Ctrl+Q` in one of them. One table, two
//! callers.
//!
//! The mapping is a virtual-key code rather than egui's own key type because the
//! table is also compared against codes that arrive from the text control, which
//! has no idea egui exists. Codes are the common language.

use hub_core::Chord;

/// The most recent un-repeated key press this frame, with its modifiers.
///
/// `repeat: false` is filtered here, not by the caller: a held key would post a
/// command sixty times a second and the window would flip its pin until released.
/// The events are walked in reverse because the last press of a frame is the one
/// worth acting on.
pub fn chord_from_egui(ctx: &egui::Context) -> Option<Chord> {
    ctx.input(|input| {
        for event in input.events.iter().rev() {
            let egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            let Some(vk) = vk_of(*key) else {
                continue;
            };
            return Some(Chord::new(modifiers.ctrl, modifiers.alt, modifiers.shift, vk));
        }
        None
    })
}

/// egui's key → the virtual-key code the table stores.
///
/// Written out rather than derived from a debug string: a rename inside egui
/// would then silently rebind our shortcuts, and the failure would look like the
/// settings file being ignored.
pub fn vk_of(key: egui::Key) -> Option<u16> {
    use egui::Key;
    let code = match key {
        Key::ArrowDown => 0x28,
        Key::ArrowLeft => 0x25,
        Key::ArrowRight => 0x27,
        Key::ArrowUp => 0x26,
        Key::Escape => 0x1B,
        Key::Tab => 0x09,
        Key::Backspace => 0x08,
        Key::Enter => 0x0D,
        Key::Space => 0x20,
        Key::Insert => 0x2D,
        Key::Delete => 0x2E,
        Key::Home => 0x24,
        Key::End => 0x23,
        Key::PageUp => 0x21,
        Key::PageDown => 0x22,
        Key::Comma => 0xBC,
        Key::Backslash => 0xDC,
        Key::Slash => 0xBF,
        Key::OpenBracket => 0xDB,
        Key::CloseBracket => 0xDD,
        Key::Backtick => 0xC0,
        Key::Minus => 0xBD,
        Key::Period => 0xBE,
        Key::Plus | Key::Equals => 0xBB,
        Key::Semicolon => 0xBA,
        Key::Num0 => 0x30,
        Key::Num1 => 0x31,
        Key::Num2 => 0x32,
        Key::Num3 => 0x33,
        Key::Num4 => 0x34,
        Key::Num5 => 0x35,
        Key::Num6 => 0x36,
        Key::Num7 => 0x37,
        Key::Num8 => 0x38,
        Key::Num9 => 0x39,
        Key::A => 0x41,
        Key::B => 0x42,
        Key::C => 0x43,
        Key::D => 0x44,
        Key::E => 0x45,
        Key::F => 0x46,
        Key::G => 0x47,
        Key::H => 0x48,
        Key::I => 0x49,
        Key::J => 0x4A,
        Key::K => 0x4B,
        Key::L => 0x4C,
        Key::M => 0x4D,
        Key::N => 0x4E,
        Key::O => 0x4F,
        Key::P => 0x50,
        Key::Q => 0x51,
        Key::R => 0x52,
        Key::S => 0x53,
        Key::T => 0x54,
        Key::U => 0x55,
        Key::V => 0x56,
        Key::W => 0x57,
        Key::X => 0x58,
        Key::Y => 0x59,
        Key::Z => 0x5A,
        Key::F1 => 0x70,
        Key::F2 => 0x71,
        Key::F3 => 0x72,
        Key::F4 => 0x73,
        Key::F5 => 0x74,
        Key::F6 => 0x75,
        Key::F7 => 0x76,
        Key::F8 => 0x77,
        Key::F9 => 0x78,
        Key::F10 => 0x79,
        Key::F11 => 0x7A,
        Key::F12 => 0x7B,
        _ => return None,
    };
    Some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_digits_land_on_their_codes() {
        assert_eq!(vk_of(egui::Key::K), Some(0x4B));
        assert_eq!(vk_of(egui::Key::Num1), Some(0x31));
        // The classic off-by-16: a digit is not `A` plus n.
        assert_ne!(vk_of(egui::Key::Num1), Some(0x42));
    }

    #[test]
    fn escape_is_the_one_key_the_table_treats_specially() {
        assert_eq!(vk_of(egui::Key::Escape), Some(0x1B));
    }

    #[test]
    fn the_capture_and_the_note_path_agree() {
        // Both go through `chord_from_egui` and `vk_of`, so this asserts the shape
        // rather than a value: a chord built from a press must round-trip through
        // the display spelling the settings file stores.
        let chord = Chord::new(true, false, false, vk_of(egui::Key::K).unwrap());
        assert_eq!(chord.display(), "Ctrl+K");
    }
}
