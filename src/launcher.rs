use crate::*;
use std::fs;
// Add SystemTime for checking modification timestamps
use std::time::SystemTime; 
use x11::{keysym, xlib};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use freedesktop_desktop_entry::DesktopEntry;
use dirs;
use log;

#[derive(Debug, Clone)]
pub struct LauncherEntry {
    pub name: String,        // Display name from Name= field
    pub exec: String,        // Full Exec= command  
    pub terminal: bool,      // Whether it's a terminal app
}

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
                        command_to_run = Some(self.all_commands[command_idx].exec.clone());
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
                    .filter_map(|(i, entry)| matcher.fuzzy_match(&entry.name, input).map(|score| (score, i)))
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
        // Refresh commands to pick up newly installed packages
        self.all_commands = Gmux::get_commands();
        log::info!("Launcher entered: {} commands loaded", self.all_commands.len());
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

    // This function is now refactored to use a cache and focus on .desktop files.
    pub fn get_commands() -> Vec<LauncherEntry> {
        log::info!("get_commands() called - loading launcher entries");
        // 1. Define cache path and identify all source directories.
        let cache_dir = dirs::cache_dir().map(|p| p.join("gmux"));
        let cache_path = cache_dir.as_ref().map(|p| p.join("commands"));

        let mut source_dirs = vec![
            "/usr/share/applications".into(),
            "/usr/local/share/applications".into(),
        ];
        if let Some(mut home_dir) = dirs::home_dir() {
            home_dir.push(".local/share/applications");
            source_dirs.push(home_dir);
        }

        // 2. Check if the cache is valid by comparing modification times.
        if let Some(ref path) = cache_path {
            if path.exists() {
                if let Ok(cached_metadata) = fs::metadata(path) {
                    let cache_mtime = cached_metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    
                    // Check if any source directory is newer than cache (mimics stest -dqr -n)
                    let mut cache_is_valid = true;
                    for dir in &source_dirs {
                        if let Ok(dir_metadata) = fs::metadata(dir) {
                            let dir_mtime = dir_metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                            if dir_mtime > cache_mtime {
                                cache_is_valid = false;
                                break;
                            }
                        }
                    }

                    if cache_is_valid {
                        if let Ok(cached_data) = fs::read_to_string(path) {
                            let mut entries = Vec::new();
                            for line in cached_data.lines() {
                                let parts: Vec<&str> = line.split('\t').collect();
                                if parts.len() == 3 {
                                    entries.push(LauncherEntry {
                                        name: parts[0].to_string(),
                                        exec: parts[1].to_string(),
                                        terminal: parts[2] == "true",
                                    });
                                }
                            }
                            return entries;
                        }
                    }
                }
            }
        }
        
        // 3. CACHE MISS: Regenerate the command list from .desktop files.
        let mut entries = Vec::new();
        let mut total_desktop_files = 0;
        let mut parsed_files = 0;
        let mut filtered_out = 0;

        for dir in &source_dirs {
            if let Ok(dir_entries) = fs::read_dir(dir) {
                for entry in dir_entries.flatten() {
                    let path = entry.path();
                    
                    // Only process .desktop files
                    if path.extension().and_then(|s| s.to_str()) == Some("desktop") {
                        total_desktop_files += 1;
                        if let Ok(desktop_entry) = DesktopEntry::from_path::<&str>(&path, None) {
                            parsed_files += 1;
                            
                            let is_app = desktop_entry.type_() == Some("Application");
                            let is_hidden = desktop_entry.no_display();
                            let is_terminal = desktop_entry.terminal();
                            
                            // Filter for GUI applications only
                            if is_app && !is_hidden && !is_terminal {
                                if let Some(name) = desktop_entry.name(&[] as &[&str]) {
                                    if let Some(exec) = desktop_entry.exec() {
                                        entries.push(LauncherEntry {
                                            name: name.to_string(),
                                            exec: exec.to_string(),
                                            terminal: desktop_entry.terminal(),
                                        });
                                    }
                                }
                            } else {
                                filtered_out += 1;
                            }
                        }
                    }
                }
            }
        }
        
        log::info!("Desktop files found: {}, parsed: {}, filtered out: {}, final entries: {}", 
            total_desktop_files, parsed_files, filtered_out, entries.len());

        entries.sort_by(|a, b| a.name.cmp(&b.name));

        // 4. Write the new data to the cache file.
        if let (Some(dir), Some(path)) = (cache_dir, cache_path) {
            if fs::create_dir_all(dir).is_ok() {
                let cache_lines: Vec<String> = entries.iter()
                    .map(|e| format!("{}\t{}\t{}", e.name, e.exec, e.terminal))
                    .collect();
                let cache_data = cache_lines.join("\n");
                let _ = fs::write(path, cache_data);
            }
        }
        
        entries
    }

    pub fn draw_launcher_bar(&mut self, mon_idx: usize) {
        let (prompt, input, candidate_indices, selected_idx) =
            if let BarState::Launcher { prompt, input, candidate_indices, selected_idx, .. } = &self.bar_state {
                (prompt.clone(), input.clone(), candidate_indices.clone(), *selected_idx)
            } else {
                return;
            };


        let bar_wh = ivec2(self.mons[mon_idx].ww, self.bar_height);

        self.xwrapper.rect(Colour::BarBackground, ivec2(0, 0), bar_wh, true);

        let prompt_text = format!("{}{}", prompt, input);
        let text_w = self.get_text_width(&prompt_text);
        let min_prompt_w = (LAUNCHER_PROPORTION * self.mons[mon_idx].ww as f32) as u32;
        let prompt_w = std::cmp::max(text_w, min_prompt_w);
        self.xwrapper.text(
            Colour::TextNormal,
            ivec2(0, 0),
            ivec2(prompt_w as _, self.bar_height),
            self.lr_padding/2,
            &prompt_text,
        );

        let pos_x = prompt_w as i32;
        let available_width = self.mons[mon_idx].ww as i32 - pos_x;

        if candidate_indices.is_empty() {
            self.xwrapper.map_drawable(self.mons[mon_idx].bar_window, 0, 0, bar_wh.x as u32, bar_wh.y as u32);
            return;
        }

        let candidate_widths: Vec<i32> = candidate_indices
            .iter()
            .map(|&command_idx| {
                let candidate = &self.all_commands[command_idx].name;
                (self.get_text_width(candidate)) as i32
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

            if draw_pos_x + w > pos_x && draw_pos_x < self.mons[mon_idx].ww as i32 {
                let command_idx = candidate_indices[i];
                let candidate = &self.all_commands[command_idx].name;
                let wh = ivec2(w as _, self.bar_height);

                let (bg_col, fg_col) = if i == selected_idx {
                    (Colour::BarForeground, Colour::TextNormal)
                } else {
                    (Colour::BarBackground, Colour::TextQuiet)
                };

                self.xwrapper.rect(bg_col, ivec2(draw_pos_x, 0), wh, true);
                self.xwrapper.text(fg_col, ivec2(draw_pos_x, 0), wh, self.lr_padding/2, candidate);
            }
            current_x += w;
        }

        self.xwrapper.map_drawable(self.mons[mon_idx].bar_window, 0, 0, bar_wh.x as u32, bar_wh.y as u32);
    }
}
