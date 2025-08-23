use std::os::raw::{c_int, c_uint};
use std::ffi::CString;
use std::ptr::null_mut;
use x11::xlib;
use x11::keysym;

#[derive(Debug)]
pub enum XError {
    DisplayOpen,
    AtomIntern(String),
}

// Newtype wrapper for Window
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window(pub xlib::Window);

impl Default for Window {
    fn default() -> Self {
        Window(0)
    }
}

pub struct XWrapper {
    dpy: *mut xlib::Display,
}

impl XWrapper {
    pub fn connect() -> Result<Self, XError> {
        unsafe {
            // Locale setting should be handled by the application's main function
            // as it affects the entire process.
            let dpy = xlib::XOpenDisplay(null_mut());
            if dpy.is_null() {
                Err(XError::DisplayOpen)
            } else {
                Ok(Self { dpy })
            }
        }
    }

    /// Provides temporary access to the raw display pointer.
    /// This should be phased out as more functionality is moved into the wrapper.
    pub fn dpy(&self) -> *mut xlib::Display {
        self.dpy
    }

    pub fn intern_atom(&self, atom_name: &str) -> Result<xlib::Atom, XError> {
        let c_str = CString::new(atom_name)
            .map_err(|_| XError::AtomIntern(atom_name.to_string()))?;
        unsafe { Ok(xlib::XInternAtom(self.dpy, c_str.as_ptr(), 0)) }
    }

    pub fn set_error_handler(
        &self,
        handler: Option<unsafe extern "C" fn(*mut xlib::Display, *mut xlib::XErrorEvent) -> c_int>,
    ) {
        unsafe {
            xlib::XSetErrorHandler(handler);
        }
    }
    
    pub fn default_screen(&self) -> i32 {
        unsafe { xlib::XDefaultScreen(self.dpy) }
    }

    pub fn root_window(&self, screen_num: i32) -> Window {
        unsafe { Window(xlib::XRootWindow(self.dpy, screen_num)) }
    }
    
    pub fn display_width(&self, screen_num: i32) -> i32 {
        unsafe { xlib::XDisplayWidth(self.dpy, screen_num) }
    }
    
    pub fn display_height(&self, screen_num: i32) -> i32 {
        unsafe { xlib::XDisplayHeight(self.dpy, screen_num) }
    }

    pub fn create_simple_window(
        &self,
        parent: Window,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        border_width: u32,
        border: u64,
        background: u64,
    ) -> Window {
        unsafe {
            Window(xlib::XCreateSimpleWindow(
                self.dpy,
                parent.0,
                x,
                y,
                width,
                height,
                border_width,
                border,
                background,
            ))
        }
    }

    pub fn change_window_attributes(
        &self,
        win: Window,
        value_mask: u64,
        attributes: &mut xlib::XSetWindowAttributes,
    ) {
        unsafe {
            xlib::XChangeWindowAttributes(self.dpy, win.0, value_mask, attributes);
        }
    }

    pub fn create_font_cursor(&self, shape: u32) -> xlib::Cursor {
        unsafe { xlib::XCreateFontCursor(self.dpy, shape) }
    }

    pub fn default_depth(&self, screen_num: i32) -> c_int {
        unsafe { xlib::XDefaultDepth(self.dpy, screen_num) }
    }

    pub fn default_visual(&self, screen_num: i32) -> *mut xlib::Visual {
        unsafe { xlib::XDefaultVisual(self.dpy, screen_num) }
    }

    pub fn map_raised(&self, win: Window) {
        unsafe { xlib::XMapRaised(self.dpy, win.0) };
    }

    pub fn change_property(
        &self,
        win: Window,
        property: xlib::Atom,
        type_: xlib::Atom,
        format: i32,
        mode: i32,
        data: *const u8,
        nelements: i32,
    ) {
        unsafe {
            xlib::XChangeProperty(
                self.dpy,
                win.0,
                property,
                type_,
                format,
                mode,
                data,
                nelements,
            );
        }
    }

    pub fn delete_property(&self, win: Window, property: xlib::Atom) {
        unsafe {
            xlib::XDeleteProperty(self.dpy, win.0, property);
        }
    }

    pub fn select_input(&self, win: Window, mask: i64) {
        unsafe {
            xlib::XSelectInput(self.dpy, win.0, mask);
        }
    }

    pub fn get_numlock_mask(&self) -> u32 {
        unsafe {
            let mut numlockmask = 0;
            let modmap = xlib::XGetModifierMapping(self.dpy);
            if modmap.is_null() {
                return 0;
            }

            let max_keypermod = (*modmap).max_keypermod;
            let mut p = (*modmap).modifiermap;

            for i in 0..8 {
                for _j in 0..max_keypermod {
                    if *p != 0 {
                        if xlib::XKeycodeToKeysym(self.dpy, *p, 0) as u32 == keysym::XK_Num_Lock {
                            numlockmask = 1 << i;
                        }
                    }
                    p = p.offset(1);
                }
            }

            xlib::XFreeModifiermap(modmap);
            numlockmask as u32
        }
    }
}

impl Drop for XWrapper {
    fn drop(&mut self) {
        unsafe {
            xlib::XCloseDisplay(self.dpy);
        }
    }
}
