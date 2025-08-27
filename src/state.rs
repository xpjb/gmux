use std::os::raw::{c_int, c_uint};
use std::ffi::CString;
use std::os::raw::c_uchar;
use std::sync::mpsc::{Sender, Receiver};
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::env;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use x11::{keysym, xlib};

use crate::*;

// Global state
pub struct Gmux {
    pub status_text: String,
    pub screen: c_int,
    pub screen_width: c_int,
    pub screen_height: c_int,
    pub bar_height: c_int,
    pub _bar_line_width: c_int,
    pub lr_padding: c_int,
    pub numlock_mask: c_uint,
    pub running: c_int,
    pub cursor: [CursorId; crate::CursorType::Last as usize],
    pub xwrapper: XWrapper,
    pub mons: Vec<Monitor>,
    pub selected_monitor: usize,
    pub root: Window,
    pub wm_check_window: Window,
    pub _xerror: bool,
    pub tags: [&'static str; 9],
    pub all_commands: Vec<String>,
    pub bar_state: BarState,
    pub command_sender: Sender<GmuxError>,
    pub command_receiver: Receiver<GmuxError>,
}

impl Gmux {
    pub unsafe fn window_to_monitor(&self, w: xlib::Window) -> usize {
        let wrapped_w = Window(w);
        if wrapped_w == self.root {
            if let Some((x, y)) = self.xwrapper.query_pointer_position() {
                return self.rect_to_monitor(x, y, 1, 1);
            }
        }
        for (i, m) in self.mons.iter().enumerate() {
            if m.bar_window == wrapped_w {
                return i;
            }
        }
        if let Some((mon_idx, _)) = unsafe { self.window_to_client_idx(w) } {
            return mon_idx;
        }
        self.selected_monitor
    }

    
    pub fn rect_to_monitor(&self, x: i32, y: i32, w: i32, h: i32) -> usize {
        let mut r = self.selected_monitor;
        let mut area = 0;
        for (i, m) in self.mons.iter().enumerate() {
            let a = m.intersect_area(x, y, w, h) as i32;
            if a > area {
                area = a;
                r = i;
            }
        }
        r
    }

    pub fn arrange(&mut self, mon_idx: Option<usize>) {
        if let Some(idx) = mon_idx {
            let stack = self.mons[idx].stack.clone();
            self.show_hide(idx, &stack);
            self.arrange_monitor(idx);
    
            // ======================== NEW LOGIC START ========================
            // After arranging, determine the correct client to focus.
            let new_sel_idx = {
                let mon = &self.mons[idx];
                // Check if the current selection is still visible.
                let current_sel_is_visible = mon.sel
                    .and_then(|s_idx| mon.clients.get(s_idx)) // Safely get the client
                    .map(|s_client| s_client.is_visible_on(mon))
                    .unwrap_or(false);
    
                if current_sel_is_visible {
                    // If it's still visible, keep it selected.
                    mon.sel
                } else {
                    // Otherwise, find the first visible client and select it.
                    // If no client is visible, this will be `None`.
                    mon.clients.iter().enumerate()
                        .find(|(_, c)| c.is_visible_on(mon))
                        .map(|(i, _)| i)
                }
            };
    
            // Update the focus with the new selection (or None).
            // This function will also call `draw_bars` to update the visuals.
            self.focus(idx, new_sel_idx);
            // ========================= NEW LOGIC END =========================
    
            self.restack(idx);
    
        } else {
            // This part arranges all monitors. You might need to apply
            // similar focus logic here if you want multi-monitor
            // arrange operations to update focus correctly.
            for i in 0..self.mons.len() {
                let stack = self.mons[i].stack.clone();
                self.show_hide(i, &stack);
                self.arrange_monitor(i);
            }
            for i in 0..self.mons.len() {
                self.restack(i);
            }
        }
    }

    
    pub fn arrange_monitor(&mut self, mon_idx: usize) {
        if let Some(mon) = self.mons.get(mon_idx) {
            let layout = mon.lt[mon.selected_lt as usize];
            if let Some(arrange_fn) = layout.arrange {
                arrange_fn(self, mon_idx);
            }
        }
    }

    
    pub fn restack(&mut self, mon_idx: usize) {
        self.draw_bar(mon_idx);
        if let Some(m) = self.mons.get(mon_idx) {
            if m.sel.is_none() {
                return;
            }
            let sel_client = &m.clients[m.sel.unwrap()];
            if sel_client.is_floating || m.lt[m.selected_lt as usize].arrange.is_none() {
                self.xwrapper.raise_window(sel_client.win);
            }

            let mut windows_to_stack: Vec<Window> = Vec::new();
            windows_to_stack.push(m.bar_window);

            for &c_idx in &m.stack {
                let c = &m.clients[c_idx];
                if !c.is_floating && c.is_visible_on(m) {
                    windows_to_stack.push(c.win);
                }
            }
            
            self.xwrapper.stack_windows(&windows_to_stack);
        }
    }

