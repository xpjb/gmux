use std::ffi::CString;
use std::os::raw::{c_int, c_uint};
use std::ptr::null_mut;
use x11::{keysym, xft, xlib};

use crate::die;
use fontconfig::{self};

struct Drw {
    pub w: c_uint,
    pub h: c_uint,
    pub dpy: *mut xlib::Display,
    pub screen: c_int,
    pub root: xlib::Window,
    pub drawable: xlib::Drawable,
    pub gc: xlib::GC,
    pub scheme: *mut Clr,
    pub fonts: Vec<Fnt>,
}

struct Fnt {
    pub dpy: *mut xlib::Display,
    pub h: c_uint,
    pub xfont: *mut xft::XftFont,
}

impl Drop for Fnt {
    fn drop(&mut self) {
        unsafe {
            if !self.xfont.is_null() {
                xft::XftFontClose(self.dpy, self.xfont);
            }
        }
    }
}

type Clr = xft::XftColor;

#[derive(PartialEq, Copy, Clone)]
enum Scheme {
    Norm,
    Sel,
    Urg,
}

#[derive(Debug, Clone, Copy)]
pub struct SchemeId(pub usize);

impl Drw {
    /// Return pixel width of UTF-8 text using the first font in the set.
    pub fn text_width(&self, text: &str) -> u32 {
        if self.fonts.is_empty() {
            return 0;
        }
        unsafe {
            let mut ext = std::mem::zeroed();
            let font = &self.fonts[0];
            xft::XftTextExtentsUtf8(
                self.dpy,
                font.xfont,
                text.as_ptr() as *const u8,
                text.len() as i32,
                &mut ext,
            );
            ext.xOff as u32
        }
    }

    pub fn create(
        dpy: *mut xlib::Display,
        screen: c_int,
        root: xlib::Window,
        w: c_uint,
        h: c_uint,
    ) -> Self {
        unsafe {
            let drawable =
                xlib::XCreatePixmap(dpy, root, w, h, xlib::XDefaultDepth(dpy, screen) as u32);
            let gc = xlib::XCreateGC(dpy, root, 0, null_mut());
            xlib::XSetLineAttributes(dpy, gc, 1, xlib::LineSolid, xlib::CapButt, xlib::JoinMiter);
            Drw {
                w,
                h,
                dpy,
                screen,
                root,
                drawable,
                gc,
                scheme: null_mut(),
                fonts: Vec::new(),
            }
        }
    }

    pub fn free(&mut self) {
        unsafe {
            xlib::XFreePixmap(self.dpy, self.drawable);
            xlib::XFreeGC(self.dpy, self.gc);
            self.fontset_free();
        }
    }

    pub fn fontset_create(&mut self, font_names: &[&str]) -> bool {
        let mut success = true;
        for font_name in font_names {
            if !self.xfont_create(font_name) {
                success = false;
            }
        }
        success
    }

