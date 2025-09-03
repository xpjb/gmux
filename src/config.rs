use crate::Action;
use x11::{keysym, xlib};
use lazy_static::lazy_static;
use std::path::PathBuf;

pub const BORDER_PX: i32 = 6;
lazy_static! {
    /// The path to the application's data directory.
    pub static ref DATA_PATH: PathBuf = {
        dirs::data_dir()
            .map(|mut path| {
                path.push("gmux");
                path
            })
            .unwrap_or_else(|| PathBuf::from(".")) // Fallback to current directory
    };

    /// The path to the application's log file.
    pub static ref LOG_PATH: PathBuf = {
        let mut path = DATA_PATH.clone();
        path.push("gmux.log");
        path
    };
}

// Statically-known strings
pub const TAGS: [&str; 5] = ["1", "2", "3", "4", "5"];

#[derive(Debug, Clone)]
pub struct Rule {
    pub class: Option<String>,
    pub instance: Option<String>, 
    pub title: Option<String>,
    pub tags: u32,
    pub is_floating: bool,
    pub monitor: i32, // -1 for current monitor
}

pub fn rules() -> Vec<Rule> {
    vec![
        Rule {
            class: Some("firefox".to_string()),
            instance: None,
            title: None,
            tags: 1 << 4, // Tag 5
            is_floating: false,
            monitor: -1,
        },
        Rule {
            class: Some("discord".to_string()),
            instance: None,
            title: None,
            tags: 1 << 3, // Tag 4
            is_floating: false,
            monitor: -1,
        },
        Rule {
            class: Some("steam".to_string()),
            instance: None,
            title: None,
            tags: 1 << 2, // Tag 4
            is_floating: false,
            monitor: -1,
        },
    ]
}

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
        action: Action::SpawnDirect("alacritty".to_string(), vec![]),
    });
    keys.push(KeyBinding {
        mask: MOD | SHIFT_MASK,
        keysym: keysym::XK_l,
        action: Action::SpawnDirect(
            "alacritty".to_string(),
            vec![
                "-e".to_string(),
                "tail".to_string(),
                "-f".to_string(),
                LOG_PATH.to_str().expect("Log path is not valid UTF-8").to_string(),
            ],
        ),
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
        keysym: keysym::XK_Tab,
        action: Action::CycleTag(1),
    });
    keys.push(KeyBinding {
        mask: MOD | SHIFT_MASK,
        keysym: keysym::XK_Tab,
        action: Action::CycleTag(-1),
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
    
    // TEMPORARY: Test panic binding (Alt+Shift+P for panic test)
    keys.push(KeyBinding {
        mask: MOD | SHIFT_MASK,
        keysym: keysym::XK_p,
        action: Action::TestPanic,
    });

    const TAG_KEYS: &[(u32, u32)] = &[
        (keysym::XK_1, 0),
        (keysym::XK_2, 1),
        (keysym::XK_3, 2),
        (keysym::XK_4, 3),
        (keysym::XK_5, 4),
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
