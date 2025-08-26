use std::ffi::CString;
use std::os::raw::c_uchar;
use std::time::{Duration, Instant};
use x11::xlib;
use colour::Colour;
use xwrapper::{Atom, CursorId, KeySpecification, Net, Window, XWrapper};
use state::{Client, Gmux, Monitor};
use config::{KeyBinding, BAR_H_PADDING, BORDER_PX};
use actions::Action;
use layouts::LAYOUTS;
use crate::ivec2::ivec2;

mod ivec2;
mod xwrapper;
mod command;
mod colour;
mod state;
mod config;
mod layouts;
mod actions;
mod events;
mod utils;

pub enum BarState {
    Normal,
    ErrorDisplay {
        message: String,
        expiry: Instant,
    },
}

const TAG_MASK: u32 = (1 << config::TAGS.len()) - 1;

#[derive(PartialEq, Copy, Clone)]
enum CursorType {
    Normal,
    Resize,
    Move,
    Last,
}

impl Gmux {
    fn run(&mut self) {
        self.xwrapper.sync(false);
        while self.running != 0 {
            if let BarState::ErrorDisplay { expiry, .. } = self.bar_state {
                if Instant::now() >= expiry {
                    self.bar_state = BarState::Normal;
                    self.draw_bars();
                }
            }
            if let Some(ev) = self.xwrapper.next_event() {
                match ev {
                    xwrapper::Event::KeyPress(kev) => {
                        if let Some(action) = events::parse_key_press(self, &kev) {
                            action.execute(self);
                        }
                    }
                    xwrapper::Event::ButtonPress(mut bev) => unsafe { events::button_press(self, &mut bev) },
                    xwrapper::Event::MotionNotify(mut mev) => unsafe { events::motion_notify(self, &mut mev) },
                    xwrapper::Event::MapRequest(mut mrev) => unsafe { events::map_request(self, &mut mrev) },
                    xwrapper::Event::DestroyNotify(mut drev) => unsafe { events::destroy_notify(self, &mut drev) },
                    xwrapper::Event::EnterNotify(mut erev) => unsafe { events::enter_notify(self, &mut erev) },
                    xwrapper::Event::PropertyNotify(mut prev) => unsafe { events::property_notify(self, &mut prev) },
                    _ => (),
                }
            }
        }
    }

    fn draw_bar(&mut self, mon_idx: usize) {
        // We clone the message to avoid borrowing issues with `self`
        let message = match &self.bar_state {
            BarState::ErrorDisplay { message, .. } => Some(message.clone()),
            _ => None,
        };

        match self.bar_state {
            BarState::Normal => self.draw_normal_bar(mon_idx),
            BarState::ErrorDisplay { .. } => self.draw_error_bar(mon_idx, &message.unwrap()),
        }
    }

