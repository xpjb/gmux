use crate::command::Command;
use crate::Action;
use x11::{keysym, xlib};

pub const BORDER_PX: i32 = 2;
pub const BAR_H_PADDING: u32 = 15; // Horizontal padding for bar elements

// Statically-known strings
pub const TAGS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

pub struct KeyBinding {
    pub mask: u32,
    pub keysym: u32,
    pub action: Action,
}

pub fn grab_keys() -> Vec<KeyBinding> {
    let mut keys: Vec<KeyBinding> = vec![];
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask,
        keysym: keysym::XK_p,
        action: Action::Spawn(&Command::Dmenu),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask | xlib::ShiftMask,
        keysym: keysym::XK_Return,
        action: Action::Spawn(&Command::Terminal),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask,
        keysym: keysym::XK_b,
        action: Action::ToggleBar,
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask,
        keysym: keysym::XK_j,
        action: Action::FocusStack(1),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask,
        keysym: keysym::XK_k,
        action: Action::FocusStack(-1),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask,
        keysym: keysym::XK_i,
        action: Action::IncNMaster(1),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask,
        keysym: keysym::XK_d,
        action: Action::IncNMaster(-1),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask,
        keysym: keysym::XK_h,
        action: Action::SetMFact(-0.05),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask,
        keysym: keysym::XK_l,
        action: Action::SetMFact(0.05),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask,
        keysym: keysym::XK_Return,
        action: Action::Zoom,
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask,
        keysym: keysym::XK_Tab,
        action: Action::ViewPrevTag,
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask | xlib::ShiftMask,
        keysym: keysym::XK_c,
        action: Action::KillClient,
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask,
        keysym: keysym::XK_space,
        action: Action::SetLayout(&crate::layouts::LAYOUTS[0]),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask | xlib::ShiftMask,
        keysym: keysym::XK_space,
        action: Action::ToggleFloating,
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask,
        keysym: keysym::XK_0,
        action: Action::ViewTag(!0, None),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask | xlib::ShiftMask,
        keysym: keysym::XK_0,
        action: Action::Tag(!0),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask,
        keysym: keysym::XK_comma,
        action: Action::FocusMon(-1),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask,
        keysym: keysym::XK_period,
        action: Action::FocusMon(1),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask | xlib::ShiftMask,
        keysym: keysym::XK_comma,
        action: Action::TagMon(-1),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask | xlib::ShiftMask,
        keysym: keysym::XK_period,
        action: Action::TagMon(1),
    });
    keys.push(KeyBinding {
        mask: xlib::Mod1Mask | xlib::ShiftMask,
        keysym: keysym::XK_q,
        action: Action::Quit,
    });
    keys.push(KeyBinding {
        mask: 0,
        keysym: keysym::XK_Print,
        action: Action::Spawn(&Command::Screenshot),
    });

    const TAG_KEYS: &[(u32, u32)] = &[
        (keysym::XK_1, 0),
        (keysym::XK_2, 1),
        (keysym::XK_3, 2),
        (keysym::XK_4, 3),
        (keysym::XK_5, 4),
        (keysym::XK_6, 5),
        (keysym::XK_7, 6),
        (keysym::XK_8, 7),
        (keysym::XK_9, 8),
    ];

    for &(keysym, tag_idx) in TAG_KEYS {
        keys.push(KeyBinding {
            mask: xlib::Mod1Mask,
            keysym,
            action: Action::ViewTag(1 << tag_idx, None),
        });
        keys.push(KeyBinding {
            mask: xlib::Mod1Mask | xlib::ControlMask,
            keysym,
            action: Action::ToggleView(1 << tag_idx),
        });
        keys.push(KeyBinding {
            mask: xlib::Mod1Mask | xlib::ShiftMask,
            keysym,
            action: Action::Tag(1 << tag_idx),
        });
        keys.push(KeyBinding {
            mask: xlib::Mod1Mask | xlib::ControlMask | xlib::ShiftMask,
            keysym,
            action: Action::ToggleTag(1 << tag_idx),
        });
    }

    keys
}