    fn xfont_create(&mut self, font_name: &str) -> bool {
        unsafe {
            let _fc_handle = fontconfig::Fontconfig::new();

            let cstr = match CString::new(font_name) {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("error, invalid font name '{}': contains NUL", font_name);
                    return false;
                }
            };

            let xfont = xft::XftFontOpenName(self.dpy, self.screen, cstr.as_ptr());
            if xfont.is_null() {
                eprintln!("error, cannot load font from name: '{}'", font_name);
                return false;
            }

            let h = ((*xfont).ascent + (*xfont).descent) as c_uint;
            let fnt = Fnt {
                dpy: self.dpy,
                h,
                xfont,
            };
            self.fonts.push(fnt);
            true
        }
    }

    pub fn fontset_free(&mut self) {
        self.fonts.clear();
    }

    pub fn scm_create(&mut self, clr_names: &[&str]) -> *mut Clr {
        let mut clrs = Vec::with_capacity(clr_names.len());
        for clr_name in clr_names {
            let mut clr = unsafe { std::mem::zeroed() };
            let cstr = CString::new(*clr_name).expect("color name contains NUL");
            unsafe {
                if xft::XftColorAllocName(
                    self.dpy,
                    xlib::XDefaultVisual(self.dpy, self.screen),
                    xlib::XDefaultColormap(self.dpy, self.screen),
                    cstr.as_ptr(),
                    &mut clr,
                ) == 0
                {
                    die("cannot allocate color");
                }
            }
            clrs.push(clr);
        }
        Box::into_raw(clrs.into_boxed_slice()) as *mut Clr
    }

    pub fn font_getexts(&self, font: *mut Fnt, text: &str, len: u32, w: &mut u32, h: &mut u32) {
        unsafe {
            let mut ext = std::mem::zeroed();
            xft::XftTextExtentsUtf8(
                self.dpy,
                (*font).xfont,
                text.as_ptr() as *const u8,
                len as i32,
                &mut ext,
            );
            *w = ext.xOff as u32;
            *h = ((*(*font).xfont).ascent + (*(*font).xfont).descent) as u32;
        }
    }

    pub fn text(
        &self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        lpad: u32,
        text: &str,
        _invert: bool,
    ) -> i32 {
        if self.scheme.is_null() || self.fonts.is_empty() {
            return 0;
        }
        unsafe {
            let d = xft::XftDrawCreate(
                self.dpy,
                self.drawable,
                xlib::XDefaultVisual(self.dpy, self.screen),
                xlib::XDefaultColormap(self.dpy, self.screen),
            );
            let usedfont = &self.fonts[0];
            let x = x + lpad as i32;
            let w = w.saturating_sub(lpad);

            xft::XftDrawStringUtf8(
                d,
                &mut (*self.scheme.add(Scheme::Sel as usize)),
                usedfont.xfont,
                x,
                y + (h as i32 - ((*usedfont.xfont).ascent + (*usedfont.xfont).descent) as i32) / 2
                    + (*usedfont.xfont).ascent as i32,
                text.as_ptr() as *const u8,
                text.len() as i32,
            );

            xft::XftDrawDestroy(d);
            x + w as i32
        }
    }

    pub fn rect(&self, _x: i32, _y: i32, _w: u32, _h: u32, _filled: bool, _invert: bool) {
        if self.scheme.is_null() {
            return;
        }
        unsafe {
            xlib::XSetForeground(
                self.dpy,
                self.gc,
                if _invert {
                    (*self.scheme.add(Scheme::Norm as usize)).pixel
                } else {
                    (*self.scheme.add(Scheme::Sel as usize)).pixel
                },
            );
            if _filled {
                xlib::XFillRectangle(self.dpy, self.drawable, self.gc, _x, _y, _w, _h);
            } else {
                xlib::XDrawRectangle(self.dpy, self.drawable, self.gc, _x, _y, _w - 1, _h - 1);
            }
        }
    }

    pub fn map(&self, win: xlib::Window, x: i32, y: i32, w: u32, h: u32) {
        unsafe {
            xlib::XCopyArea(self.dpy, self.drawable, win, self.gc, x, y, w, h, x, y);
            xlib::XSync(self.dpy, 0);
        }
    }
}

// Newtype wrapper for Window
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window(pub xlib::Window);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorId(pub u64);

impl Default for Window {
    fn default() -> Self {
        Window(0)
    }
}

pub struct KeySpec {
    pub mask: u32,
    pub keysym: u32,
}