    fn draw_normal_bar(&mut self, mon_idx: usize) {
        let mon = &mut self.mons[mon_idx];
        mon.clickables.clear();

        let bar_wh = ivec2(mon.ww, self.bar_height);
        let barwin = mon.bar_window;
        let mut pos = ivec2(0, 0);
        let box_wh = ivec2(self.bar_height / 6 + 2, self.bar_height / 6 + 2);
        let box_xy = ivec2(self.bar_height / 9, self.bar_height/9);

    // --- 1. Clear the entire bar with the default background ---
        self.xwrapper.rect(Colour::BarBackground, pos, bar_wh, true);

    // --- 2. Render Left-aligned elements (Layout Symbol, Tags) ---

        // Calculate occupied and urgent tags
        let mut occ: u32 = 0;
        let mut urg: u32 = 0;
        for c in &mon.clients {
            occ |= c.tags;
            if c.is_urgent {
                urg |= c.tags;
            }
        }

    // Draw tags
        for i in 0..self.tags.len() {
            let tag = self.tags[i];
        let selected = (mon.tagset[mon.selected_tags as usize] & 1 << i) != 0;
        
            let w = self.xwrapper.text_width(tag) + (BAR_H_PADDING * 2);
        
        let (bg_col, fg_col) = if selected {
            (Colour::BarForeground, Colour::TextNormal)
        } else {
            (Colour::BarBackground, Colour::TextQuiet)
        };

            let tag_wh = ivec2(w as _, self.bar_height);
            self.xwrapper.rect(bg_col, pos, tag_wh, true);

        // Draw indicator for urgent windows on this tag
        if (urg & (1 << i)) != 0 {
            // Note: This draws a border around the entire tag, which differs from
            // dwm's color inversion. This is fine, but just a heads-up.
                self.xwrapper.rect(Colour::WindowActive, pos + ivec2(1, 1), tag_wh - ivec2(2, 2), false);
        }
        
        // --- CORRECTED INDICATOR LOGIC ---
        // 1. Check if ANY client is on this tag.
        if (occ & (1 << i)) != 0 {
            // 2. Determine if the box should be filled. It is filled if the
            //    currently selected client is on this tag.
            let is_filled = mon.sel.map_or(false, |sel_idx| {
                (mon.clients[sel_idx].tags & (1 << i)) != 0
            });

            // 3. Draw the box using the current scheme's foreground color.
                self.xwrapper.rect(fg_col, pos + box_xy, box_wh, is_filled);
        }
        // --- END CORRECTION ---

            self.xwrapper.text(fg_col, pos, tag_wh, BAR_H_PADDING, tag);
            let action = Action::ViewTag(1 << i, Some(mon_idx));
            mon.clickables.push(state::Clickable{pos, size: tag_wh, action});
            pos.x += w as i32;
        }

    // Right Text
    let s = "right_text";
        let w_right = self.xwrapper.text_width(s) + (BAR_H_PADDING * 2);
    let p_right = ivec2(bar_wh.x - w_right as i32, 0);
        let wh_right = ivec2(w_right as i32, self.bar_height);
        self.xwrapper.rect(Colour::BarBackground, p_right, wh_right, true);
        self.xwrapper.text(Colour::TextQuiet, p_right, wh_right, BAR_H_PADDING, s);


    // Center Text
    let s = mon.sel.and_then(|i| mon.clients.get(i).map(|c| c.name.as_str()));
    let (col, text_to_draw) = if let Some(name) = s {
        (Colour::BarForeground, name)
    } else {
        (Colour::BarBackground, "")
    };

    let wh_center = (bar_wh - pos) - wh_right.proj_x();
        self.xwrapper.rect(col, pos, wh_center, true);
        self.xwrapper.text(Colour::TextNormal, pos, wh_center, BAR_H_PADDING, text_to_draw);

    // --- 5. Map the drawing buffer to the screen ---
        self.xwrapper.map_drawable(barwin, 0, 0, bar_wh.x as u32, bar_wh.y as u32);
    }

    fn draw_error_bar(&mut self, mon_idx: usize, message: &str) {
        let mon = &mut self.mons[mon_idx];
        let bar_wh = ivec2(mon.ww, self.bar_height);
        let barwin = mon.bar_window;

        // 1. Clear bar with error color
        self.xwrapper.rect(Colour::Urgent, ivec2(0, 0), bar_wh, true);

        // 2. Draw centered text
        self.xwrapper.text(Colour::TextNormal, ivec2(0, 0), bar_wh, BAR_H_PADDING, message);

        // 3. Map to screen
        self.xwrapper.map_drawable(barwin, 0, 0, bar_wh.x as u32, bar_wh.y as u32);
    }


    fn draw_bars(&mut self) {
        for i in 0..self.mons.len() {
            self.draw_bar(i);
    }
}

    pub fn set_error_state(&mut self, error_msg: String) {
        self.bar_state = BarState::ErrorDisplay {
            message: error_msg,
            expiry: Instant::now() + Duration::from_secs(5),
        };
        self.draw_bars(); // Redraw immediately to show the error
    }

/// Finds a client by its absolute (x, y) coordinates on the screen.
    fn client_at_pos(&self, x: i32, y: i32) -> Option<(usize, usize)> {
        for (mon_idx, m) in self.mons.iter().enumerate() {
        for (client_idx, c) in m.clients.iter().enumerate() {
            // Check only visible clients
            if c.is_visible_on(m) && (x >= c.x && x < c.x + c.w && y >= c.y && y < c.y + c.h) {
                return Some((mon_idx, client_idx));
            }
        }
    }
    None
}

