use crate::*;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use x11::{keysym, xlib};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
// Add the new dependencies
use freedesktop_desktop_entry::DesktopEntry;
use dirs;

pub const LAUNCHER_PROPORTION: f32 = 0.381953;

impl Gmux {
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
                    self.xwrapper.ungrab_keyboard();
                }
                keysym::XK_Return => {
                    if !candidate_indices.is_empty() {
                        let command_idx = candidate_indices[*selected_idx];
                        command_to_run = Some(self.all_commands[command_idx].clone());
                    }
                    new_state = Some(BarState::Normal);
                    self.xwrapper.ungrab_keyboard();
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
        self.xwrapper.grab_keyboard(self.root);
        self.draw_bars();
    }

    // This is the only function that needs to be changed.
    pub fn get_commands() -> Vec<String> {
        let mut commands = HashSet::new();

        // 1. Scan PATH for executables (same as before)
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

        // 2. Scan .desktop file directories and extract commands
        let mut desktop_dirs = vec![
            "/usr/share/applications".into(),
            "/usr/local/share/applications".into(),
        ];
        if let Some(mut home_dir) = dirs::home_dir() {
            home_dir.push(".local/share/applications");
            desktop_dirs.push(home_dir);
        }

        for dir in desktop_dirs {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("desktop") {
                        if let Ok(entry) = DesktopEntry::from_path::<&str>(&path, None) {
                            if entry.type_() == Some("Application") && !entry.no_display() {
                                if let Some(exec) = entry.exec() {
                                    // This logic finds the first part of the Exec string that isn't
                                    // an environment variable, which is usually the command itself.
                                    let mut command_to_add = None;
                                    for part in exec.split_whitespace() {
                                        if !part.contains('=') {
                                            // Take the filename if it's a path (e.g., /usr/bin/firefox -> firefox)
                                            let command = Path::new(part)
                                                .file_name()
                                                .and_then(|s| s.to_str())
                                                .unwrap_or(part);
                                            command_to_add = Some(command.to_string());
                                            break; // We found the command, stop searching.
                                        }
                                    }
                                    
                                    if let Some(cmd) = command_to_add {
                                        if !cmd.is_empty() {
                                            commands.insert(cmd);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // 3. Collect, sort, and return (same as before)
        let mut sorted_commands: Vec<_> = commands.into_iter().collect();
        sorted_commands.sort();
        sorted_commands
    }

    pub fn draw_launcher_bar(&mut self, mon_idx: usize) {
        let (prompt, input, candidate_indices, selected_idx) =
            if let BarState::Launcher { prompt, input, candidate_indices, selected_idx, .. } = &self.bar_state {
                (prompt.clone(), input.clone(), candidate_indices.clone(), *selected_idx)
            } else {
                return;
            };

        let mon = &mut self.mons[mon_idx];
        let bar_wh = ivec2(mon.ww, self.bar_height);

        self.xwrapper.rect(Colour::BarBackground, ivec2(0, 0), bar_wh, true);

        let prompt_text = format!("{}{}", prompt, input);
        let text_w = self.xwrapper.text_width(&prompt_text) + (BAR_H_PADDING * 2);
        let min_prompt_w = (LAUNCHER_PROPORTION * mon.ww as f32) as u32;
        let prompt_w = std::cmp::max(text_w, min_prompt_w);
        self.xwrapper.text(
            Colour::TextNormal,
            ivec2(0, 0),
            ivec2(prompt_w as _, self.bar_height),
            BAR_H_PADDING,
            &prompt_text,
        );

        let pos_x = prompt_w as i32;
        let available_width = mon.ww as i32 - pos_x;

        if candidate_indices.is_empty() {
            self.xwrapper.map_drawable(mon.bar_window, 0, 0, bar_wh.x as u32, bar_wh.y as u32);
            return;
        }

        let candidate_widths: Vec<i32> = candidate_indices
            .iter()
            .map(|&command_idx| {
                let candidate = &self.all_commands[command_idx];
                (self.xwrapper.text_width(candidate) + (BAR_H_PADDING * 2)) as i32
            })
            .collect();

        let total_width: i32 = candidate_widths.iter().sum();

        let mut offset = 0;
        if total_width > available_width {
            let width_before_selected: i32 = candidate_widths[..selected_idx].iter().sum();
            let selected_width = candidate_widths[selected_idx];
            let center_pos = available_width / 2;

            offset = width_before_selected - (center_pos - selected_width / 2);
            let max_offset = total_width - available_width;
            offset = offset.clamp(0, max_offset);
        }

        let mut current_x = 0;
        for i in 0..candidate_indices.len() {
            let w = candidate_widths[i];
            let draw_pos_x = pos_x + current_x - offset;

            if draw_pos_x + w > pos_x && draw_pos_x < mon.ww as i32 {
                let command_idx = candidate_indices[i];
                let candidate = &self.all_commands[command_idx];
                let wh = ivec2(w as _, self.bar_height);

                let (bg_col, fg_col) = if i == selected_idx {
                    (Colour::BarForeground, Colour::TextNormal)
                } else {
                    (Colour::BarBackground, Colour::TextQuiet)
                };

                self.xwrapper.rect(bg_col, ivec2(draw_pos_x, 0), wh, true);
                self.xwrapper.text(fg_col, ivec2(draw_pos_x, 0), wh, BAR_H_PADDING, candidate);
            }
            current_x += w;
        }

        self.xwrapper.map_drawable(mon.bar_window, 0, 0, bar_wh.x as u32, bar_wh.y as u32);
    }
}
