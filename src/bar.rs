use std::time::Instant;
use chrono::Local;
use crate::state::Gmux;
use crate::ivec2::ivec2;
use crate::colour::Colour;
use crate::config::BAR_H_PADDING;
use crate::actions::Action;
use crate::state;

#[derive(Clone)]
pub enum BarState {
    Normal,
    ErrorDisplay {
        message: String,
        expiry: Instant,
    },
    Launcher {
        prompt: String,
        input: String,
        candidate_indices: Vec<usize>,
        selected_idx: usize,
    },
}

impl Gmux {
    pub fn update_bars(&mut self) {
        if let BarState::ErrorDisplay { expiry, .. } = self.bar_state {
            if Instant::now() >= expiry {
                self.bar_state = BarState::Normal;
                self.draw_bars();
            }
        }
    }
    pub fn draw_bars(&mut self) {
        for i in 0..self.mons.len() {
            self.draw_bar(i);
        }
    }
    pub fn draw_bar(&mut self, mon_idx: usize) {
        let bar_state_clone = self.bar_state.clone();
        match bar_state_clone {
            BarState::Normal => self.draw_normal_bar(mon_idx),
            BarState::ErrorDisplay { message, .. } => {
                self.draw_error_bar(mon_idx, &message);
            }
            BarState::Launcher { .. } => self.draw_launcher_bar(mon_idx),
        }
    }
    pub fn draw_normal_bar(&mut self, mon_idx: usize) {
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
    let s = Local::now().format("%B %d %H:%M").to_string();
        let w_right = self.xwrapper.text_width(&s) + (BAR_H_PADDING * 2);
    let p_right = ivec2(bar_wh.x - w_right as i32, 0);
        let wh_right = ivec2(w_right as i32, self.bar_height);
        self.xwrapper.rect(Colour::BarBackground, p_right, wh_right, true);
        self.xwrapper.text(Colour::TextQuiet, p_right, wh_right, BAR_H_PADDING, &s);


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

    fn draw_launcher_bar(&mut self, mon_idx: usize) {
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
        let min_prompt_w = (0.381953 * mon.ww as f32) as u32;
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