    pub fn focus(&mut self, mon_idx: usize, c_idx: Option<usize>) {
        let selmon_idx = self.selected_monitor;
    
        if let Some(old_sel_idx) = self.mons[selmon_idx].sel {
            if c_idx.is_none() || mon_idx != selmon_idx || old_sel_idx != c_idx.unwrap() {
                self.unfocus(selmon_idx, old_sel_idx, false);
            }
        }
    
        if let Some(idx) = c_idx {
            let new_mon_idx = self.mons[mon_idx].clients[idx].monitor_idx;
            if new_mon_idx != selmon_idx {
                self.selected_monitor = new_mon_idx;
            }
            let c_win = self.mons[new_mon_idx].clients[idx].win;
            let c_isurgent = self.mons[new_mon_idx].clients[idx].is_urgent;
            if c_isurgent {
                // seturgent(c, 0);
            }
            // detachstack(c);
            // attachstack(c);
            self.grab_buttons(new_mon_idx, idx, true);
            let keys = crate::config::grab_keys();
            let key_specs: Vec<KeySpecification> = keys
                .iter()
                .map(|k| KeySpecification {
                    mask: k.mask,
                    keysym: k.keysym,
                })
                .collect();
            self
                .xwrapper
                .grab_keys(c_win, self.numlock_mask, &key_specs);
            self.xwrapper.set_window_border_color(c_win, Colour::WindowActive);
            self
                .xwrapper
                .set_input_focus(c_win, xlib::RevertToPointerRoot);
        } else {
            self
                .xwrapper
                .set_input_focus(self.root, xlib::RevertToPointerRoot);
            // XDeleteProperty(dpy, root, netatom[NetActiveWindow]);
        }
        self.mons[self.selected_monitor].sel = c_idx;
        self.draw_bars();
    }
    
    pub fn unfocus(&mut self, mon_idx: usize, c_idx: usize, setfocus: bool) {
        if c_idx >= self.mons[mon_idx].clients.len() {
            return;
        }
        let c_win = self.mons[mon_idx].clients[c_idx].win;
        self.grab_buttons(mon_idx, c_idx, false);
        self.xwrapper.ungrab_keys(c_win);
        self.xwrapper.set_window_border_color(c_win, Colour::WindowInactive);
        if setfocus {
            self
                .xwrapper
                .set_input_focus(self.root, xlib::RevertToPointerRoot);
            // XDeleteProperty(dpy, root, netatom[NetActiveWindow]);
        }
    }

    pub unsafe fn resize(&mut self, mon_idx: usize, client_idx: usize, x: i32, y: i32, w: i32, h: i32, _interact: bool) {
        let client = &mut self.mons[mon_idx].clients[client_idx];
        client.oldx = client.x;
        client.x = x;
        client.oldy = client.y;
        client.y = y;
        client.oldw = client.w;
        client.w = w;
        client.oldh = client.h;
        client.h = h;
        self.xwrapper.configure_window(
            client.win,
            client.x,
            client.y,
            client.w,
            client.h,
            crate::config::BORDER_PX,
        );
    }

    fn grab_buttons(&mut self, _mon_idx: usize, _c_idx: usize, _focused: bool) {
        // For now, this is a stub
        // idk if its needed, i dont intend to support monocle or floating or resizing etc, unless it would be great for resizing camera or something
    }

