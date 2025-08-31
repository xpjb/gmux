use x11::xlib;
use crate::*;

#[derive(Clone, Debug)]
pub enum Action {
    Spawn(String),
    SpawnDirect(String, Vec<String>),
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
    FocusClient(ClientHandle),
    EnterLauncherMode,
}

impl Action {
    pub fn execute(&self, state: &mut Gmux) {
        match self {
            Action::Spawn(cmd) => {
                state.spawn(cmd);
            }
            Action::SpawnDirect(program, args) => {
                state.spawn_direct(program, args);
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
                let sel_handle = if let Some(sel) = selmon.sel {
                    sel
                } else {
                    return;
                };

                let visible_clients: Vec<ClientHandle> = selmon.stack.iter()
                    .filter(|h| state.clients.get(h).map_or(false, |c| c.is_visible_on(selmon)))
                    .cloned()
                    .collect();

                if visible_clients.is_empty() {
                    return;
                }

                if let Some(pos) = visible_clients.iter().position(|h| *h == sel_handle) {
                    let new_pos = (pos as i32 + i + visible_clients.len() as i32) as usize % visible_clients.len();
                    state.focus(Some(visible_clients[new_pos]));
                } else {
                    state.focus(visible_clients.first().cloned());
                }
                state.restack(selmon_idx);
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
                if let Some(sel_handle) = state.mons[selmon_idx].sel {
                    if let Some(c) = state.clients.get(&sel_handle) {
                        if state.mons[selmon_idx].lt[state.mons[selmon_idx].selected_lt as usize]
                            .arrange
                            .is_none()
                            || c.is_floating
                        {
                            return;
                        }

                        let mon = &mut state.mons[selmon_idx];
                        if let Some(pos) = mon.stack.iter().position(|h| *h == sel_handle) {
                            mon.stack.remove(pos);
                            mon.stack.insert(0, sel_handle);
                            state.arrange(Some(selmon_idx));
                        }
                    }
                }
            }
            Action::ViewTag(ui, opt_mon_idx) => {
                let mon_idx = match opt_mon_idx {
                    Some(idx) => {
                        if *idx != state.selected_monitor {
                            let last_sel = state.mons[*idx].sel;
                            state.focus(last_sel);
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
                if let Some(sel_handle) = state.mons[selmon_idx].sel {
                    if let Some(client_to_kill) = state.clients.get(&sel_handle) {
                        if !state.xwrapper.send_wm_protocol_event(
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
                if let Some(sel_handle) = state.mons[selmon_idx].sel {
                    if let Some(client) = state.clients.get_mut(&sel_handle) {
                        client.is_floating = !client.is_floating;
                    }
                    state.arrange(Some(selmon_idx));
                }
            }
            Action::Tag(ui) => {
                let selmon_idx = state.selected_monitor;
                if let Some(sel_handle) = state.mons[selmon_idx].sel {
                    if (*ui & TAG_MASK) != 0 {
                        if let Some(client) = state.clients.get_mut(&sel_handle) {
                            client.tags = *ui & TAG_MASK;
                        }
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
                state.focus(last_sel);
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
                if let Some(sel_handle) = state.mons[selmon_idx].sel {
                    if let Some(client) = state.clients.get_mut(&sel_handle) {
                        let newtags = client.tags ^ (*ui & TAG_MASK);
                        if newtags != 0 {
                            client.tags = newtags;
                            state.arrange(Some(selmon_idx));
                        }
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
            Action::FocusClient(handle) => {
                state.focus(Some(*handle));
                state.restack(state.selected_monitor);
                state.xwrapper.allow_events(xlib::ReplayPointer);
            }
        }
    }
}

impl Gmux {
    pub fn spawn(&mut self, cmd: &str) {
        let sender = self.command_sender.clone();
        let command_string = cmd.to_string();

        thread::spawn(move || {
            let output_result = Command::new("sh")
                .arg("-c")
                .arg(&command_string)
                .output();

            let output = match output_result {
                Ok(o) => o,
                Err(e) => {
                    // The command failed to even start
                    let error = GmuxError::Subprocess {
                        command: command_string,
                        stderr: e.to_string(),
                    };
                    let _ = sender.send(error);
                    return;
                }
            };

            // 1. Always log stderr if it's not empty
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() {
                log::info!("stderr from '{}': {}", command_string, stderr.trim());
            }

            // 2. Only send an error to the bar on a non-zero exit code
            if !output.status.success() {
                let error = GmuxError::Subprocess {
                    command: command_string,
                    stderr: stderr.to_string(),
                };
                let _ = sender.send(error);
            }
        });
    }

    /// Spawns a command directly in a new thread to collect errors without blocking.
    pub fn spawn_direct(&mut self, program: &str, args: &[String]) {
        let sender = self.command_sender.clone();
        let program_string = program.to_string();
        let args_vec: Vec<String> = args.to_vec();
        let full_command_str = format!("{} {}", program_string, args_vec.join(" "));

        thread::spawn(move || {
            let output_result = Command::new(&program_string)
                .args(&args_vec)
                .output(); // This blocks the new thread, NOT the main event loop.

            let output = match output_result {
                Ok(o) => o,
                Err(e) => {
                    let error = GmuxError::Subprocess {
                        command: full_command_str,
                        stderr: e.to_string(),
                    };
                    let _ = sender.send(error);
                    return;
                }
            };

            // Only send an error back if the command had a non-zero exit code.
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                log::info!("stderr from '{}': {}", full_command_str, stderr.trim());
                let error = GmuxError::Subprocess {
                    command: full_command_str,
                    stderr,
                };
                let _ = sender.send(error);
            }
        });
    }
}