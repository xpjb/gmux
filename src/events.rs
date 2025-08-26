use crate::actions::Action;
use crate::config;
use crate::state::Gmux;
use x11::xlib;

pub fn parse_key_press(state: &Gmux, ev: &xlib::XKeyEvent) -> Option<Action> {
    let key_actions = config::grab_keys();
    let keysym = state.xwrapper.keycode_to_keysym(ev.keycode) as u32;
    for key in key_actions.iter() {
        if keysym == key.keysym
            && state.xwrapper.clean_mask(key.mask) == state.xwrapper.clean_mask(ev.state)
        {
            return Some(key.action);
        }
    }
    None
}
pub fn parse_button_press(state: &Gmux, ev: &xlib::XButtonPressedEvent) -> Option<Action> {
    let mon_idx = unsafe { state.window_to_monitor(ev.window) };
    let mon = &state.mons[mon_idx];

    if ev.window == mon.bar_window.0 {
        // Find what was clicked, if anything
        for clickable in &mon.clickables {
            if ev.x >= clickable.pos.x && ev.x < clickable.pos.x + clickable.size.x {
                // This is the clickable element that was clicked
                match clickable.action {
                    Action::ViewTag(_, _) => {
                        // It's a tag
                        match ev.button {
                            1 => return Some(clickable.action), // Left click
                            4 => return Some(Action::CycleTag(-1)), // Scroll up
                            5 => return Some(Action::CycleTag(1)), // Scroll down
                            _ => return None, // Other clicks on tags do nothing
                        }
                    },
                    _ => { // Not a tag
                        if ev.button == 1 {
                            return Some(clickable.action); // Left click for other elements
                        } else {
                            return None;
                        }
                    }
                }
            }
        }
        return None; // Click was on the bar, but not on a clickable element

    } else if let Some((m_idx, c_idx)) = unsafe { state.window_to_client_idx(ev.window) } {
        if ev.button == 1 {
            return Some(Action::FocusClient(m_idx, c_idx));
        }
    }
    None
}

pub unsafe extern "C" fn button_press(state: &mut Gmux, ev: &mut xlib::XButtonPressedEvent) {
    let mon_idx = unsafe { state.window_to_monitor(ev.window) };
    if mon_idx != state.selected_monitor {
        state.unfocus(
            state.selected_monitor,
            state.mons[state.selected_monitor].sel.unwrap(),
            true,
        );
        state.selected_monitor = mon_idx;
        state.focus(mon_idx, None);
    }
    if let Some(action) = parse_button_press(state, ev) {
        action.execute(state);
    }
}

// DestroyNotify handler to unmanage windows
pub unsafe extern "C" fn destroy_notify(state: &mut Gmux, ev: &mut xlib::XDestroyWindowEvent) {
    if let Some((mon_idx, client_idx)) = unsafe { state.window_to_client_idx(ev.window) } {
        state.unmanage(mon_idx, client_idx, true);
    }
}

pub unsafe extern "C" fn motion_notify(state: &mut Gmux, ev: &mut xlib::XMotionEvent) {
    if ev.window != state.root.0 {
        return;
    }
    let m = state.rect_to_monitor(ev.x_root, ev.y_root, 1, 1);
    if m != state.selected_monitor {
        if let Some(sel_idx) = state.mons[state.selected_monitor].sel {
            state.unfocus(state.selected_monitor, sel_idx, true);
        }
        state.selected_monitor = m;
        state.focus(m, None);
    }
}

pub unsafe extern "C" fn enter_notify(state: &mut Gmux, ev: &mut xlib::XCrossingEvent) {
    // Ignore non-normal or inferior events (same filtering as dwm)
    if (ev.mode != xlib::NotifyNormal as i32 || ev.detail == xlib::NotifyInferior as i32)
        && ev.window != state.root.0
    {
        return;
    }

    // First, try to find the client by the event's window ID.
    // If that fails, it might be a root window event, so find the client by cursor position.
    let client_info = unsafe { state.window_to_client_idx(ev.window) }
        .or_else(|| state.client_at_pos(ev.x_root, ev.y_root));

    if let Some((mon_idx, client_idx)) = client_info {
        // Check if the found client is already selected on its monitor
        if Some(client_idx) != state.mons[mon_idx].sel {
            state.focus(mon_idx, Some(client_idx));
        }
    }
}

pub unsafe extern "C" fn map_request(state: &mut Gmux, ev: &mut xlib::XMapRequestEvent) {
    if let Ok(mut wa) = state.xwrapper.get_window_attributes(crate::xwrapper::Window(ev.window)) {
        if wa.override_redirect != 0 {
            return;
        }
        if unsafe { state.window_to_client_idx(ev.window) }.is_none() {
            unsafe { state.manage(ev.window, &mut wa) };
        }
    }
}

pub unsafe extern "C" fn property_notify(state: &mut Gmux, ev: &mut xlib::XPropertyEvent) {
    // Check if the event is for a window we manage
    if let Some((mon_idx, client_idx)) = unsafe { state.window_to_client_idx(ev.window) } {
        let client = &mut state.mons[mon_idx].clients[client_idx];

        // We only care about name changes.
        // _NET_WM_NAME is the modern, UTF-8 compatible standard.
        // XA_WM_NAME is the older, legacy standard.
        if ev.atom == state.xwrapper.atoms.get(crate::xwrapper::Atom::Net(crate::xwrapper::Net::WMName))
            || ev.atom == xlib::XA_WM_NAME
        {
            // Refetch the window title
            if let Some(new_name) = state.xwrapper.get_window_title(client.win) {
                if new_name != client.name {
                    client.name = new_name;
                    // Redraw the bar for the monitor this client is on
                    let mon_idx = client.monitor_idx;
                    state.draw_bar(mon_idx);
                }
            }
        }
    }
}
