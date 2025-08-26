use std::ffi::CString;
use std::os::raw::c_uchar;
use x11::xlib;
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

use colour::Colour;
use xwrapper::{Atom, CursorId, KeySpecification, Net, Window, XWrapper};
use state::{Client, Gmux, Monitor};
use config::{KeyBinding, BAR_H_PADDING, BORDER_PX};
use actions::Action;
use layouts::LAYOUTS;

fn draw_bar(state: &mut Gmux, mon_idx: usize) {
    let mon = &state.mons[mon_idx];
    let bar_wh = ivec2(mon.ww, state.bar_height);
    let barwin = mon.bar_window;
    let mut pos = ivec2(0, 0);
    let box_wh = ivec2(state.bar_height / 6 + 2, state.bar_height / 6 + 2);
    let box_xy = ivec2(state.bar_height / 9, state.bar_height/9);

    // --- 1. Clear the entire bar with the default background ---
    state.xwrapper.rect(Colour::BarBackground, pos, bar_wh, true);

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
    for i in 0..state.tags.len() {
        let tag = state.tags[i];
        let selected = (mon.tagset[mon.selected_tags as usize] & 1 << i) != 0;
        
        let w = state.xwrapper.text_width(tag) + (BAR_H_PADDING * 2);
        
        let (bg_col, fg_col) = if selected {
            (Colour::BarForeground, Colour::TextNormal)
        } else {
            (Colour::BarBackground, Colour::TextQuiet)
        };

        let tag_wh = ivec2(w as _, state.bar_height);
        state.xwrapper.rect(bg_col, pos, tag_wh, true);

        // Draw indicator for urgent windows on this tag
        if (urg & (1 << i)) != 0 {
            // Note: This draws a border around the entire tag, which differs from
            // dwm's color inversion. This is fine, but just a heads-up.
            state.xwrapper.rect(Colour::WindowActive, pos + ivec2(1, 1), tag_wh - ivec2(2, 2), false);
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
            state.xwrapper.rect(fg_col, pos + box_xy, box_wh, is_filled);
        }
        // --- END CORRECTION ---

        state.xwrapper.text(fg_col, pos, tag_wh, BAR_H_PADDING, tag);
        pos.x += w as i32;
    }

    // Right Text
    let s = "right_text";
    let w_right = state.xwrapper.text_width(s) + (BAR_H_PADDING * 2);
    let p_right = ivec2(bar_wh.x - w_right as i32, 0);
    let wh_right = ivec2(w_right as i32, state.bar_height);
    state.xwrapper.rect(Colour::BarBackground, p_right, wh_right, true);
    state.xwrapper.text(Colour::TextQuiet, p_right, wh_right, BAR_H_PADDING, s);


    // Center Text
    let s = mon.sel.and_then(|i| mon.clients.get(i).map(|c| c.name.as_str()));
    let (col, text_to_draw) = if let Some(name) = s {
        (Colour::BarForeground, name)
    } else {
        (Colour::BarBackground, "")
    };

    let wh_center = (bar_wh - pos) - wh_right.proj_x();
    state.xwrapper.rect(col, pos, wh_center, true);
    state.xwrapper.text(Colour::TextNormal, pos, wh_center, BAR_H_PADDING, text_to_draw);

    // --- 5. Map the drawing buffer to the screen ---
    state.xwrapper.map_drawable(barwin, 0, 0, bar_wh.x as u32, bar_wh.y as u32);
}


fn draw_bars(state: &mut Gmux) {
    for i in 0..state.mons.len() {
        draw_bar(state, i);
    }
}

/// Finds a client by its absolute (x, y) coordinates on the screen.
fn client_at_pos(state: &Gmux, x: i32, y: i32) -> Option<(usize, usize)> {
    for (mon_idx, m) in state.mons.iter().enumerate() {
        for (client_idx, c) in m.clients.iter().enumerate() {
            // Check only visible clients
            if is_visible(c, m) && (x >= c.x && x < c.x + c.w && y >= c.y && y < c.y + c.h) {
                return Some((mon_idx, client_idx));
            }
        }
    }
    None
}

// Enums

#[derive(PartialEq, Copy, Clone)]
enum CursorType {
    Normal,
    Resize,
    Move,
    Last,
}

/// There's no way to check accesses to destroyed windows, thus those cases are
/// ignored (especially on UnmapNotify's). Other types of errors call Xlibs
/// default error handler, which may call exit.


