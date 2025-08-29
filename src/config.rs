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
    const MOD: u32 = xlib::Mod1Mask;
    const SHIFT_MASK: u32 = xlib::ShiftMask;

    let mut keys: Vec<KeyBinding> = vec![];
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_p,
        action: Action::EnterLauncherMode,
    });
    keys.push(KeyBinding {
        mask: MOD | SHIFT_MASK,
        keysym: keysym::XK_Return,
        action: Action::Spawn("alacritty".to_string()),
    });
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_b,
        action: Action::ToggleBar,
    });
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_j,
        action: Action::FocusStack(1),
    });
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_k,
        action: Action::FocusStack(-1),
    });
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_i,
        action: Action::IncNMaster(1),
    });
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_d,
        action: Action::IncNMaster(-1),
    });
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_h,
        action: Action::SetMFact(-0.05),
    });
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_l,
        action: Action::SetMFact(0.05),
    });
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_Return,
        action: Action::Zoom,
    });
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_Tab,
        action: Action::ViewPrevTag,
    });
    keys.push(KeyBinding {
        mask: MOD | SHIFT_MASK,
        keysym: keysym::XK_c,
        action: Action::KillClient,
    });
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_space,
        action: Action::SetLayout(&crate::layouts::LAYOUTS[0]),
    });
    keys.push(KeyBinding {
        mask: MOD | SHIFT_MASK,
        keysym: keysym::XK_space,
        action: Action::ToggleFloating,
    });
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_0,
        action: Action::ViewTag(!0, None),
    });
    keys.push(KeyBinding {
        mask: MOD | SHIFT_MASK,
        keysym: keysym::XK_0,
        action: Action::Tag(!0),
    });
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_comma,
        action: Action::FocusMon(-1),
    });
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_period,
        action: Action::FocusMon(1),
    });
    keys.push(KeyBinding {
        mask: MOD | SHIFT_MASK,
        keysym: keysym::XK_comma,
        action: Action::TagMon(-1),
    });
    keys.push(KeyBinding {
        mask: MOD | SHIFT_MASK,
        keysym: keysym::XK_period,
        action: Action::TagMon(1),
    });
    keys.push(KeyBinding {
        mask: MOD | SHIFT_MASK,
        keysym: keysym::XK_q,
        action: Action::Quit,
    });

    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_w, // 'w' for warning
        // This command succeeds (exit code 0) but prints to stderr.
        // CHECK: gmux.log should contain "This is a test warning", but the bar should NOT turn red.
        action: Action::Spawn("echo 'This is a test warning' >&2".to_string()),
    });

    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_e, // 'e' for error
        // This command fails (non-zero exit code).
        // CHECK: gmux.log should contain the error, AND the bar should turn red.
        action: Action::Spawn("ls /nonexistent/directory".to_string()),
    });
    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_Print,
        action: Action::Spawn(
            "scrot -u \"$HOME/Pictures/screenshots/%Y-%m-%d-%T-screenshot.png\" -e 'xclip -selection clipboard -t image/png < $f'"
                .to_string(),
        ),
    });
    keys.push(KeyBinding {
        mask: 0,
        keysym: keysym::XK_Print,
        action: Action::Spawn(
            "scrot \"$HOME/Pictures/screenshots/%Y-%m-%d-%T-screenshot.png\" -e 'xclip -selection clipboard -t image/png < $f'"
                .to_string(),
        ),
    });

    keys.push(KeyBinding {
        mask: MOD,
        keysym: keysym::XK_s,
        action: Action::Spawn(
            "scrot -s \"$HOME/Pictures/screenshots/%Y-%m-%d-%T-screenshot.png\" -e 'xclip -selection clipboard -t image/png < $f'"
                .to_string(),
        ),
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
            mask: MOD,
            keysym,
            action: Action::ViewTag(1 << tag_idx, None),
        });
        keys.push(KeyBinding {
            mask: MOD | xlib::ControlMask,
            keysym,
            action: Action::ToggleView(1 << tag_idx),
        });
        keys.push(KeyBinding {
            mask: MOD | SHIFT_MASK,
            keysym,
            action: Action::Tag(1 << tag_idx),
        });
        keys.push(KeyBinding {
            mask: MOD | xlib::ControlMask | SHIFT_MASK,
            keysym,
            action: Action::ToggleTag(1 << tag_idx),
        });
    }

    keys
}
