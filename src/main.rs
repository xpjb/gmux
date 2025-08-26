use std::time::{Duration, Instant};
use x11::xlib;
use colour::Colour;
use xwrapper::Window;
use state::{Client, Gmux};
use config::BORDER_PX;
use actions::Action;
use crate::bar::BarState;
use simplelog::*;
use std::fs::File;
use crate::error::GmuxError;
use std::process::Command;
use std::sync::mpsc::channel;
use std::thread;

mod ivec2;
mod xwrapper;
mod colour;
mod state;
mod config;
mod layouts;
mod actions;
mod events;
mod bar;
mod error;

const TAG_MASK: u32 = (1 << config::TAGS.len()) - 1;

#[derive(PartialEq, Copy, Clone)]
enum CursorType {
    Normal,
    Resize,
    Move,
    Last,
}

impl Gmux {
    fn spawn(&mut self, cmd: &str) {
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

    fn process_error(&mut self, error: GmuxError) {
        // Log it as a high-priority error
        log::error!("{}", error);
        // Display it on the bar
        self.set_error_state(error.to_string());
    }

    fn run(&mut self) {
        const BAR_UPDATE_INTERVAL: Duration = Duration::from_millis(100);
        let mut bar_acc = Duration::from_millis(0);
        let mut last_frame = Instant::now();
        self.xwrapper.sync(false);
        while self.running != 0 {
            let now = Instant::now();
            bar_acc += now.duration_since(last_frame);
            last_frame = now;

            // Reap any reported errors from child processes
            while let Ok(error) = self.command_receiver.try_recv() {
                self.process_error(error);
            }
            if bar_acc >= BAR_UPDATE_INTERVAL {
                self.update_bars();
                bar_acc -= BAR_UPDATE_INTERVAL;
            }

            // Process X11 events as normal
            if let Some(ev) = self.xwrapper.next_event() {
                match ev {
                    xwrapper::Event::KeyPress(kev) => {
                        if let BarState::Launcher { .. } = self.bar_state {
                            self.handle_launcher_keypress(&kev);
                        } else if let Some(action) = events::parse_key_press(self, &kev) {
                            match action {
                                Action::Spawn(cmd) => self.spawn(&cmd),
                                _ => action.execute(self),
                            }
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

    pub fn set_error_state(&mut self, error_msg: String) {
        self.bar_state = BarState::ErrorDisplay {
            message: error_msg,
            expiry: Instant::now() + Duration::from_secs(1),
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
    CombinedLogger::init(vec![
        WriteLogger::new(
            LevelFilter::Info,
            Config::default(),
            File::create("gmux.log").unwrap(),
        ),
    ]).unwrap();

    log::info!("Starting gmux...");
    let (tx, rx) = channel();
    match Gmux::new(tx, rx) {
        Ok(mut gmux) => {
            gmux.scan();
            gmux.run();
        }
        Err(e) => panic!("{}", e),
    }
}
