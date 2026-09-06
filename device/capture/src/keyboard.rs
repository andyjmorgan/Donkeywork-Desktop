use crate::*;
use x11rb::protocol::{
    xkb::{self, ConnectionExt as _},
    xproto::ConnectionExt as _,
};

impl X11Backend {
    pub(crate) fn hid_keycode(&self, usage: u16) -> Result<u8> {
        let name = hid_name(usage)
            .ok_or_else(|| BackendError::new(ErrorKind::Unsupported, "HID usage is unsupported"))?;
        let reply = self
            .connection
            .xkb_get_names(u16::from(xkb::ID::USE_CORE_KBD), xkb::NameDetail::KEY_NAMES)
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        let names = reply.value_list.key_names.ok_or_else(|| {
            BackendError::new(ErrorKind::Unsupported, "XKB key names unavailable")
        })?;
        let index = names
            .iter()
            .position(|key| key.name == name)
            .ok_or_else(|| {
                BackendError::new(
                    ErrorKind::Unsupported,
                    "physical key missing from XKB keymap",
                )
            })?;
        u8::try_from(usize::from(reply.first_key) + index)
            .map_err(|_| BackendError::new(ErrorKind::Unsupported, "invalid XKB keycode"))
    }

    pub(crate) fn type_text(&mut self, text: &str, guard: &mut dyn ActionGuard) -> Result<()> {
        if text.len() > 16 * 1024 {
            return Err(BackendError::new(
                ErrorKind::ResourceExhausted,
                "text request exceeds limit",
            ));
        }
        if !self.held_keys.is_empty() {
            return Err(BackendError::new(
                ErrorKind::Unsupported,
                "release held keys before text insertion",
            ));
        }
        self.ensure_text_state()?;
        let map = self
            .connection
            .xkb_get_map(
                u16::from(xkb::ID::USE_CORE_KBD),
                xkb::MapPart::KEY_TYPES | xkb::MapPart::KEY_SYMS,
                xkb::MapPart::default(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                xkb::VMod::default(),
                0,
                0,
                0,
                0,
                0,
                0,
            )
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        let keysyms = text_columns(&map)?;
        let plan = text_plan(text, map.first_key_sym, 2, &keysyms)?;
        let shift = if plan.iter().any(|(_, shifted)| *shifted) {
            let shift = self.hid_keycode(0xe1)?;
            let modifiers = self
                .connection
                .get_modifier_mapping()
                .map_err(BackendError::os)?
                .reply()
                .map_err(BackendError::os)?;
            if !modifiers.keycodes[..usize::from(modifiers.keycodes_per_modifier())]
                .contains(&shift)
            {
                return Err(BackendError::new(
                    ErrorKind::Unsupported,
                    "physical left Shift does not map to the Shift modifier",
                ));
            }
            Some(shift)
        } else {
            None
        };
        // No injection begins until every character has a representation.
        for (key, shifted) in plan {
            self.ensure_text_state()?;
            if shifted {
                self.key_event(shift.expect("shift preflighted"), true, guard)?;
            }
            self.key_event(key, true, guard)?;
            self.key_event(key, false, guard)?;
            if shifted {
                self.key_event(shift.expect("shift preflighted"), false, guard)?;
            }
        }
        Ok(())
    }

    fn ensure_text_state(&self) -> Result<()> {
        let state = self
            .connection
            .xkb_get_state(u16::from(xkb::ID::USE_CORE_KBD))
            .map_err(BackendError::os)?
            .reply()
            .map_err(BackendError::os)?;
        if u16::from(state.mods) != 0
            || u16::from(state.latched_mods) != 0
            || u16::from(state.locked_mods) != 0
            || u8::from(state.group) != 0
            || state.base_group != 0
            || state.latched_group != 0
        {
            return Err(BackendError::new(
                ErrorKind::Unsupported,
                "text requires neutral modifiers and keyboard group zero",
            ));
        }
        Ok(())
    }
}

// Compute the actual group-zero symbols for neutral and Shift-only state from
// XKB key types. Do not assume that Shift always selects symbol index one.
fn text_columns(map: &xkb::GetMapReply) -> Result<Vec<u32>> {
    let types =
        map.map.types_rtrn.as_ref().ok_or_else(|| {
            BackendError::new(ErrorKind::Unsupported, "XKB key types unavailable")
        })?;
    let keys = map
        .map
        .syms_rtrn
        .as_ref()
        .ok_or_else(|| BackendError::new(ErrorKind::Unsupported, "XKB symbols unavailable"))?;
    Ok(resolve_columns(map.first_type, types, keys))
}

fn resolve_columns(first_type: u8, types: &[xkb::KeyType], keys: &[xkb::KeySymMap]) -> Vec<u32> {
    let mut columns = Vec::with_capacity(keys.len() * 2);
    for key in keys {
        let kind = key.kt_index[0]
            .checked_sub(first_type)
            .and_then(|i| types.get(i as usize));
        for modifiers in [0u16, 1] {
            let symbol = kind
                .and_then(|kind| {
                    if key.group_info & 0xf == 0
                        || key.width == 0
                        || u16::from(kind.mods_vmods) != 0
                    {
                        return None;
                    }
                    let active = modifiers & u16::from(kind.mods_mask);
                    let level = kind
                        .map
                        .iter()
                        .find(|entry| entry.active && u16::from(entry.mods_mask) == active)
                        .map_or(0, |entry| entry.level);
                    if level >= kind.num_levels || level >= key.width {
                        return None;
                    }
                    key.syms.get(level as usize).copied()
                })
                .unwrap_or(0);
            columns.push(symbol);
        }
    }
    columns
}

// USB HID usage-page 7 physical positions to standard XKB key names.
// Names are resolved against the live server; no assumed Linux keycode offset.
pub(crate) fn hid_name(usage: u16) -> Option<[u8; 4]> {
    let name = match usage {
        0x04 => "AC01",
        0x05 => "AB05",
        0x06 => "AB03",
        0x07 => "AC03",
        0x08 => "AD03",
        0x09 => "AC04",
        0x0a => "AC05",
        0x0b => "AC06",
        0x0c => "AD08",
        0x0d => "AC07",
        0x0e => "AC08",
        0x0f => "AC09",
        0x10 => "AB07",
        0x11 => "AB06",
        0x12 => "AD09",
        0x13 => "AD10",
        0x14 => "AD01",
        0x15 => "AD04",
        0x16 => "AC02",
        0x17 => "AD05",
        0x18 => "AD07",
        0x19 => "AB04",
        0x1a => "AD02",
        0x1b => "AB02",
        0x1c => "AD06",
        0x1d => "AB01",
        0x1e => "AE01",
        0x1f => "AE02",
        0x20 => "AE03",
        0x21 => "AE04",
        0x22 => "AE05",
        0x23 => "AE06",
        0x24 => "AE07",
        0x25 => "AE08",
        0x26 => "AE09",
        0x27 => "AE10",
        0x28 => "RTRN",
        0x29 => "ESC\0",
        0x2a => "BKSP",
        0x2b => "TAB\0",
        0x2c => "SPCE",
        0x2d => "AE11",
        0x2e => "AE12",
        0x2f => "AD11",
        0x30 => "AD12",
        0x31 => "BKSL",
        0x33 => "AC10",
        0x34 => "AC11",
        0x35 => "TLDE",
        0x36 => "AB08",
        0x37 => "AB09",
        0x38 => "AB10",
        0x39 => "CAPS",
        0x3a => "FK01",
        0x3b => "FK02",
        0x3c => "FK03",
        0x3d => "FK04",
        0x3e => "FK05",
        0x3f => "FK06",
        0x40 => "FK07",
        0x41 => "FK08",
        0x42 => "FK09",
        0x43 => "FK10",
        0x44 => "FK11",
        0x45 => "FK12",
        0x49 => "INS\0",
        0x4a => "HOME",
        0x4b => "PGUP",
        0x4c => "DELE",
        0x4d => "END\0",
        0x4e => "PGDN",
        0x4f => "RGHT",
        0x50 => "LEFT",
        0x51 => "DOWN",
        0x52 => "UP\0\0",
        0x64 => "LSGT",
        0xe0 => "LCTL",
        0xe1 => "LFSH",
        0xe2 => "LALT",
        0xe3 => "LWIN",
        0xe4 => "RCTL",
        0xe5 => "RTSH",
        0xe6 => "RALT",
        0xe7 => "RWIN",
        _ => return None,
    };
    Some(name.as_bytes().try_into().expect("four-byte XKB key name"))
}

pub(crate) fn text_plan(
    text: &str,
    min_keycode: u8,
    per_key: u8,
    keysyms: &[u32],
) -> Result<Vec<(u8, bool)>> {
    if per_key < 2 || !keysyms.len().is_multiple_of(usize::from(per_key)) {
        return Err(BackendError::new(
            ErrorKind::Unsupported,
            "keyboard map lacks explicit unshifted/shifted levels",
        ));
    }
    text.chars()
        .map(|c| {
            let keysym = match c {
                '\n' | '\r' => 0xff0d,
                '\t' => 0xff09,
                c if c.is_control() => {
                    return Err(BackendError::new(
                        ErrorKind::Unsupported,
                        "unsupported text control character",
                    ))
                }
                c if c as u32 <= 255 => c as u32,
                c => 0x01000000 | c as u32,
            };
            for (index, levels) in keysyms.chunks_exact(per_key as usize).enumerate() {
                if let Some(level) = levels[..2].iter().position(|s| *s == keysym) {
                    let key = u8::try_from(usize::from(min_keycode) + index).map_err(|_| {
                        BackendError::new(ErrorKind::Unsupported, "invalid keyboard keycode")
                    })?;
                    return Ok((key, level == 1));
                }
            }
            Err(BackendError::new(
                ErrorKind::Unsupported,
                "text contains characters unavailable in keyboard group zero",
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xkb_type_controls_shift_level_and_virtual_types_are_excluded() {
        let mut kind = xkb::KeyType {
            mods_mask: 1u8.into(),
            num_levels: 3,
            map: vec![xkb::KTMapEntry {
                active: true,
                mods_mask: 1u8.into(),
                level: 2,
                ..Default::default()
            }],
            ..Default::default()
        };
        let key = xkb::KeySymMap {
            kt_index: [0; 4],
            group_info: 1,
            width: 3,
            syms: vec![97, 98, 65],
        };
        assert_eq!(
            resolve_columns(0, &[kind.clone()], std::slice::from_ref(&key)),
            [97, 65]
        );
        kind.mods_vmods = 1u16.into();
        assert_eq!(resolve_columns(0, &[kind], &[key]), [0, 0]);
    }
    #[test]
    fn server_mapping_controls_text_keycodes() {
        assert_eq!(
            text_plan(
                "aA!",
                40,
                2,
                &[b'a' as u32, b'A' as u32, b'1' as u32, b'!' as u32]
            )
            .unwrap(),
            [(40, false), (40, true), (41, true)]
        );
    }
    #[test]
    fn unsupported_suffix_rejects_entire_plan() {
        assert_eq!(
            text_plan("a🔑", 40, 2, &[97, 65]).unwrap_err().kind,
            ErrorKind::Unsupported
        );
    }
    #[test]
    fn malformed_mapping_and_control_text_fail() {
        assert!(text_plan("a", 8, 0, &[]).is_err());
        assert!(text_plan("a", 8, 2, &[97]).is_err());
        assert!(text_plan("\0", 8, 2, &[0, 0]).is_err());
    }
    #[test]
    fn hid_positions_are_not_character_lookup() {
        assert_eq!(hid_name(4), Some(*b"AC01"));
        assert_eq!(hid_name(0xe1), Some(*b"LFSH"));
        assert_eq!(hid_name(0xffff), None);
    }
}