    fn show_hide(&mut self, mon_idx: usize, stack: &[usize]) {
        for &c_idx in stack.iter().rev() {
            let c = &self.mons[mon_idx].clients[c_idx];
            if c.is_visible_on(&self.mons[c.monitor_idx]) {
                self.xwrapper.move_window(c.win, c.x, c.y);
                if self.mons[c.monitor_idx].lt[self.mons[c.monitor_idx].selected_lt as usize].arrange.is_none()
                    || c.is_floating && !c.is_fullscreen
                {
                    unsafe { self.resize(c.monitor_idx, c_idx, c.x, c.y, c.w, c.h, false) };
                }
            }
        }
    
        for &c_idx in stack {
            let c = &self.mons[mon_idx].clients[c_idx];
            if !c.is_visible_on(&self.mons[c.monitor_idx]) {
                self.xwrapper.move_window(c.win, -2 * c.width(), c.y);
            }
        }
    }
    
    
    fn unmanage(&mut self, mon_idx: usize, client_idx: usize, destroyed: bool) {
        let client = if let Some(c) = self.detach(mon_idx, client_idx) {
        c
    } else {
        return;
    };
        self.detachstack(mon_idx, client_idx);
    
    if !destroyed {
            self.xwrapper.unmanage_window(client.win);
        }
        
        let new_sel = self.mons[mon_idx].sel;
        self.focus(mon_idx, new_sel);
        self.arrange(Some(mon_idx));
    }
    
    
    fn pop(&mut self, mon_idx: usize, client_idx: usize) {
        if let Some(client) = self.detach(mon_idx, client_idx) {
            let new_c_idx = self.attach(client);
            self.focus(mon_idx, Some(new_c_idx));
            self.arrange(Some(mon_idx));
        }
    }
    
    
    fn detach(&mut self, mon_idx: usize, client_idx: usize) -> Option<Client> {
        let mon = &mut self.mons[mon_idx];
    if client_idx >= mon.clients.len() {
        return None;
    }
    let client = mon.clients.remove(client_idx);

    if let Some(sel) = mon.sel {
        if sel == client_idx {
            if mon.clients.is_empty() {
                mon.sel = None;
            } else {
                mon.sel = Some(client_idx.min(mon.clients.len() - 1));
            }
        } else if sel > client_idx {
            mon.sel = Some(sel - 1);
        }
    }
    mon.stack.retain(|&i| i != client_idx);
    for s in mon.stack.iter_mut() {
        if *s > client_idx {
            *s -= 1;
        }
    }
    Some(client)
}


    fn attach(&mut self, c: Client) -> usize {
    let mon_idx = c.monitor_idx;
        let mon = &mut self.mons[mon_idx];

    if let Some(sel) = mon.sel.as_mut() {
        *sel += 1;
    }
    for s in mon.stack.iter_mut() {
        *s += 1;
    }

    mon.clients.insert(0, c);
    0
}


    fn attachstack(&mut self, mon_idx: usize, c_idx: usize) {
        self.mons[mon_idx].stack.insert(0, c_idx);
}


    fn detachstack(&mut self, mon_idx: usize, c_idx: usize) {
        let mon = &mut self.mons[mon_idx];
    mon.stack.retain(|&x| x != c_idx);
}