fn setup(state: &mut Gmux) {
    unsafe {
        state.screen = state.xwrapper.default_screen();
        state.screen_width = state.xwrapper.display_width(state.screen);
        state.screen_height = state.xwrapper.display_height(state.screen);
        state.root = state.xwrapper.root_window(state.screen);
        
        let fonts = &["monospace:size=12"]; // TODO: configurable
        if !state.xwrapper.fontset_create(fonts) {
            die("no fonts could be loaded.");
        }

        // derive bar height and lr_padding from font height like dwm
        let h = state.xwrapper.get_font_height() as i32;
        if h > 0 {
            state.bar_height = h + 2;
            state.lr_padding = h + 2;
        }

        // initialise status text sample
        state.status_text = "gmux".to_string();

        draw_bars(state);

        // Create a monitor
        let mut mon = Monitor::default();
        mon.tagset = [1, 1];
        mon.mfact = 0.55;
        mon.nmaster = 1;
        mon.show_bar = true;
        mon.top_bar = true;
        // Calculate window area accounting for the bar height
        if mon.show_bar {
            mon.by = if mon.top_bar { 0 } else { state.screen_height - state.bar_height };
            mon.wy = if mon.top_bar { state.bar_height } else { 0 };
            mon.wh = state.screen_height - state.bar_height;
        } else {
            mon.by = -state.bar_height;
            mon.wy = 0;
            mon.wh = state.screen_height;
        }
        mon.lt[0] = &LAYOUTS[0];
        mon.lt[1] = &LAYOUTS[1];
        mon.lt_symbol = LAYOUTS[0].symbol.to_string();
        mon.wx = 0;
        mon.ww = state.screen_width;
        let mut wa: xlib::XSetWindowAttributes = std::mem::zeroed();
        wa.override_redirect = 1;
        wa.background_pixmap = xlib::ParentRelative as u64;
        wa.event_mask = xlib::ButtonPressMask | xlib::ExposureMask;
        let valuemask = xlib::CWOverrideRedirect | xlib::CWBackPixmap | xlib::CWEventMask;
        let barwin = state.xwrapper.create_window(
            state.root,
            mon.wx,
            mon.by,
            mon.ww as u32,
            state.bar_height as u32,
            0,
            state.xwrapper.default_depth(state.screen),
            xlib::InputOutput as u32,
            state.xwrapper.default_visual(state.screen),
            valuemask as u64,
            &mut wa,
        );
        mon.bar_window = barwin;
        state.xwrapper.map_raised(mon.bar_window);
        state.mons.push(mon);
        state.selected_monitor = state.mons.len() - 1;

        state.cursor[CursorType::Normal as usize] = state.xwrapper.create_font_cursor_as_id(68);
        state.cursor[CursorType::Resize as usize] = state.xwrapper.create_font_cursor_as_id(120);
        state.cursor[CursorType::Move as usize] = state.xwrapper.create_font_cursor_as_id(52);
        
        state.wm_check_window = state.xwrapper.create_simple_window(state.root, 0, 0, 1, 1, 0, 0, 0);
        let wmcheckwin_val = state.wm_check_window.0;
        state.xwrapper.change_property(state.wm_check_window, state.xwrapper.atoms.get(Atom::Net(Net::WMCheck)), xlib::XA_WINDOW, 32,
            xlib::PropModeReplace, &wmcheckwin_val as *const u64 as *const c_uchar, 1);

        let dwm_name = CString::new("dwm").unwrap();
        state.xwrapper.change_property(state.wm_check_window, state.xwrapper.atoms.get(Atom::Net(Net::WMName)), xlib::XA_STRING, 8,
            xlib::PropModeReplace, dwm_name.as_ptr() as *const c_uchar, 3);
        state.xwrapper.change_property(state.root, state.xwrapper.atoms.get(Atom::Net(Net::WMCheck)), xlib::XA_WINDOW, 32,
            xlib::PropModeReplace, &wmcheckwin_val as *const u64 as *const c_uchar, 1);

        state.xwrapper.change_property(state.root, state.xwrapper.atoms.get(Atom::Net(Net::Supported)), xlib::XA_ATOM, 32,
            xlib::PropModeReplace, state.xwrapper.atoms.net_atom_ptr() as *const c_uchar, Net::Last as i32);
        state.xwrapper.delete_property(state.root, state.xwrapper.atoms.get(Atom::Net(Net::ClientList)));

        let mut wa: xlib::XSetWindowAttributes = std::mem::zeroed();
        wa.cursor = state.cursor[CursorType::Normal as usize].0;
        wa.event_mask = (xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask
            | xlib::ButtonPressMask | xlib::PointerMotionMask | xlib::EnterWindowMask
            | xlib::LeaveWindowMask | xlib::StructureNotifyMask | xlib::PropertyChangeMask
            | xlib::KeyPressMask) as i64;
        state.xwrapper.change_window_attributes(state.root, (xlib::CWEventMask | xlib::CWCursor) as u64, &mut wa);
        state.xwrapper.select_input(state.root, wa.event_mask);

        // Update NumLockMask and grab global keys
        state.numlock_mask = state.xwrapper.get_numlock_mask();
        let key_actions: Vec<KeyBinding> = config::grab_keys();
        let key_specs: Vec<KeySpecification> = key_actions
            .iter()
            .map(|k| KeySpecification {
                mask: k.mask,
                keysym: k.keysym,
            })
            .collect();
        state
            .xwrapper
            .grab_keys(state.root, state.numlock_mask, &key_specs);

        state.focus(0, None);
    }
}