pub struct XWrapper {
    dpy: *mut xlib::Display,
    drw: Drw,
    schemes: Vec<*mut Clr>,
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
                let screen = unsafe { xlib::XDefaultScreen(dpy) };
                let root = unsafe { xlib::XRootWindow(dpy, screen) };
                let w = unsafe { xlib::XDisplayWidth(dpy, screen) as u32 };
                let h = unsafe { xlib::XDisplayHeight(dpy, screen) as u32 };
                let drw = Drw::create(dpy, screen, root, w, h);
                Ok(Self {
                    dpy,
                    drw,
                    schemes: Vec::new(),
                })
            }
        }
    }

    /// Provides temporary access to the raw display pointer.
    /// This should be phased out as more functionality is moved into the wrapper.
    pub fn dpy(&self) -> *mut xlib::Display {
        self.dpy
    }

    pub fn fontset_create(&mut self, font_names: &[&str]) -> bool {
        self.drw.fontset_create(font_names)
    }

    pub fn scm_create(&mut self, clr_names: &[&str]) -> SchemeId {
        let scheme_ptr = self.drw.scm_create(clr_names);
        self.schemes.push(scheme_ptr);
        SchemeId(self.schemes.len() - 1)
    }

    pub fn get_font_height(&self) -> u32 {
        if self.drw.fonts.is_empty() {
            0
        } else {
            self.drw.fonts[0].h
        }
    }

    pub fn rect(&mut self, scheme: SchemeId, x: i32, y: i32, w: u32, h: u32, filled: bool, invert: bool) {
        if let Some(s) = self.schemes.get(scheme.0) {
            self.drw.scheme = *s;
            self.drw.rect(x, y, w, h, filled, invert);
        }
    }

    pub fn text(&mut self, scheme: SchemeId, x: i32, y: i32, w: u32, h: u32, lpad: u32, text: &str, invert: bool) -> i32 {
        if let Some(s) = self.schemes.get(scheme.0) {
            self.drw.scheme = *s;
            self.drw.text(x, y, w, h, lpad, text, invert)
        } else {
            0
        }
    }

    pub fn text_width(&self, text: &str) -> u32 {
        self.drw.text_width(text)
    }

    pub fn map_drawable(&mut self, win: Window, x: i32, y: i32, w: u32, h: u32) {
        self.drw.map(win.0, x, y, w, h);
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

    pub fn create_window(
        &self,
        parent: Window,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        border_width: u32,
        depth: i32,
        class: u32,
        visual: *mut xlib::Visual,
        valuemask: u64,
        attributes: &mut xlib::XSetWindowAttributes,
    ) -> Window {
        unsafe {
            Window(xlib::XCreateWindow(
                self.dpy,
                parent.0,
                x,
                y,
                width,
                height,
                border_width,
                depth,
                class,
                visual,
                valuemask,
                attributes,
            ))
        }
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

    pub fn set_window_cursor(&self, win: Window, cursor_id: CursorId) {
        unsafe {
            xlib::XDefineCursor(self.dpy, win.0, cursor_id.0);
        }
    }

    pub fn create_font_cursor_as_id(&self, shape: u32) -> CursorId {
        CursorId(unsafe { xlib::XCreateFontCursor(self.dpy, shape) })
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

    pub fn allow_events(&self, mode: i32) {
        unsafe {
            xlib::XAllowEvents(self.dpy, mode, xlib::CurrentTime);
        }
    }

    pub fn ungrab_key(&self, win: Window) {
        unsafe {
            xlib::XUngrabKey(self.dpy, xlib::AnyKey, xlib::AnyModifier, win.0);
        }
    }

    pub fn select_input_for_substructure_redirect(&self, win: Window) {
        unsafe {
            xlib::XSelectInput(
                self.dpy,
                win.0,
                xlib::SubstructureRedirectMask,
            );
        }
    }

    pub fn get_window_attributes(&self, win: Window) -> Result<xlib::XWindowAttributes, ()> {
        unsafe {
            let mut wa: xlib::XWindowAttributes = std::mem::zeroed();
            if xlib::XGetWindowAttributes(self.dpy, win.0, &mut wa) != 0 {
                Ok(wa)
            } else {
                Err(())
            }
        }
    }

    pub fn get_transient_for_hint(&self, win: Window) -> Option<Window> {
        unsafe {
            let mut transient_win: xlib::Window = 0;
            if xlib::XGetTransientForHint(self.dpy, win.0, &mut transient_win) != 0 {
                Some(Window(transient_win))
            } else {
                None
            }
        }
    }

    pub fn query_tree(&self, win: Window) -> Result<(Window, Window, Vec<Window>), ()> {
        unsafe {
            let mut root_return: xlib::Window = 0;
            let mut parent_return: xlib::Window = 0;
            let mut children: *mut xlib::Window = std::ptr::null_mut();
            let mut nchildren: u32 = 0;
            if xlib::XQueryTree(
                self.dpy,
                win.0,
                &mut root_return,
                &mut parent_return,
                &mut children,
                &mut nchildren,
            ) != 0
            {
                let children_vec = if nchildren > 0 {
                    std::slice::from_raw_parts(children, nchildren as usize)
                        .iter()
                        .map(|&w| Window(w))
                        .collect()
                } else {
                    Vec::new()
                };
                if !children.is_null() {
                    xlib::XFree(children as *mut _);
                }
                Ok((
                    Window(root_return),
                    Window(parent_return),
                    children_vec,
                ))
            } else {
                Err(())
            }
        }
    }

    pub fn map_window(&self, win: Window) {
        unsafe { xlib::XMapWindow(self.dpy, win.0) };
    }

    pub fn raise_window(&self, win: Window) {
        unsafe { xlib::XRaiseWindow(self.dpy, win.0) };
    }

    pub fn keycode_to_keysym(&self, keycode: u32) -> u64 {
        unsafe { xlib::XKeycodeToKeysym(self.dpy, keycode as u8, 0) }
    }

    pub fn set_input_focus(&self, win: Window, revert_to: i32) {
        unsafe {
            xlib::XSetInputFocus(self.dpy, win.0, revert_to, xlib::CurrentTime);
        }
    }

    pub fn move_window(&self, win: Window, x: i32, y: i32) {
        unsafe {
            xlib::XMoveWindow(self.dpy, win.0, x, y);
        }
    }

    pub fn configure_window(
        &self,
        win: Window,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        border_width: i32,
    ) {
        unsafe {
            let mut wc: xlib::XWindowChanges = std::mem::zeroed();
            wc.x = x;
            wc.y = y;
            wc.width = w;
            wc.height = h;
            wc.border_width = border_width;
            let mask = xlib::CWX | xlib::CWY | xlib::CWWidth | xlib::CWHeight | xlib::CWBorderWidth;
            xlib::XConfigureWindow(self.dpy, win.0, mask as u32, &mut wc);
        }
    }

    pub fn grab_keys(&self, win: Window, numlockmask: u32, keys: &[KeySpec]) {
        unsafe {
            xlib::XUngrabKey(self.dpy, xlib::AnyKey, xlib::AnyModifier, win.0);

            let modifiers: [u32; 4] = [0, xlib::LockMask, numlockmask, numlockmask | xlib::LockMask];

            for key in keys {
                let code = xlib::XKeysymToKeycode(self.dpy, key.keysym as u64);
                if code == 0 {
                    continue;
                }
                for &m in &modifiers {
                    xlib::XGrabKey(
                        self.dpy,
                        code as c_int,
                        key.mask | m,
                        win.0,
                        1,
                        xlib::GrabModeAsync,
                        xlib::GrabModeAsync,
                    );
                }
            }
        }
    }

    pub fn ungrab_keys(&self, win: Window) {
        unsafe {
            xlib::XUngrabKey(self.dpy, xlib::AnyKey, xlib::AnyModifier, win.0);
        }
    }

    pub fn unmanage_window(&self, win: Window) {
        unsafe {
            xlib::XUngrabButton(
                self.dpy,
                xlib::AnyButton as u32,
                xlib::AnyModifier as u32,
                win.0,
            );
            xlib::XSetWindowBorder(self.dpy, win.0, 0);
            xlib::XRemoveFromSaveSet(self.dpy, win.0);
        }
    }

    pub fn get_wm_protocols(&self, win: Window) -> Vec<xlib::Atom> {
        unsafe {
            let mut protocols_ptr: *mut xlib::Atom = std::ptr::null_mut();
            let mut count = 0;
            let status = xlib::XGetWMProtocols(self.dpy, win.0, &mut protocols_ptr, &mut count);

            if status != 0 && count > 0 && !protocols_ptr.is_null() {
                let protocols = std::slice::from_raw_parts(protocols_ptr, count as usize).to_vec();
                xlib::XFree(protocols_ptr as *mut _);
                protocols
            } else {
                Vec::new()
            }
        }
    }

    pub fn send_client_message(
        &self,
        win: Window,
        message_type: xlib::Atom,
        data: [i64; 5],
    ) {
        unsafe {
            let mut ev: xlib::XEvent = std::mem::zeroed();
            ev.client_message.type_ = xlib::ClientMessage;
            ev.client_message.window = win.0;
            ev.client_message.message_type = message_type;
            ev.client_message.format = 32;
            ev.client_message.data.set_long(0, data[0]);
            ev.client_message.data.set_long(1, data[1]);
            ev.client_message.data.set_long(2, data[2]);
            ev.client_message.data.set_long(3, data[3]);
            ev.client_message.data.set_long(4, data[4]);
            xlib::XSendEvent(self.dpy, win.0, 0, xlib::NoEventMask, &mut ev);
        }
    }

    pub fn grab_server(&self) {
        unsafe { xlib::XGrabServer(self.dpy) };
    }

    pub fn ungrab_server(&self) {
        unsafe { xlib::XUngrabServer(self.dpy) };
    }

    pub fn set_close_down_mode(&self, mode: i32) {
        unsafe { xlib::XSetCloseDownMode(self.dpy, mode) };
    }

    pub fn kill_client(&self, win: Window) {
        unsafe { xlib::XKillClient(self.dpy, win.0) };
    }

    pub fn sync(&self, discard: bool) {
        unsafe { xlib::XSync(self.dpy, if discard { 1 } else { 0 }) };
    }

    pub fn next_event(&self) -> Option<xlib::XEvent> {
        unsafe {
            let mut ev: xlib::XEvent = std::mem::zeroed();
            if xlib::XNextEvent(self.dpy, &mut ev) == 0 {
                Some(ev)
            } else {
                None
            }
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

#[derive(Debug)]
pub enum XError {
    DisplayOpen,
    AtomIntern(String),
}
