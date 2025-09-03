use std::time::Instant;
use chrono::Local;
use crate::*;

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
        self.mons[mon_idx].clickables.clear();

        let bar_wh = ivec2(self.mons[mon_idx].ww, self.bar_height);
        let barwin = self.mons[mon_idx].bar_window;
        let mut pos = ivec2(0, 0);
        let box_wh = ivec2(self.bar_height / 6 + 2, self.bar_height / 6 + 2);
        let box_xy = ivec2(self.bar_height / 9, self.bar_height / 9);

        // --- 1. Clear the entire bar with the default background ---
        self.xwrapper.rect(Colour::BarBackground, pos, bar_wh, true);

        // --- 2. Render Left-aligned elements ---
        // Calculate occupied and urgent tags
        let mut occ: u32 = 0;
        let mut urg: u32 = 0;
        for c in self.clients.values() {
            if c.monitor_idx == mon_idx {
                occ |= c.tags;
                if c.is_urgent {
                    urg |= c.tags;
                }
            }
        }

        // Draw tags
        for i in 0..self.tags.len() {
            let tag = self.tags[i];
            let selected = (self.mons[mon_idx].tagset[self.mons[mon_idx].selected_tags as usize] & 1 << i) != 0;
            let is_urgent = (urg & (1 << i)) != 0;
            
            // --- MODIFIED: Use the new helper function ---
            let w = self.get_text_width(tag);
        
            let (bg_col, fg_col) = if is_urgent {
                // Urgent tags get urgent color scheme regardless of selection
                (Colour::Urgent, Colour::TextNormal)
            } else if selected {
                (Colour::BarForeground, Colour::TextNormal)
            } else {
                (Colour::BarBackground, Colour::TextQuiet)
            };

            let tag_wh = ivec2(w as _, self.bar_height);
            self.xwrapper.rect(bg_col, pos, tag_wh, true);
            
            // Draw indicator for occupied tags
            if (occ & (1 << i)) != 0 {
                let is_filled = self.mons[mon_idx].sel.map_or(false, |sel_handle| {
                    self.clients.get(&sel_handle).map_or(false, |c| (c.tags & (1 << i)) != 0)
                });
                self.xwrapper.rect(fg_col, pos + box_xy, box_wh, is_filled);
            }

            // --- MODIFIED: Use lr_padding/2 for the text offset ---
            self.xwrapper.text(fg_col, pos, tag_wh, self.lr_padding / 2, tag);

            let action = Action::ViewTag(1 << i, Some(mon_idx));
            self.mons[mon_idx].clickables.push(Clickable{pos, size: tag_wh, action});
            pos.x += w as i32;
        }

        // --- 3. Render Right-aligned elements (Status Text) ---
        let s = Local::now().format("%B %d %H:%M").to_string();
        // --- MODIFIED: Use the new helper function ---
        let w_right = self.get_text_width(&s);
        let p_right = ivec2(bar_wh.x - w_right as i32, 0);
        let wh_right = ivec2(w_right as i32, self.bar_height);
        self.xwrapper.rect(Colour::BarBackground, p_right, wh_right, true);
        // --- MODIFIED: Use lr_padding/2 for the text offset ---
        self.xwrapper.text(Colour::TextQuiet, p_right, wh_right, self.lr_padding / 2, &s);

        // --- 4. Render Centered elements (Window Title) ---
        let wh_center = (bar_wh - pos) - wh_right.proj_x();
        let s = self.mons[mon_idx].sel.and_then(|h| self.clients.get(&h).map(|c| c.name.as_str()));
        let (col, text_to_draw) = if let Some(name) = s {
            // Clip the window title to fit within the available center area
            let clipped_name = self.clip_text_to_width(name, wh_center.x);
            (Colour::BarForeground, clipped_name)
        } else {
            (Colour::BarBackground, String::new())
        };

        self.xwrapper.rect(col, pos, wh_center, true);
        // --- MODIFIED: Use lr_padding/2 for the text offset ---
        self.xwrapper.text(Colour::TextNormal, pos, wh_center, self.lr_padding / 2, &text_to_draw);

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
        // --- MODIFIED: Use lr_padding/2 for the text offset ---
        self.xwrapper.text(Colour::TextNormal, ivec2(0, 0), bar_wh, self.lr_padding / 2, message);

        // 3. Map to screen
        self.xwrapper.map_drawable(barwin, 0, 0, bar_wh.x as u32, bar_wh.y as u32);
    }
}