    pub fn handle_launcher_keypress(&mut self, kev: &xlib::XKeyEvent) {
        let mut new_state = None;
        let mut command_to_run = None;

        if let BarState::Launcher {
            input,
            candidate_indices,
            selected_idx,
            ..
        } = &mut self.bar_state
        {
            let mut dirty = false;
            let keysym = unsafe { xlib::XLookupKeysym(kev as *const _ as *mut _, 0) } as u32;

            match keysym {
                keysym::XK_Escape => {
                    new_state = Some(BarState::Normal);
                }
                keysym::XK_Return => {
                    if !candidate_indices.is_empty() {
                        let command_idx = candidate_indices[*selected_idx];
                        command_to_run = Some(self.all_commands[command_idx].clone());
                    }
                    new_state = Some(BarState::Normal);
                }
                keysym::XK_Left => {
                    if *selected_idx > 0 {
                        *selected_idx -= 1;
                    }
                }
                keysym::XK_Right => {
                    if !candidate_indices.is_empty() && *selected_idx < candidate_indices.len() - 1 {
                        *selected_idx += 1;
                    }
                }
                keysym::XK_BackSpace => {
                    input.pop();
                    dirty = true;
                }
                _ => {
                    if let Some(mut s) = self.xwrapper.keysym_to_string(keysym) {
                        if s.len() == 1
                            && s.chars().next().unwrap().is_ascii()
                            && !s.chars().next().unwrap().is_ascii_control()
                        {
                            s.make_ascii_lowercase();
                            input.push_str(&s);
                            dirty = true;
                        }
                    }
                }
            }

            if dirty {
                let matcher = SkimMatcherV2::default();
                let mut new_candidates: Vec<(i64, usize)> = self
                    .all_commands
                    .iter()
                    .enumerate()
                    .filter_map(|(i, cmd)| matcher.fuzzy_match(cmd, input).map(|score| (score, i)))
                    .collect();

                new_candidates.sort_by(|a, b| b.0.cmp(&a.0));
                *candidate_indices = new_candidates.into_iter().map(|(_, i)| i).collect();
                *selected_idx = 0;
            }
        }

        if let Some(cmd) = command_to_run {
            self.spawn(&cmd);
        }

        if let Some(state) = new_state {
            self.bar_state = state;
        }

        self.draw_bars();
    }

    pub fn enter_launcher_mode(&mut self) {
        let initial_candidates = (0..self.all_commands.len()).collect();
        self.bar_state = BarState::Launcher {
            prompt: "> ".to_string(),
            input: String::new(),
            candidate_indices: initial_candidates,
            selected_idx: 0,
        };
        self.draw_bars();
    }

