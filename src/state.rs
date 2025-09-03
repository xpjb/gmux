use std::os::raw::{c_int, c_uint};
use std::ffi::CString;
use std::os::raw::c_uchar;
use std::sync::mpsc::{Sender, Receiver};
use x11::xlib;
use std::collections::HashMap;

use crate::*;

// Global state
pub struct Gmux {
    pub status_text: String,
    pub screen: c_int,
    pub screen_width: c_int,
    pub screen_height: c_int,
    pub bar_height: c_int,
    pub _bar_line_width: c_int,
    pub lr_padding: u32,
    pub numlock_mask: c_uint,
    pub running: c_int,
    pub cursor: [CursorId; crate::CursorType::Last as usize],
    pub xwrapper: XWrapper,
    pub mons: Vec<Monitor>,
    pub selected_monitor: usize,
    pub root: Window,
    pub wm_check_window: Window,
    pub _xerror: bool,
    pub tags: [&'static str; 5],
    pub all_commands: Vec<String>,
    pub bar_state: BarState,
    pub command_sender: Sender<GmuxError>,
    pub command_receiver: Receiver<GmuxError>,
    pub clients: HashMap<ClientHandle, Client>,
}

impl Gmux {
    pub fn get_text_width(&self, text: &str) -> u32 {
        self.xwrapper.text_width(text) + self.lr_padding as u32
    }

