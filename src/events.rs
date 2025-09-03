use crate::*;
use x11::xlib;

pub fn parse_key_press(state: &Gmux, ev: &xlib::XKeyEvent) -> Option<Action> {
    let key_actions = config::grab_keys();
    let keysym = state.xwrapper.keycode_to_keysym(ev.keycode) as u32;
    for key in key_actions.iter() {
        if keysym == key.keysym
            && state.xwrapper.clean_mask(key.mask) == state.xwrapper.clean_mask(ev.state)
        {
            return Some(key.action.clone());
        }
    }
    None
}
pub fn parse_button_press(state: &Gmux, ev: &xlib::XButtonPressedEvent) -> Option<Action> {
    let mon_idx = unsafe { state.window_to_monitor(ev.window) };
    let mon = &state.mons[mon_idx];

    if ev.window == mon.bar_window.0 {
        // Handle scroll events anywhere on the bar
        match ev.button {
            4 => return Some(Action::CycleTag(1)), // Scroll up anywhere on bar
            5 => return Some(Action::CycleTag(-1)),  // Scroll down anywhere on bar
            _ => {} // Continue to clickable detection for other buttons
        }

        // Find what was clicked, if anything
        for clickable in &mon.clickables {
            if ev.x >= clickable.pos.x && ev.x < clickable.pos.x + clickable.size.x {
                // This is the clickable element that was clicked
                match clickable.action {
                    Action::ViewTag(_, _) => {
                        // It's a tag
                        if ev.button == 1 {
                            return Some(clickable.action.clone()); // Left click
                        }
                    },
                    _ => { // Not a tag
                        if ev.button == 1 {
                            return Some(clickable.action.clone()); // Left click for other elements
                        }
                    }
                }
            }
        }
        return None; // Click was on the bar, but not on a clickable element

    } else if let Some(handle) = state.window_to_client_handle(ev.window) {
        if ev.button == 1 {
            return Some(Action::FocusClient(handle));
        }
    }
    None
}

/// Handles ConfigureNotify events. This is the key handler for detecting screen
/// changes after waking from sleep.
pub unsafe fn configure_notify(state: &mut Gmux, ev: &mut xlib::XConfigureEvent) {
    // We only care about configure events on the root window.
    if ev.window == state.root.0 {
        log::info!("Root window configured. Re-arranging all monitors to adapt to potential screen changes.");
        // In DWM, this triggers updategeom() and then a full rearrange.
        // Calling arrange on all monitors is the correct equivalent.
        state.arrange(None);
    }
}

/// Handles Expose events, which are requests to redraw a window.
/// This acts as a fallback to ensure the bar is redrawn if its contents are damaged.
pub unsafe fn expose(state: &mut Gmux, ev: &mut xlib::XExposeEvent) {
    // ev.count == 0 means this is the last in a series of expose events.
    if ev.count == 0 {
        // Iterate through monitors to find which bar needs redrawing.
        for i in 0..state.mons.len() {
            if state.mons[i].bar_window.0 == ev.window {
                log::info!("Bar for monitor {} exposed. Redrawing.", i);
                state.draw_bar(i);
                return; // Found the bar, no need to check others
            }
        }
    }
}

/// Handles requests from client windows to change their own configuration (size, position).
pub unsafe fn configure_request(state: &mut Gmux, ev: &mut xlib::XConfigureRequestEvent) {
    if let Some(handle) = state.window_to_client_handle(ev.window) {
        // If the window is managed by us, we must release our mutable borrow of the client
        // before we can borrow `state` again to call other methods.
        
        // 1. Determine the action to take and gather necessary info.
        let (is_floating, new_geom) = {
            // Scope the mutable borrow so it's released immediately after.
            let client = state.clients.get_mut(&handle).unwrap();
            
            if client.is_floating {
                // If it's floating, update its internal state and store the new geometry.
                if ev.value_mask & xlib::CWX as u64 != 0 { client.x = state.mons[client.monitor_idx].wx + ev.x; }
                if ev.value_mask & xlib::CWY as u64 != 0 { client.y = state.mons[client.monitor_idx].wy + ev.y; }
                if ev.value_mask & xlib::CWWidth as u64 != 0 { client.w = ev.width; }
                if ev.value_mask & xlib::CWHeight as u64 != 0 { client.h = ev.height; }
                (true, Some((client.x, client.y, client.w, client.h)))
            } else {
                // It's tiled, so we will ignore the request.
                (false, None)
            }
        }; // `client` borrow ends here.

        // 2. Now perform the action. We can mutably borrow `state` again.
        if is_floating {
            let (x, y, w, h) = new_geom.unwrap();
            state.resize(handle, x, y, w, h);
        } else {
            // For tiled clients, ignore the request and enforce our layout.
            state.send_configure_notify(handle);
        }

    } else {
        // If we don't manage this window, just approve the request.
        state.xwrapper.configure_window(
            crate::xwrapper::Window(ev.window),
            ev.x,
            ev.y,
            ev.width,
            ev.height,
            ev.border_width,
        );
    }
    state.xwrapper.sync(false);
}