    unsafe fn manage(&mut self, w: xlib::Window, wa: &mut xlib::XWindowAttributes) { unsafe {
    let mut client = Client {
        win: Window(w),
        name: String::new(),
        min_aspect: 0.0,
        max_aspect: 0.0,
        x: wa.x,
        y: wa.y,
        w: wa.width,
        h: wa.height,
        oldx: wa.x,
        oldy: wa.y,
        oldw: wa.width,
        oldh: wa.height,
        base_width: 0,
        base_height: 0,
        width_inc: 0,
        height_inc: 0,
        max_width: 0,
        max_height: 0,
        min_width: 0,
        min_height: 0,
        bw: BORDER_PX,
        _oldbw: wa.border_width,
        tags: 0,
        is_fixed: false,
        is_floating: false,
        is_urgent: false,
        _never_focus: false,
        _old_state: false,
        is_fullscreen: false,
            monitor_idx: self.selected_monitor,
    };

    // 1. Fetch window title
        if let Some(name) = self.xwrapper.get_window_title(client.win) {
        client.name = name;
    }

    // 2. Handle transient windows
        let is_transient = if let Some(parent_win) = self.xwrapper.get_transient_for_hint(client.win) {
            if let Some((mon_idx, client_idx)) = self.window_to_client_idx(parent_win.0) {
                let parent_client = &self.mons[mon_idx].clients[client_idx];
            client.monitor_idx = parent_client.monitor_idx;
            client.tags = parent_client.tags;
            client.is_floating = true;
            true
        } else {
            false
        }
    } else {
        false
    };

    if !is_transient {
        // Assign to currently selected tag set so client is visible
            client.tags = self.mons[self.selected_monitor].tagset[self.mons[self.selected_monitor].selected_tags as usize];
            client.monitor_idx = self.selected_monitor;
    }

    // 3. Process size hints
        if let Ok(hints) = self.xwrapper.get_wm_normal_hints(client.win) {
        if hints.flags & xlib::PBaseSize != 0 {
            client.base_width = hints.base_width;
            client.base_height = hints.base_height;
        } else if hints.flags & xlib::PMinSize != 0 {
            client.base_width = hints.min_width;
            client.base_height = hints.min_height;
        }
        if hints.flags & xlib::PResizeInc != 0 {
            client.width_inc = hints.width_inc;
            client.height_inc = hints.height_inc;
        }
        if hints.flags & xlib::PMaxSize != 0 {
            client.max_width = hints.max_width;
            client.max_height = hints.max_height;
        }
        if hints.flags & xlib::PMinSize != 0 {
            client.min_width = hints.min_width;
            client.min_height = hints.min_height;
        }

        if hints.flags & xlib::PAspect != 0 {
            client.min_aspect = hints.min_aspect.y as f32 / hints.min_aspect.x as f32;
            client.max_aspect = hints.max_aspect.x as f32 / hints.max_aspect.y as f32;
        }

        if client.max_width > 0 && client.max_height > 0 && client.max_width == client.min_width && client.max_height == client.min_height {
            client.is_fixed = true;
            client.is_floating = true;
        }
    }

    let win_copy = client.win;
        let c_idx = self.attach(client);
        self.attachstack(self.selected_monitor, c_idx);
        
        self.arrange(Some(self.selected_monitor));
        let sel_client_idx = self.mons[self.selected_monitor].clients.iter().position(|c| c.win.0 == win_copy.0).unwrap();
        let sel_client = &self.mons[self.selected_monitor].clients[sel_client_idx];
        
        self.xwrapper.select_input(
        sel_client.win,
        xlib::EnterWindowMask | xlib::FocusChangeMask | xlib::PropertyChangeMask
    );

        self.xwrapper.map_window(sel_client.win);
        self.focus(self.selected_monitor, Some(sel_client_idx));
}}


    unsafe fn window_to_client_idx(&self, w: xlib::Window) -> Option<(usize, usize)> {
        for (mon_idx, m) in self.mons.iter().enumerate() {
        if let Some(client_idx) = m.clients.iter().position(|c| c.win.0 == w) {
            return Some((mon_idx, client_idx));
        }
    }
    None
}

    fn scan(&mut self) {
        if let Ok((_, _, wins)) = self.xwrapper.query_tree(self.root) {
        for &win in &wins {
                if let Ok(_wa) = self.xwrapper.get_window_attributes(win) {
                    if self.xwrapper.get_transient_for_hint(win).is_some() {
                    continue;
                }
                // Potentially manage the window here if it's not already managed
            }
        }
    }
}
}



fn main() {
    println!("Starting gmux...");
    match Gmux::new() {
        Ok(mut gmux) => {
            gmux.scan();
            gmux.run();
        }
        Err(e) => panic!("{}", e),
    }
}