fn die(s: &str) {
    eprintln!("{}", s);
    std::process::exit(1);
}

const TAG_MASK: u32 = (1 << config::TAGS.len()) - 1;


fn show_hide(state: &mut Gmux, mon_idx: usize, stack: &[usize]) {
    for &c_idx in stack.iter().rev() {
        let c = &state.mons[mon_idx].clients[c_idx];
        if is_visible(c, &state.mons[c.monitor_idx]) {
            state.xwrapper.move_window(c.win, c.x, c.y);
            if state.mons[c.monitor_idx].lt[state.mons[c.monitor_idx].selected_lt as usize].arrange.is_none()
                || c.is_floating && !c.is_fullscreen
            {
                unsafe { resize(state, c.monitor_idx, c_idx, c.x, c.y, c.w, c.h, false) };
            }
        }
    }

    for &c_idx in stack {
        let c = &state.mons[mon_idx].clients[c_idx];
        if !is_visible(c, &state.mons[c.monitor_idx]) {
            state.xwrapper.move_window(c.win, -2 * client_width(c), c.y);
        }
    }
}


fn unmanage(state: &mut Gmux, mon_idx: usize, client_idx: usize, destroyed: bool) {
    let client = if let Some(c) = detach(state, mon_idx, client_idx) {
        c
    } else {
        return;
    };
    detachstack(state, mon_idx, client_idx);
    
    if !destroyed {
        state.xwrapper.unmanage_window(client.win);
    }
    
    let new_sel = state.mons[mon_idx].sel;
    state.focus(mon_idx, new_sel);
    state.arrange(Some(mon_idx));
}


fn pop(state: &mut Gmux, mon_idx: usize, client_idx: usize) {
    if let Some(client) = detach(state, mon_idx, client_idx) {
        let new_c_idx = attach(state, client);
        state.focus(mon_idx, Some(new_c_idx));
        state.arrange(Some(mon_idx));
    }
}


