use std::os::raw::{c_int, c_uint};
use crate::layouts::Layout;
use crate::xwrapper::{CursorId, Window, XWrapper, KeySpecification};
use x11::xlib;
use crate::Colour;

// Structs

#[derive(Debug, Clone)]
pub struct Monitor {
    pub lt_symbol: String,
    pub mfact: f32,
    pub nmaster: i32,
    pub _num: i32,
    pub by: i32,
    pub _mx: i32,
    pub _my: i32,
    pub _mw: i32,
    pub _mh: i32,
    pub wx: i32,
    pub wy: i32,
    pub ww: i32,
    pub wh: i32,
    pub selected_tags: u32,
    pub selected_lt: u32,
    pub tagset: [u32; 2],
    pub show_bar: bool,
    pub top_bar: bool,
    pub clients: Vec<Client>,
    pub sel: Option<usize>,
    pub stack: Vec<usize>,
    pub bar_window: Window,
    pub lt: [&'static Layout; 2],
}

impl Monitor {
    pub fn intersect_area(&self, x: i32, y: i32, w: i32, h: i32) -> i32 {
        std::cmp::max(
            0,
            std::cmp::min(x + w, self.wx + self.ww) - std::cmp::max(x, self.wx),
        ) * std::cmp::max(
            0,
            std::cmp::min(y + h, self.wy + self.wh) - std::cmp::max(y, self.wy),
        )
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Monitor {
            lt_symbol: String::new(),
            mfact: 0.0,
            nmaster: 0,
            _num: 0,
            by: 0,
            _mx: 0,
            _my: 0,
            _mw: 0,
            _mh: 0,
            wx: 0,
            wy: 0,
            ww: 0,
            wh: 0,
            selected_tags: 0,
            selected_lt: 0,
            tagset: [0, 0],
            show_bar: false,
            top_bar: false,
            clients: Vec::new(),
            sel: None,
            stack: Vec::new(),
            bar_window: Window(0),
            lt: [&crate::layouts::LAYOUTS[0], &crate::layouts::LAYOUTS[1]],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Client {
    pub name: String,
    pub min_aspect: f32,
    pub max_aspect: f32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub oldx: i32,
    pub oldy: i32,
    pub oldw: i32,
    pub oldh: i32,
    pub base_width: i32,
    pub base_height: i32,
    pub width_inc: i32,
    pub height_inc: i32,
    pub max_width: i32,
    pub max_height: i32,
    pub min_width: i32,
    pub min_height: i32,
    pub bw: i32,
    pub _oldbw: i32,
    pub tags: u32,
    pub is_fixed: bool,
    pub is_floating: bool,
    pub is_urgent: bool,
    pub _never_focus: bool,
    pub _old_state: bool,
    pub is_fullscreen: bool,
    pub monitor_idx: usize,
    pub win: Window,
}

impl Client {
    pub fn width(&self) -> i32 {
        self.w + 2 * self.bw
    }

    pub fn is_visible_on(&self, m: &Monitor) -> bool {
        (self.tags & m.tagset[m.selected_tags as usize]) != 0
    }
}

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
        if let Some((mon_idx, _)) = unsafe { crate::window_to_client_idx(self, w) } {
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
            crate::show_hide(self, idx, &stack);
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
                crate::show_hide(self, i, &stack);
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
        crate::draw_bar(self, mon_idx);
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
        crate::draw_bars(self);
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
    }
}