    fn get_commands() -> Vec<String> {
        let mut commands = HashSet::new();
        if let Ok(path_var) = env::var("PATH") {
            for path in path_var.split(':') {
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries.flatten() {
                        if let Ok(metadata) = entry.metadata() {
                            if metadata.is_file() && (metadata.permissions().mode() & 0o111 != 0) {
                                if let Some(command) = entry.file_name().to_str() {
                                    commands.insert(command.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut sorted_commands: Vec<_> = commands.into_iter().collect();
        sorted_commands.sort();
        sorted_commands
    }

    pub fn new(command_sender: Sender<GmuxError>, command_receiver: Receiver<GmuxError>) -> Result<Gmux, String> {
        let mut xwrapper = XWrapper::connect().expect("Failed to open display");
        unsafe {
            let locale = CString::new("").unwrap();
            if libc::setlocale(libc::LC_CTYPE, locale.as_ptr()).is_null()
                || xlib::XSupportsLocale() == 0
            {
                eprintln!("warning: no locale support");
            }

            if let Err(e) = xwrapper.check_for_other_wm() {
                return Err(e);
            }
            xwrapper.set_default_error_handler();
        }

        let mut state = Gmux {
            status_text: String::new(),
            screen: 0,
            screen_width: 0,
            screen_height: 0,
            bar_height: 0,
            _bar_line_width: 0,
            lr_padding: 0,
            numlock_mask: 0,
            running: 1,
            cursor: [CursorId(0); CursorType::Last as usize],
            mons: Vec::new(),
            selected_monitor: 0,
            root: Window(0),
            wm_check_window: Window(0),
            _xerror: false,
            tags: ["1", "2", "3", "4", "5", "6", "7", "8", "9"],
            bar_state: BarState::Normal,
            all_commands: Gmux::get_commands(),
            xwrapper,
            command_sender,
            command_receiver,
        };

        state.setup();
        Ok(state)
    }

    fn setup(&mut self) {
        unsafe {
            self.screen = self.xwrapper.default_screen();
            self.screen_width = self.xwrapper.display_width(self.screen);
            self.screen_height = self.xwrapper.display_height(self.screen);
            self.root = self.xwrapper.root_window(self.screen);
            
            let fonts = &["monospace:size=12"]; // TODO: configurable
            if !self.xwrapper.fontset_create(fonts) {
                panic!("no fonts could be loaded.");
            }

            // derive bar height and lr_padding from font height like dwm
            let h = self.xwrapper.get_font_height() as i32;
            if h > 0 {
                self.bar_height = h + 2;
                self.lr_padding = h + 2;
            }

            // initialise status text sample
            self.status_text = "gmux".to_string();

            self.draw_bars();

            // Create a monitor
            let mut mon = Monitor::default();
            mon.tagset = [1, 1];
            mon.mfact = 0.55;
            mon.nmaster = 1;
            mon.show_bar = true;
            mon.top_bar = true;
            // Calculate window area accounting for the bar height
            if mon.show_bar {
                mon.by = if mon.top_bar { 0 } else { self.screen_height - self.bar_height };
                mon.wy = if mon.top_bar { self.bar_height } else { 0 };
                mon.wh = self.screen_height - self.bar_height;
            } else {
                mon.by = -self.bar_height;
                mon.wy = 0;
                mon.wh = self.screen_height;
            }
            mon.lt[0] = &LAYOUTS[0];
            mon.lt[1] = &LAYOUTS[1];
            mon.lt_symbol = LAYOUTS[0].symbol.to_string();
            mon.wx = 0;
            mon.ww = self.screen_width;
            let mut wa: xlib::XSetWindowAttributes = std::mem::zeroed();
            wa.override_redirect = 1;
            wa.background_pixmap = xlib::ParentRelative as u64;
            wa.event_mask = xlib::ButtonPressMask | xlib::ExposureMask;
            let valuemask = xlib::CWOverrideRedirect | xlib::CWBackPixmap | xlib::CWEventMask;
            let barwin = self.xwrapper.create_window(
                self.root,
                mon.wx,
                mon.by,
                mon.ww as u32,
                self.bar_height as u32,
                0,
                self.xwrapper.default_depth(self.screen),
                xlib::InputOutput as u32,
                self.xwrapper.default_visual(self.screen),
                valuemask as u64,
                &mut wa,
            );
            mon.bar_window = barwin;
            self.xwrapper.map_raised(mon.bar_window);
            self.mons.push(mon);
            self.selected_monitor = self.mons.len() - 1;

            self.cursor[CursorType::Normal as usize] = self.xwrapper.create_font_cursor_as_id(68);
            self.cursor[CursorType::Resize as usize] = self.xwrapper.create_font_cursor_as_id(120);
            self.cursor[CursorType::Move as usize] = self.xwrapper.create_font_cursor_as_id(52);
            
            self.wm_check_window = self.xwrapper.create_simple_window(self.root, 0, 0, 1, 1, 0, 0, 0);
            let wmcheckwin_val = self.wm_check_window.0;
            self.xwrapper.change_property(self.wm_check_window, self.xwrapper.atoms.get(Atom::Net(Net::WMCheck)), xlib::XA_WINDOW, 32,
                xlib::PropModeReplace, &wmcheckwin_val as *const u64 as *const c_uchar, 1);

            let dwm_name = CString::new("dwm").unwrap();
            self.xwrapper.change_property(self.wm_check_window, self.xwrapper.atoms.get(Atom::Net(Net::WMName)), xlib::XA_STRING, 8,
                xlib::PropModeReplace, dwm_name.as_ptr() as *const c_uchar, 3);
            self.xwrapper.change_property(self.root, self.xwrapper.atoms.get(Atom::Net(Net::WMCheck)), xlib::XA_WINDOW, 32,
                xlib::PropModeReplace, &wmcheckwin_val as *const u64 as *const c_uchar, 1);

            self.xwrapper.change_property(self.root, self.xwrapper.atoms.get(Atom::Net(Net::Supported)), xlib::XA_ATOM, 32,
                xlib::PropModeReplace, self.xwrapper.atoms.net_atom_ptr() as *const c_uchar, Net::Last as i32);
            self.xwrapper.delete_property(self.root, self.xwrapper.atoms.get(Atom::Net(Net::ClientList)));

            let mut wa: xlib::XSetWindowAttributes = std::mem::zeroed();
            wa.cursor = self.cursor[CursorType::Normal as usize].0;
            wa.event_mask = (xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask
                | xlib::ButtonPressMask | xlib::PointerMotionMask | xlib::EnterWindowMask
                | xlib::LeaveWindowMask | xlib::StructureNotifyMask | xlib::PropertyChangeMask
                | xlib::KeyPressMask) as i64;
            self.xwrapper.change_window_attributes(self.root, (xlib::CWEventMask | xlib::CWCursor) as u64, &mut wa);
            self.xwrapper.select_input(self.root, wa.event_mask);

            // Update NumLockMask and grab global keys
            self.numlock_mask = self.xwrapper.get_numlock_mask();
            let key_actions: Vec<KeyBinding> = config::grab_keys();
            let key_specs: Vec<KeySpecification> = key_actions
                .iter()
                .map(|k| KeySpecification {
                    mask: k.mask,
                    keysym: k.keysym,
                })
                .collect();
            self
                .xwrapper
                .grab_keys(self.root, self.numlock_mask, &key_specs);

            self.focus(0, None);
        }
    }
}

impl Drop for Gmux {
    fn drop(&mut self) {
        for i in 0..self.mons.len() {
            while !self.mons[i].stack.is_empty() {
                let c_idx = self.mons[i].stack.pop().unwrap();
                self.unmanage(i, c_idx, false);
            }
        }
        self.xwrapper.ungrab_key(self.root);
    }
}