fn detach(state: &mut Gmux, mon_idx: usize, client_idx: usize) -> Option<Client> {
    let mon = &mut state.mons[mon_idx];
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


fn attach(state: &mut Gmux, c: Client) -> usize {
    let mon_idx = c.monitor_idx;
    let mon = &mut state.mons[mon_idx];

    if let Some(sel) = mon.sel.as_mut() {
        *sel += 1;
    }
    for s in mon.stack.iter_mut() {
        *s += 1;
    }

    mon.clients.insert(0, c);
    0
}


fn attachstack(state: &mut Gmux, mon_idx: usize, c_idx: usize) {
    state.mons[mon_idx].stack.insert(0, c_idx);
}


fn detachstack(state: &mut Gmux, mon_idx: usize, c_idx: usize) {
    let mon = &mut state.mons[mon_idx];
    mon.stack.retain(|&x| x != c_idx);
}

unsafe fn manage(state: &mut Gmux, w: xlib::Window, wa: &mut xlib::XWindowAttributes) { unsafe {
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
        monitor_idx: state.selected_monitor,
    };

    // 1. Fetch window title
    if let Some(name) = state.xwrapper.get_window_title(client.win) {
        client.name = name;
    }

    // 2. Handle transient windows
    let is_transient = if let Some(parent_win) = state.xwrapper.get_transient_for_hint(client.win) {
        if let Some((mon_idx, client_idx)) = window_to_client_idx(state, parent_win.0) {
            let parent_client = &state.mons[mon_idx].clients[client_idx];
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
        client.tags = state.mons[state.selected_monitor].tagset[state.mons[state.selected_monitor].selected_tags as usize];
        client.monitor_idx = state.selected_monitor;
    }

    // 3. Process size hints
    if let Ok(hints) = state.xwrapper.get_wm_normal_hints(client.win) {
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
    let c_idx = attach(state, client);
    attachstack(state, state.selected_monitor, c_idx);
    
    state.arrange(Some(state.selected_monitor));
    let sel_client_idx = state.mons[state.selected_monitor].clients.iter().position(|c| c.win.0 == win_copy.0).unwrap();
    let sel_client = &state.mons[state.selected_monitor].clients[sel_client_idx];
    
    state.xwrapper.select_input(
        sel_client.win,
        xlib::EnterWindowMask | xlib::FocusChangeMask | xlib::PropertyChangeMask
    );

    state.xwrapper.map_window(sel_client.win);
    state.focus(state.selected_monitor, Some(sel_client_idx));
}}


unsafe fn window_to_client_idx(state: &Gmux, w: xlib::Window) -> Option<(usize, usize)> {
    for (mon_idx, m) in state.mons.iter().enumerate() {
        if let Some(client_idx) = m.clients.iter().position(|c| c.win.0 == w) {
            return Some((mon_idx, client_idx));
        }
    }
    None
}

fn intersect(x: i32, y: i32, w: i32, h: i32, m: &Monitor) -> i32 {
    std::cmp::max(
        0,
        std::cmp::min(x + w, m.wx + m.ww) - std::cmp::max(x, m.wx),
    ) * std::cmp::max(
        0,
        std::cmp::min(y + h, m.wy + m.wh) - std::cmp::max(y, m.wy),
    )
}



fn grab_buttons(_state: &mut Gmux, _mon_idx: usize, _c_idx: usize, _focused: bool) {
    // For now, this is a stub
}


// Helper functions for layouts

unsafe fn resize(
    state: &mut Gmux,
    mon_idx: usize,
    client_idx: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    _interact: bool,
) {
    let client = &mut state.mons[mon_idx].clients[client_idx];
    client.oldx = client.x;
    client.x = x;
    client.oldy = client.y;
    client.y = y;
    client.oldw = client.w;
    client.w = w;
    client.oldh = client.h;
    client.h = h;
    state.xwrapper.configure_window(
        client.win,
        client.x,
        client.y,
        client.w,
        client.h,
        BORDER_PX,
    );
}

fn is_visible(c: &Client, m: &Monitor) -> bool {
    (c.tags & m.tagset[m.selected_tags as usize]) != 0
}

fn scan(state: &mut Gmux) {
    if let Ok((_, _, wins)) = state.xwrapper.query_tree(state.root) {
        for &win in &wins {
            if let Ok(_wa) = state.xwrapper.get_window_attributes(win) {
                if state.xwrapper.get_transient_for_hint(win).is_some() {
                    continue;
                }
                // Potentially manage the window here if it's not already managed
            }
        }
    }
}

fn run(state: &mut Gmux) {
    state.xwrapper.sync(false);
    while state.running != 0 {
        if let Some(mut ev) = state.xwrapper.next_event() {
            let event_type = ev.get_type();
            match event_type {
                xlib::KeyPress => {
                    let kev = unsafe { &*(ev.as_mut() as *mut _ as *mut xlib::XKeyEvent) };
                    if let Some(action) = events::parse_key_press(state, kev) {
                        action.execute(state);
                    }
                }
                xlib::ButtonPress => unsafe { events::button_press(state, &mut ev) },
                xlib::MotionNotify => unsafe { events::motion_notify(state, &mut ev) },
                xlib::MapRequest => unsafe { events::map_request(state, &mut ev) },
                xlib::DestroyNotify => unsafe { events::destroy_notify(state, &mut ev) },
                xlib::EnterNotify => unsafe { events::enter_notify(state, &mut ev) },
                xlib::PropertyNotify => unsafe { events::property_notify(state, &mut ev) },

                _ => (),
            }
        }
    }
}


fn cleanup(state: &mut Gmux) {
    for i in 0..state.mons.len() {
        while !state.mons[i].stack.is_empty() {
            let c_idx = state.mons[i].stack.pop().unwrap();
            unmanage(state, i, c_idx, false);
        }
    }
    state.xwrapper.ungrab_key(state.root);
}


fn client_width(c: &Client) -> i32 {
    c.w + 2 * c.bw
}

fn main() {
    println!("Starting gmux...");
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
        xwrapper: XWrapper::connect().expect("Failed to open display"),
        mons: Vec::new(),
        selected_monitor: 0,
        root: Window(0),
        wm_check_window: Window(0),
        _xerror: false,
        tags: ["1", "2", "3", "4", "5", "6", "7", "8", "9"],
    };
    
    unsafe {
        let locale = CString::new("").unwrap();
        if libc::setlocale(libc::LC_CTYPE, locale.as_ptr()).is_null()
            || xlib::XSupportsLocale() == 0
        {
            eprintln!("warning: no locale support");
        }

        if let Err(e) = state.xwrapper.check_for_other_wm() {
            die(e);
        }
        state.xwrapper.set_default_error_handler();

        setup(&mut state);
        scan(&mut state);
        run(&mut state);
        
        cleanup(&mut state);
    }
}