pub unsafe fn button_press(state: &mut Gmux, ev: &mut xlib::XButtonPressedEvent) {
    if let Some(handle) = state.window_to_client_handle(ev.window) {
        if let Some(client) = state.clients.get(&handle) {
            let mon_idx = client.monitor_idx;
            
            // Switch monitor if necessary
            if mon_idx != state.selected_monitor {
                if let Some(sel_handle) = state.mons[state.selected_monitor].sel {
                    state.unfocus(sel_handle, true);
                }
                state.selected_monitor = mon_idx;
            }
            
            // Always focus the clicked client, even if on the same monitor
            if state.mons[mon_idx].sel != Some(handle) {
                state.focus(Some(handle));
            }
        }
    } else {
        let mon_idx = unsafe { state.window_to_monitor(ev.window) };
        if mon_idx != state.selected_monitor {
            if let Some(sel_handle) = state.mons[state.selected_monitor].sel {
                state.unfocus(sel_handle, true);
            }
            state.selected_monitor = mon_idx;
            state.focus(None);
        }
    }

    if let Some(action) = parse_button_press(state, ev) {
        action.execute(state);
    }
    
    // If this was a grabbed button event (click on unfocused window), 
    // replay the event to the application
    if state.window_to_client_handle(ev.window).is_some() {
        state.xwrapper.allow_events(xlib::ReplayPointer);
    }
}

// DestroyNotify handler to unmanage windows
pub unsafe fn destroy_notify(state: &mut Gmux, ev: &mut xlib::XDestroyWindowEvent) {
    if let Some(handle) = state.window_to_client_handle(ev.window) {
        state.unmanage(handle, true);
    }
}

pub unsafe fn motion_notify(state: &mut Gmux, ev: &mut xlib::XMotionEvent) {
    // Handle motion events from client windows (via XSelectInput)
    if let Some(handle) = state.window_to_client_handle(ev.window) {
        if let Some(client) = state.clients.get(&handle) {
            // Focus this client if it's not already focused
            if state.mons[client.monitor_idx].sel != Some(handle) {
                state.focus(Some(handle));
            }
            // Note: No need for allow_events with XSelectInput - motion events aren't "grabbed"
        }
        return;
    }
    
    // Handle motion events on root window (original logic)
    if ev.window == state.root.0 {
        let m = state.rect_to_monitor(ev.x_root, ev.y_root, 1, 1);
        if m != state.selected_monitor {
            if let Some(sel_handle) = state.mons[state.selected_monitor].sel {
                state.unfocus(sel_handle, true);
            }
            state.selected_monitor = m;
            state.focus(None);
        }
        
        // Also check if there's a client at this position (for root window motion)
        if let Some(handle) = state.client_at_pos(ev.x_root, ev.y_root) {
            if let Some(client) = state.clients.get(&handle) {
                if state.mons[client.monitor_idx].sel != Some(handle) {
                    log::info!("MotionNotify: root motion focusing client {:?} at ({}, {})", handle, ev.x_root, ev.y_root);
                    state.focus(Some(handle));
                }
            }
        }
    }
}

pub unsafe fn enter_notify(state: &mut Gmux, ev: &mut xlib::XCrossingEvent) {
    // Ignore non-normal or inferior events (same filtering as dwm)
    if (ev.mode != xlib::NotifyNormal as i32 || ev.detail == xlib::NotifyInferior as i32)
        && ev.window != state.root.0
    {
        return;
    }

    // First, try to find the client by the event's window ID.
    // If that fails, it might be a root window event, so find the client by cursor position.
    let handle = state.window_to_client_handle(ev.window)
        .or_else(|| state.client_at_pos(ev.x_root, ev.y_root));

    if let Some(h) = handle {
        if let Some(c) = state.clients.get(&h) {
            if state.mons[c.monitor_idx].sel.is_some() && Some(h) != state.mons[c.monitor_idx].sel {
                state.focus(Some(h));
            }
        }
    }
}

pub unsafe fn map_request(state: &mut Gmux, ev: &mut xlib::XMapRequestEvent) {
    if let Ok(mut wa) = state.xwrapper.get_window_attributes(crate::xwrapper::Window(ev.window)) {
        if wa.override_redirect != 0 {
            return;
        }
        if state.window_to_client_handle(ev.window).is_none() {
            unsafe { state.manage(ev.window, &mut wa) };
        }
    }
}

pub unsafe fn property_notify(state: &mut Gmux, ev: &mut xlib::XPropertyEvent) {
    // Check if the event is for a window we manage
    if let Some(handle) = state.window_to_client_handle(ev.window) {
        if let Some(client) = state.clients.get_mut(&handle) {
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
}