    /// Clips text to fit within the given width, adding "..." if truncated
    pub fn clip_text_to_width(&self, text: &str, max_width: i32) -> String {
        if max_width <= 0 {
            return String::new();
        }
        
        let max_width_u32 = max_width as u32;
        
        // If the text already fits, return it as-is
        if self.get_text_width(text) <= max_width_u32 {
            return text.to_string();
        }
        
        // Calculate width of ellipsis
        let ellipsis = "...";
        let ellipsis_width = self.get_text_width(ellipsis);
        
        // If even ellipsis doesn't fit, return empty string
        if ellipsis_width > max_width_u32 {
            return String::new();
        }
        
        // Binary search to find the maximum length that fits with ellipsis
        let available_width = max_width_u32 - ellipsis_width;
        let mut left = 0;
        let mut right = text.len();
        let mut best_len = 0;
        
        while left <= right {
            let mid = (left + right) / 2;
            let truncated = &text[..mid];
            let truncated_width = self.get_text_width(truncated);
            
            if truncated_width <= available_width {
                best_len = mid;
                if mid == text.len() {
                    break;
                }
                left = mid + 1;
            } else {
                if mid == 0 {
                    break;
                }
                right = mid - 1;
            }
        }
        
        // Make sure we don't break on UTF-8 character boundaries
        while best_len > 0 && !text.is_char_boundary(best_len) {
            best_len -= 1;
        }
        
        if best_len == 0 {
            ellipsis.to_string()
        } else {
            format!("{}{}", &text[..best_len], ellipsis)
        }
    }
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
        if let Some(handle) = self.window_to_client_handle(w) {
            if let Some(client) = self.clients.get(&handle) {
                return client.monitor_idx;
            }
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
            self.show_hide(idx, &self.mons[idx].stack.clone());
            self.arrange_monitor(idx);

            let new_sel = {
                let mon = &self.mons[idx];
                let visible_clients = mon.stack.iter()
                    .filter_map(|h| self.clients.get(h))
                    .find(|c| c.is_visible_on(mon));

                if mon.sel.is_some() && visible_clients.is_some() && visible_clients.unwrap().handle() == mon.sel.unwrap() {
                    mon.sel
                } else {
                    mon.stack.iter()
                        .find(|h| self.clients.get(h).map_or(false, |c| c.is_visible_on(mon)))
                        .cloned()
                }
            };
            self.focus(new_sel);
            self.restack(idx);

        } else {
            for i in 0..self.mons.len() {
                self.show_hide(i, &self.mons[i].stack.clone());
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
        let mon = &self.mons[mon_idx];
        if mon.sel.is_none() {
            return;
        }
        if let Some(sel_client) = mon.get_sel_client(&self.clients) {
            if sel_client.is_floating || mon.lt[mon.selected_lt as usize].arrange.is_none() {
                self.xwrapper.raise_window(sel_client.win);
            }
        }

        let mut windows_to_stack: Vec<Window> = Vec::new();
        windows_to_stack.push(mon.bar_window);

        for &handle in &mon.stack {
            if let Some(c) = self.clients.get(&handle) {
                if !c.is_floating && c.is_visible_on(mon) {
                    windows_to_stack.push(c.win);
                }
            }
        }

        self.xwrapper.stack_windows(&windows_to_stack);
    }

    pub fn focus(&mut self, handle: Option<ClientHandle>) {
        if let Some(sel_handle) = self.mons[self.selected_monitor].sel {
            if handle.is_none() || sel_handle != handle.unwrap() {
                self.unfocus(sel_handle, false);
            }
        }

        if let Some(h) = handle {
            // Clear urgent flag when focusing this client
            if let Some(c) = self.clients.get_mut(&h) {
                if c.is_urgent {
                    c.is_urgent = false;
                }
                if c.monitor_idx != self.selected_monitor {
                    self.selected_monitor = c.monitor_idx;
                }
            }
            
            let client_win = if let Some(c) = self.clients.get(&h) {
                Some(c.win)
            } else {
                None
            };

            if let Some(win) = client_win {
                // detachstack(c);
                // attachstack(c);
                self.grab_buttons(h, true);
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
                    .grab_keys(win, self.numlock_mask, &key_specs);
                self.xwrapper.set_window_border_color(win, Colour::WindowActive);
                self
                    .xwrapper
                    .set_input_focus(win, xlib::RevertToPointerRoot);
            }
        } else {
            self
                .xwrapper
                .set_input_focus(self.root, xlib::RevertToPointerRoot);
            // XDeleteProperty(dpy, root, netatom[NetActiveWindow]);
        }
        self.mons[self.selected_monitor].sel = handle;
        self.draw_bars();
    }

    pub fn unfocus(&mut self, handle: ClientHandle, setfocus: bool) {
        let client_win = self.clients.get(&handle).map(|c| c.win);
        if let Some(win) = client_win {
            self.grab_buttons(handle, false);
            self.xwrapper.ungrab_keys(win);
            self.xwrapper.set_window_border_color(win, Colour::WindowInactive);
            if setfocus {
                self
                    .xwrapper
                    .set_input_focus(self.root, xlib::RevertToPointerRoot);
                // XDeleteProperty(dpy, root, netatom[NetActiveWindow]);
            }
        }
    }

    pub fn resize(&mut self, handle: ClientHandle, x: i32, y: i32, w: i32, h: i32) {
        if let Some(client) = self.clients.get_mut(&handle) {
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
    }

    fn grab_buttons(&mut self, handle: ClientHandle, focused: bool) {
        if let Some(client) = self.clients.get(&handle) {
            // First ungrab all existing button grabs
            self.xwrapper.ungrab_button(client.win, xlib::AnyButton as u32, xlib::AnyModifier);
            
            if !focused {
                // For unfocused windows, grab button presses
                self.xwrapper.grab_button(
                    client.win,
                    xlib::AnyButton as u32,
                    xlib::AnyModifier,
                    false, // owner_events = False
                    (xlib::ButtonPressMask | xlib::ButtonReleaseMask) as u32,
                    xlib::GrabModeSync,
                    xlib::GrabModeSync,
                    None,
                    None,
                );
                
                // For motion events, we need to use XSelectInput with PointerMotionMask
                // This will send motion events to the WM instead of the application
                self.xwrapper.select_input(
                    client.win,
                    xlib::EnterWindowMask | xlib::FocusChangeMask | xlib::PropertyChangeMask | xlib::PointerMotionMask
                );
            } else {
                // For focused windows, use normal event mask (no motion grab)
                self.xwrapper.select_input(
                    client.win,
                    xlib::EnterWindowMask | xlib::FocusChangeMask | xlib::PropertyChangeMask
                );
            }
            // For focused windows, we don't need to grab buttons - they get events normally
        }
    }

    pub fn new(command_sender: Sender<GmuxError>, command_receiver: Receiver<GmuxError>) -> Result<Gmux, String> {
        let xwrapper = XWrapper::connect().expect("Failed to open display");
        unsafe {
            let locale = CString::new("").unwrap();
            if libc::setlocale(libc::LC_CTYPE, locale.as_ptr()).is_null()
                || xlib::XSupportsLocale() == 0
            {
                log::warn!("warning: no locale support");
            }

            // if let Err(e) = xwrapper.check_for_other_wm() {
            //     return Err(e);
            // }
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
            tags: ["1", "2", "3", "4", "5"],
            bar_state: BarState::Normal,
            all_commands: Gmux::get_commands(),
            xwrapper,
            command_sender,
            command_receiver,
            clients: HashMap::new(),
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
            
            if !self.xwrapper.fontset_create(FONTS) {
                panic!("no fonts could be loaded.");
            }

            // derive bar height and lr_padding from font height like dwm
            let h = self.xwrapper.get_font_height() as i32;
            if h > 0 {
                self.bar_height = h + 2;
                self.lr_padding = (h + 2) as _;
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

            self.focus(None);
        }
    }


    /// Sends a synthetic ConfigureNotify event to a client.
    pub fn send_configure_notify(&mut self, handle: ClientHandle) {
        if let Some(c) = self.clients.get(&handle) {
            let mut ce: xlib::XConfigureEvent = unsafe { std::mem::zeroed() };
            ce.type_ = xlib::ConfigureNotify;
            ce.display = self.xwrapper.display(); // FIXED: Use the public getter
            ce.event = c.win.0;
            ce.window = c.win.0;
            ce.x = c.x;
            ce.y = c.y;
            ce.width = c.w;
            ce.height = c.h;
            ce.border_width = c.bw;
            ce.above = 0; // None
            ce.override_redirect = 0; // False

            let mut ev: xlib::XEvent = ce.into();

            // Send the event to the client window using the correct general-purpose method
            // FIXED: Call the new, correct function with the correct arguments
            self.xwrapper.send_xevent(
                c.win,
                false, // propagate
                xlib::StructureNotifyMask,
                &mut ev,
            );
        }
    }


}

impl Drop for Gmux {
    fn drop(&mut self) {
        let client_handles: Vec<ClientHandle> = self.clients.keys().cloned().collect();
        for handle in client_handles {
            self.unmanage(handle, false);
        }
        self.xwrapper.ungrab_key(self.root);
    }
}
