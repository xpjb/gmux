use x11::xlib;
use crate::*;

#[derive(Clone, Debug)]
pub enum Action {
    Spawn(String),
    ToggleBar,
    FocusStack(i32),
    IncNMaster(i32),
    SetMFact(f32),
    Zoom,
    ViewTag(u32, Option<usize>),
    ViewPrevTag,
    KillClient,
    SetLayout(&'static Layout),
    ToggleFloating,
    Tag(u32),
    FocusMon(i32),
    TagMon(i32),
    Quit,
    ToggleView(u32),
    ToggleTag(u32),
    CycleTag(i32),
    FocusClient(usize, usize),
    EnterLauncherMode,
}

impl Action {
    pub fn execute(&self, state: &mut Gmux) {
        match self {
            Action::Spawn(_cmd) => {
                // This will be handled in main.rs now
            }
            Action::EnterLauncherMode => {
                state.enter_launcher_mode();
            }
            Action::ToggleBar => {
                let selmon_idx = state.selected_monitor;
                let selmon = &mut state.mons[selmon_idx];
                selmon.show_bar = !selmon.show_bar;
                state.arrange(Some(selmon_idx));
            }
            Action::FocusStack(i) => {
                let selmon_idx = state.selected_monitor;
                let selmon = &state.mons[selmon_idx];
                if selmon.sel.is_none() {
                    return;
                }
                let sel_idx = selmon.sel.unwrap();
                let c_idx: usize;

                let visible_clients_indices: Vec<usize> = selmon
                    .clients
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.is_visible_on(selmon))
                    .map(|(i, _)| i)
                    .collect();
                if visible_clients_indices.is_empty() {
                    return;
                }

                if let Some(pos) = visible_clients_indices.iter().position(|&i| i == sel_idx) {
                    c_idx = if *i > 0 {
                        visible_clients_indices[(pos + 1) % visible_clients_indices.len()]
                    } else {
                        visible_clients_indices
                            [(pos + visible_clients_indices.len() - 1) % visible_clients_indices.len()]
                    };
                } else {
                    c_idx = visible_clients_indices[0];
                }

                if c_idx < selmon.clients.len() {
                    state.focus(selmon_idx, Some(c_idx));
                    state.restack(selmon_idx);
                }
            }
            Action::IncNMaster(i) => {
                let selmon_idx = state.selected_monitor;
                let selmon = &mut state.mons[selmon_idx];
                selmon.nmaster = std::cmp::max(selmon.nmaster + i, 0);
                state.arrange(Some(selmon_idx));
            }
            Action::SetMFact(f) => {
                let selmon_idx = state.selected_monitor;
                let selmon = &mut state.mons[selmon_idx];
                if selmon.lt[selmon.selected_lt as usize].arrange.is_none() {
                    return;
                }
                let new_f = if *f < 1.0 {
                    *f + selmon.mfact
                } else {
                    *f - 1.0
                };
                if new_f < 0.05 || new_f > 0.95 {
                    return;
                }
                selmon.mfact = new_f;
                state.arrange(Some(selmon_idx));
            }
            Action::Zoom => {
                let selmon_idx = state.selected_monitor;
                if let Some(sel_idx) = state.mons[selmon_idx].sel {
                    let c = &state.mons[selmon_idx].clients[sel_idx];
                    if state.mons[selmon_idx].lt[state.mons[selmon_idx].selected_lt as usize]
                        .arrange
                        .is_none()
                        || c.is_floating
                    {
                        return;
                    }

                    let tiled_clients_indices: Vec<usize> = state.mons[selmon_idx]
                        .clients
                        .iter()
                        .enumerate()
                        .filter(|(_, cl)| !cl.is_floating && cl.is_visible_on(&state.mons[selmon_idx]))
                        .map(|(i, _)| i)
                        .collect();
                    if let Some(pos) = tiled_clients_indices.iter().position(|&i| i == sel_idx) {
                        if pos == 0 {
                            if tiled_clients_indices.len() > 1 {
                                state.pop(selmon_idx, tiled_clients_indices[1]);
                            }
                        } else {
                            state.pop(selmon_idx, sel_idx);
                        }
                    }
                }
            }
            Action::ViewTag(ui, opt_mon_idx) => {
                let mon_idx = match opt_mon_idx {
                    Some(idx) => {
                        if *idx != state.selected_monitor {
                            let last_sel = state.mons[*idx].sel;
                            state.focus(*idx, last_sel);
                        }
                        *idx
                    }
                    None => state.selected_monitor,
                };

                let mon = &mut state.mons[mon_idx];
                if (*ui & TAG_MASK) != 0 {
                    mon.selected_tags = 0;
                    mon.tagset[mon.selected_tags as usize] = *ui & TAG_MASK;
                }
                state.arrange(Some(mon_idx));
            }
            Action::ViewPrevTag => {
                let selmon = &mut state.mons[state.selected_monitor];
                selmon.selected_tags = (selmon.selected_tags + 1) % 2;
                state.arrange(Some(state.selected_monitor));
            }
            Action::KillClient => {
                let selmon_idx = state.selected_monitor;
                if let Some(sel_idx) = state.mons[selmon_idx].sel {
                    let client_to_kill = state.mons[selmon_idx].clients[sel_idx].clone();
                    if !state.xwrapper.send_event(
                        client_to_kill.win,
                        state.xwrapper.atoms.get(crate::xwrapper::Atom::Wm(crate::xwrapper::WM::Delete)),
                    ) {
                        state.xwrapper.grab_server();
                        state.xwrapper.set_ignore_error_handler();
                        state.xwrapper.set_close_down_mode(xlib::DestroyAll);
                        state.xwrapper.kill_client(client_to_kill.win);
                        state.xwrapper.sync(false);
                        state.xwrapper.set_default_error_handler();
                        state.xwrapper.ungrab_server();
                    }
                }
            }
            Action::SetLayout(l) => {
                let selmon_idx = state.selected_monitor;
                let sellt = state.mons[selmon_idx].selected_lt as usize;
                state.mons[selmon_idx].lt[sellt] = l;
                let selmon = &mut state.mons[selmon_idx];
                let symbol = selmon.lt[selmon.selected_lt as usize].symbol;
                selmon.lt_symbol = symbol.to_string();
                if selmon.sel.is_some() {
                    state.arrange(Some(selmon_idx));
                }
            }
            Action::ToggleFloating => {
                let selmon_idx = state.selected_monitor;
                if let Some(sel_idx) = state.mons[selmon_idx].sel {
                    let client = &mut state.mons[selmon_idx].clients[sel_idx];
                    client.is_floating = !client.is_floating;
                    // arrange(selmon);
                }
            }
            Action::Tag(ui) => {
                let selmon_idx = state.selected_monitor;
                if let Some(sel_idx) = state.mons[selmon_idx].sel {
                    if (*ui & TAG_MASK) != 0 {
                        state.mons[selmon_idx].clients[sel_idx].tags = *ui & TAG_MASK;
                        state.arrange(Some(selmon_idx));
                    }
                }
            }
            Action::FocusMon(i) => {
                let next_mon_idx;
                if state.mons.len() <= 1 {
                    return;
                }
                if *i > 0 {
                    next_mon_idx = (state.selected_monitor + 1) % state.mons.len();
                } else {
                    next_mon_idx =
                        (state.selected_monitor + state.mons.len() - 1) % state.mons.len();
                }
                let last_sel = state.mons[next_mon_idx].sel;
                state.focus(next_mon_idx, last_sel);
            }
            Action::TagMon(i) => {
                let selmon_idx = state.selected_monitor;
                if let Some(_sel_idx) = state.mons[selmon_idx].sel {
                    let _next_mon_idx;
                    if state.mons.len() <= 1 {
                        return;
                    }
                    if *i > 0 {
                        _next_mon_idx = (state.selected_monitor + 1) % state.mons.len();
                    } else {
                        _next_mon_idx =
                            (state.selected_monitor + state.mons.len() - 1) % state.mons.len();
                    }
                    // TODO: send client to monitor
                }
            }
            Action::Quit => {
                state.running = 0;
            }
            Action::ToggleView(ui) => {
                let selmon = &mut state.mons[state.selected_monitor];
                let newtags = selmon.tagset[selmon.selected_tags as usize] ^ (*ui & TAG_MASK);

                if newtags != 0 {
                    selmon.tagset[selmon.selected_tags as usize] = newtags;
                    state.arrange(Some(state.selected_monitor));
                }
            }
            Action::ToggleTag(ui) => {
                let selmon_idx = state.selected_monitor;
                if let Some(sel_idx) = state.mons[selmon_idx].sel {
                    let newtags =
                        state.mons[selmon_idx].clients[sel_idx].tags ^ (*ui & TAG_MASK);
                    if newtags != 0 {
                        state.mons[selmon_idx].clients[sel_idx].tags = newtags;
                        state.arrange(Some(selmon_idx));
                    }
                }
            }
            Action::CycleTag(direction) => {
                let selmon_idx = state.selected_monitor;
                let mon = &mut state.mons[selmon_idx];
                let tagset = mon.tagset[mon.selected_tags as usize];

                let num_tags = config::TAGS.len() as i32;
                let new_tag_idx = if tagset == 0 {
                    if *direction > 0 { 0 } else { num_tags - 1 }
                } else if tagset.count_ones() == 1 {
                    let current_tag_idx = tagset.trailing_zeros() as i32;
                    (current_tag_idx + direction + num_tags) % num_tags
                } else {
                    return; // More than one tag selected, do nothing.
                };

                mon.tagset[mon.selected_tags as usize] = 1 << new_tag_idx;
                state.arrange(Some(selmon_idx));
            }
            Action::FocusClient(mon_idx, client_idx) => {
                state.focus(*mon_idx, Some(*client_idx));
                state.restack(state.selected_monitor);
                state.xwrapper.allow_events(xlib::ReplayPointer);
            }
        }
    }
}
