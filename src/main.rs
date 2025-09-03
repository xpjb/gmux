use std::time::{Duration, Instant};
use x11::xlib;
use std::fs::{File, OpenOptions};
use std::process::Command;
use std::sync::mpsc::channel;
use std::thread;
use simplelog::{CombinedLogger, WriteLogger, LevelFilter, Config};
use std::fs::create_dir_all;
use std::panic;
use std::io::Write;
use std::os::raw::{c_int, c_long, c_uchar, c_ulong};

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
mod client;
mod monitor;
mod launcher;

pub use ivec2::*;
pub use xwrapper::*;
pub use colour::*;
pub use state::*;
pub use config::*;
pub use layouts::*;
pub use actions::*;
pub use events::*;
pub use bar::*;
pub use error::*;
pub use client::*;
pub use monitor::*;
pub use launcher::*;

const TAG_MASK: u32 = (1 << config::TAGS.len()) - 1;

#[derive(PartialEq, Copy, Clone)]
enum CursorType {
    Normal,
    Resize,
    Move,
    Last,
}

impl Gmux {
    fn apply_rules(&self, client: &mut Client) {
        let rules = config::rules();
        
        // Get window properties
        let title = if client.name.is_empty() { None } else { Some(client.name.clone()) };
        let (instance, class) = if let Some((inst, cls)) = self.xwrapper.get_window_class(client.win) {
            (if inst.is_empty() { None } else { Some(inst) }, 
             if cls.is_empty() { None } else { Some(cls) })
        } else {
            (None, None)
        };
        
        // Find matching rule
        for rule in &rules {
            let class_matches = match (&rule.class, &class) {
                (Some(rule_class), Some(window_class)) => {
                    rule_class.to_lowercase() == window_class.to_lowercase()
                }
                (None, _) => true,
                (Some(_), None) => false,
            };
            
            let instance_matches = match (&rule.instance, &instance) {
                (Some(rule_instance), Some(window_instance)) => {
                    rule_instance.to_lowercase() == window_instance.to_lowercase()
                }
                (None, _) => true,
                (Some(_), None) => false,
            };
            
            let title_matches = match (&rule.title, &title) {
                (Some(rule_title), Some(window_title)) => {
                    window_title.to_lowercase().contains(&rule_title.to_lowercase())
                }
                (None, _) => true,
                (Some(_), None) => false,
            };
            
            if class_matches && instance_matches && title_matches {
                // Apply the rule
                client.tags = rule.tags;
                client.is_floating = rule.is_floating;
                
                // Apply monitor assignment if specified
                if rule.monitor >= 0 && (rule.monitor as usize) < self.mons.len() {
                    client.monitor_idx = rule.monitor as usize;
                } else {
                    client.monitor_idx = self.selected_monitor;
                }
                
                log::info!("Applied rule to window '{}' (class: {:?}, instance: {:?}): tags={:b}, floating={}, monitor={}", 
                          client.name, class, instance, client.tags, client.is_floating, client.monitor_idx);
                break; // Use first matching rule
            }
        }
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
                            action.execute(self)
                        }
                    }
                    xwrapper::Event::ButtonPress(mut bev) => unsafe { events::button_press(self, &mut bev) },
                    xwrapper::Event::MotionNotify(mut mev) => unsafe { events::motion_notify(self, &mut mev) },
                    xwrapper::Event::MapRequest(mut mrev) => unsafe { events::map_request(self, &mut mrev) },
                    xwrapper::Event::DestroyNotify(mut drev) => unsafe { events::destroy_notify(self, &mut drev) },
                    xwrapper::Event::EnterNotify(mut erev) => unsafe { events::enter_notify(self, &mut erev) },
                    xwrapper::Event::PropertyNotify(mut prev) => unsafe { events::property_notify(self, &mut prev) },

                    // ADDED: Handle screen configuration changes (e.g., wake from sleep)
                    xwrapper::Event::ConfigureNotify(mut cev) => unsafe { events::configure_notify(self, &mut cev) },
                    // ADDED: Handle requests to re-draw a window
                    xwrapper::Event::Expose(mut eev) => unsafe { events::expose(self, &mut eev) },
                    // ADDED: Handle requests from windows to configure themselves
                    xwrapper::Event::ConfigureRequest(mut crev) => unsafe { events::configure_request(self, &mut crev) },
                    // ADDED: Handle ClientMessage events for NetActiveWindow
                    xwrapper::Event::ClientMessage(mut cmev) => unsafe { events::client_message(self, &mut cmev) },

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
    fn client_at_pos(&self, x: i32, y: i32) -> Option<ClientHandle> {
        for (handle, c) in &self.clients {
            let m = &self.mons[c.monitor_idx];
            if c.is_visible_on(m) && (x >= c.x && x < c.x + c.w && y >= c.y && y < c.y + c.h) {
                return Some(*handle);
            }
        }
        None
    }

    fn show_hide(&mut self, mon_idx: usize, stack: &[ClientHandle]) {
        for &handle in stack.iter().rev() {
            if let Some(c) = self.clients.get(&handle).cloned() {
                if c.is_visible_on(&self.mons[mon_idx]) {
                    self.xwrapper.move_window(c.win, c.x, c.y);
                    let client_mon = &self.mons[c.monitor_idx];
                    if client_mon.lt[client_mon.selected_lt as usize].arrange.is_none()
                        || c.is_floating && !c.is_fullscreen
                    {
                        self.resize(handle, c.x, c.y, c.w, c.h);
                    }
                }
            }
        }

        for &handle in stack {
            if let Some(c) = self.clients.get(&handle) {
                if !c.is_visible_on(&self.mons[mon_idx]) {
                    self.xwrapper.move_window(c.win, -2 * c.width(), c.y);
                }
            }
        }
    }


    fn unmanage(&mut self, handle: ClientHandle, destroyed: bool) {
        let mon_idx = if let Some(client) = self.clients.get(&handle) {
            client.monitor_idx
        } else {
            return;
        };

        if !destroyed {
            if let Some(client) = self.clients.get(&handle) {
                self.xwrapper.unmanage_window(client.win);
            }
        }
        self.clients.remove(&handle);

        let mon = &mut self.mons[mon_idx];
        mon.stack.retain(|&h| h != handle);

        let new_sel = if mon.sel == Some(handle) {
            mon.stack.first().cloned()
        } else {
            mon.sel
        };
        mon.sel = new_sel;

        self.focus(new_sel);
        self.arrange(Some(mon_idx));
    }


    unsafe fn manage(&mut self, w: xlib::Window, wa: &mut xlib::XWindowAttributes) {
        let handle = ClientHandle::from(Window(w));
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
                if let Some(parent_handle) = self.window_to_client_handle(parent_win.0) {
                    let parent_client = self.clients.get(&parent_handle).unwrap();
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
            // First set default tags and monitor
            client.tags = self.mons[self.selected_monitor].tagset[self.mons[self.selected_monitor].selected_tags as usize];
            client.monitor_idx = self.selected_monitor;
            
            // Then apply rules which may override the defaults
            self.apply_rules(&mut client);
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

        let mon_idx = client.monitor_idx;
        self.clients.insert(handle, client);
        self.mons[mon_idx].stack.insert(0, handle);

        // Check for existing window state properties (like fullscreen)
        self.update_window_state_properties(handle);

        self.arrange(Some(self.selected_monitor));
        if let Some(sel_client) = self.clients.get(&handle) {
            self.xwrapper.select_input(
                sel_client.win,
                xlib::EnterWindowMask | xlib::FocusChangeMask | xlib::PropertyChangeMask,
            );

            self.xwrapper.map_window(sel_client.win);
        }
        
        // Only focus the new client if it's visible on the current tags
        // Otherwise, it should be marked as urgent when it requests focus later
        if let Some(client) = self.clients.get(&handle) {
            let current_tags = self.mons[self.selected_monitor].tagset[self.mons[self.selected_monitor].selected_tags as usize];
            if (client.tags & current_tags) != 0 {
                self.focus(Some(handle));
            }
        }
    }


    /// Check and update window state properties like fullscreen (based on dwm's updatesizehints)
    fn update_window_state_properties(&mut self, handle: ClientHandle) {
        if let Some(client) = self.clients.get(&handle) {
            let net_wm_state = self.xwrapper.atoms.get(crate::xwrapper::Atom::Net(crate::xwrapper::Net::WMState));
            let net_wm_fullscreen = self.xwrapper.atoms.get(crate::xwrapper::Atom::Net(crate::xwrapper::Net::WMFullscreen));
            
            // Check if window has _NET_WM_STATE_FULLSCREEN set
            unsafe {
                let mut actual_type: xlib::Atom = 0;
                let mut actual_format: c_int = 0;
                let mut nitems: c_ulong = 0;
                let mut bytes_after: c_ulong = 0;
                let mut prop: *mut c_uchar = std::ptr::null_mut();
                
                let result = xlib::XGetWindowProperty(
                    self.xwrapper.display(),
                    client.win.0,
                    net_wm_state,
                    0,
                    c_long::MAX,
                    0, // delete = False
                    xlib::XA_ATOM,
                    &mut actual_type,
                    &mut actual_format,
                    &mut nitems,
                    &mut bytes_after,
                    &mut prop,
                );
                
                if result == xlib::Success as i32 && !prop.is_null() && nitems > 0 {
                    let atoms = std::slice::from_raw_parts(prop as *const xlib::Atom, nitems as usize);
                    for &atom in atoms {
                        if atom == net_wm_fullscreen {
                            log::info!("Window already has fullscreen state, applying fullscreen");
                            // Found fullscreen atom - this window is already fullscreen
                            self.setfullscreen(handle, true);
                            break;
                        }
                    }
                    xlib::XFree(prop as *mut _);
                }
            }
        }
    }

    fn window_to_client_handle(&self, w: xlib::Window) -> Option<ClientHandle> {
        let handle = ClientHandle::from(Window(w));
        if self.clients.contains_key(&handle) {
            Some(handle)
        } else {
            None
        }
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

/// Sets up a panic hook that writes panic information to the log file
fn setup_panic_hook() {
    let log_path = LOG_PATH.clone();
    panic::set_hook(Box::new(move |panic_info| {
        let panic_msg = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            format!("panic occurred: {}", s)
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            format!("panic occurred: {}", s)
        } else {
            "panic occurred: unknown payload".to_string()
        };

        let location = if let Some(location) = panic_info.location() {
            format!(" at {}:{}:{}", location.file(), location.line(), location.column())
        } else {
            " at unknown location".to_string()
        };

        let full_msg = format!("PANIC: {}{}\n", panic_msg, location);
        
        // Try to write to stderr first (may work even if logging is broken)
        eprintln!("{}", full_msg.trim());
        
        // Try to append to log file
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path) 
        {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let _ = writeln!(file, "[{}] [ERROR] {}", timestamp, full_msg.trim());
            let _ = file.flush();
        }
    }));
}

/// Sets up the logger to write to a standard user-specific data directory.
///
/// Returns the path to the log file for reference.
fn setup_logger() {
    // 1. Determine the log file path
    let log_path = &*LOG_PATH;
    let data_path = &*DATA_PATH;
    if let Err(e) = create_dir_all(data_path) {
        eprintln!("[ERROR] Failed to create log directory: {}", e);
    }
    // 2. Initialize the logger
    CombinedLogger::init(vec![
        WriteLogger::new(
            LevelFilter::Info,
            Config::default(),
            File::create(log_path)
                .unwrap_or_else(|e| panic!("Failed to create log file at {:?}: {}", log_path, e)),
        ),
    ]).expect("Failed to initialize logger");

}

fn main() {
    setup_logger();
    setup_panic_hook();

    log::info!("Starting gmux...");
    log::info!("Log file is located at: {:?}", &*LOG_PATH);
    log::info!("Panic handler installed - panics will be logged");

    let (tx, rx) = channel();
    match Gmux::new(tx, rx) {
        Ok(mut gmux) => {
            gmux.scan();
            gmux.run();
        }
        Err(e) => panic!("{}", e),
    }
}
