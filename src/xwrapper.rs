use x11::xft::XftDraw;
use std::ffi::CString;
use std::os::raw::{c_int, c_uint};
use std::ptr::null_mut;
use x11::{keysym, xft, xlib};
use crate::colour::{ALL_COLOURS, Colour};
use crate::ivec2::IVec2;

fn die(s: &str) {
    eprintln!("{}", s);
    std::process::exit(1);
}

// From <X11/Xproto.h>
pub const X_SET_INPUT_FOCUS: u8 = 42;
pub const X_POLY_TEXT8: u8 = 74;
pub const X_POLY_FILL_RECTANGLE: u8 = 69;
pub const X_POLY_SEGMENT: u8 = 66;
pub const X_CONFIGURE_WINDOW: u8 = 12;
pub const X_GRAB_BUTTON: u8 = 28;
pub const X_GRAB_KEY: u8 = 33;
pub const X_COPY_AREA: u8 = 62;

static mut X_ERROR_OCCURRED: bool = false;

#[allow(unused_variables)]
unsafe extern "C" fn x_error_ignore(dpy: *mut xlib::Display, ee: *mut xlib::XErrorEvent) -> c_int {
    // Always return 0 to tell Xlib that the error was handled.
    0
}

unsafe extern "C" fn x_error_start(
    _dpy: *mut xlib::Display,
    _ee: *mut xlib::XErrorEvent,
) -> c_int { unsafe {
    X_ERROR_OCCURRED = true;
    0
}}

/// There's no way to check accesses to destroyed windows, thus those cases are
/// ignored (especially on UnmapNotify's). Other types of errors call Xlibs
/// default error handler, which may call exit.
unsafe extern "C" fn x_error(_dpy: *mut xlib::Display, ee: *mut xlib::XErrorEvent) -> c_int {
    let ee_ref = unsafe { &*ee };
    if ee_ref.error_code == xlib::BadWindow
        || (ee_ref.request_code == X_SET_INPUT_FOCUS && ee_ref.error_code == xlib::BadMatch)
        || (ee_ref.request_code == X_POLY_TEXT8 && ee_ref.error_code == xlib::BadDrawable)
        || (ee_ref.request_code == X_POLY_FILL_RECTANGLE && ee_ref.error_code == xlib::BadDrawable)
        || (ee_ref.request_code == X_POLY_SEGMENT && ee_ref.error_code == xlib::BadDrawable)
        || (ee_ref.request_code == X_CONFIGURE_WINDOW && ee_ref.error_code == xlib::BadMatch)
        || (ee_ref.request_code == X_GRAB_BUTTON && ee_ref.error_code == xlib::BadAccess)
        || (ee_ref.request_code == X_GRAB_KEY && ee_ref.error_code == xlib::BadAccess)
        || (ee_ref.request_code == X_COPY_AREA && ee_ref.error_code == xlib::BadDrawable)
    {
        return 0;
    }

    eprintln!(
        "gmux: fatal error: request code={}, error code={}",
        ee_ref.request_code, ee_ref.error_code
    );

    // Call the default error handler which will exit
    // This is not a direct equivalent, but it's the safest thing to do
    // without the original xerrorxlib variable.
    // In a more robust implementation, we might get the default handler and call it.
    // For now, exiting is the clearest action.
    die("fatal X error");
    0 // Unreachable
}

use fontconfig::{self};

#[derive(PartialEq, Copy, Clone)]
pub enum Net {
    Supported,
    WMName,
    WMState,
    WMCheck,
    WMFullscreen,
    ActiveWindow,
    WMWindowType,
    WMWindowTypeDialog,
    ClientList,
    Last,
}
#[derive(PartialEq, Copy, Clone)]
pub enum WM {
    Protocols,
    Delete,
    State,
    TakeFocus,
    Last,
}

pub enum Atom {
    Net(Net),
    Wm(WM),
}

pub struct Font {
    pub dpy: *mut xlib::Display,
    pub h: c_uint,
    pub xfont: *mut xft::XftFont,
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe {
            if !self.xfont.is_null() {
                xft::XftFontClose(self.dpy, self.xfont);
            }
        }
    }
}

type Color = xft::XftColor;

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

pub struct KeySpecification {
    pub mask: u32,
    pub keysym: u32,
}

pub struct XWrapper {
    dpy: *mut xlib::Display,
    _w: c_uint,
    _h: c_uint,
    pub screen: c_int,
    _root: xlib::Window,
    pub drawable: xlib::Drawable,
    gc: xlib::GC,
    xftdraw: *mut XftDraw, // <<< ADDED: Cached XftDraw object
    pub fonts: Vec<Font>,
    colors: [Color; ALL_COLOURS.len()],
    pub atoms: Atoms,
}

impl XWrapper {
    pub fn connect() -> Result<Self, XError> {
        unsafe {
            let dpy = xlib::XOpenDisplay(null_mut());
            if dpy.is_null() {
                return Err(XError::DisplayOpen);
            }

            let screen = xlib::XDefaultScreen(dpy);
            let root = xlib::XRootWindow(dpy, screen);
            let w = xlib::XDisplayWidth(dpy, screen) as u32;
            let h = xlib::XDisplayHeight(dpy, screen) as u32;
            
            // Create the pixmap for double-buffering
            let drawable = xlib::XCreatePixmap(dpy, root, w, h, xlib::XDefaultDepth(dpy, screen) as u32);
            
            // Create the Graphics Context for rectangle drawing
            let gc = xlib::XCreateGC(dpy, root, 0, null_mut());
            xlib::XSetLineAttributes(dpy, gc, 1, xlib::LineSolid, xlib::CapButt, xlib::JoinMiter);

            // <<< ADDED: Create the XftDraw object ONCE for our pixmap
            let xftdraw = xft::XftDrawCreate(
                dpy,
                drawable,
                xlib::XDefaultVisual(dpy, screen),
                xlib::XDefaultColormap(dpy, screen),
            );
            if xftdraw.is_null() {
                // Handle error appropriately, maybe return an Err
                die("Failed to create XftDraw");
            }

            let atoms = Atoms::new(dpy)?;
            let mut wrapper = Self {
                dpy,
                _w: w,
                _h: h,
                screen,
                _root: root,
                drawable,
                gc,
                xftdraw, // <<< ADDED: Store the cached object
                fonts: Vec::new(),
                colors: [std::mem::zeroed(); ALL_COLOURS.len()],
                atoms,
            };
            wrapper.init_colors();
            Ok(wrapper)
        }
    }

    fn init_colors(&mut self) {
        for (i, colour) in ALL_COLOURS.iter().enumerate() {
            let rgba = colour.get_colour();
            let mut clr = unsafe { std::mem::zeroed() };
            let color_val = (rgba[3] as u32) << 24
                | (rgba[0] as u32) << 16
                | (rgba[1] as u32) << 8
                | (rgba[2] as u32);

            unsafe {
                if xft::XftColorAllocValue(
                    self.dpy,
                    xlib::XDefaultVisual(self.dpy, self.screen),
                    xlib::XDefaultColormap(self.dpy, self.screen),
                    &x11::xrender::XRenderColor {
                        red: ((color_val >> 16) & 0xff) as u16 * 0x101,
                        green: ((color_val >> 8) & 0xff) as u16 * 0x101,
                        blue: (color_val & 0xff) as u16 * 0x101,
                        alpha: ((color_val >> 24) & 0xff) as u16 * 0x101,
                    },
                    &mut clr,
                ) == 0
                {
                    die("cannot allocate color");
                }
            }
            self.colors[i] = clr;
        }
    }

    /// Provides temporary access to the raw display pointer.
    /// This should be phased out as more functionality is moved into the wrapper.
    /*
    pub fn dpy(&self) -> *mut xlib::Display {
        self.dpy
    }
    */

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
            let fnt = Font {
                dpy: self.dpy,
                h,
                xfont,
            };
            self.fonts.push(fnt);
            true
        }
    }

    pub fn get_font_height(&self) -> u32 {
        if self.fonts.is_empty() {
            0
        } else {
            self.fonts[0].h
        }
    }

    pub fn rect(&mut self, color: Colour, tl: IVec2, wh: IVec2, filled: bool) {
        let clr = &self.colors[color as usize];
        unsafe {
            xlib::XSetForeground(
                self.dpy,
                self.gc,
                clr.pixel
            );
            if filled {
                xlib::XFillRectangle(self.dpy, self.drawable, self.gc, tl.x, tl.y, wh.x as _, wh.y as _);
            } else {
                xlib::XDrawRectangle(self.dpy, self.drawable, self.gc, tl.x, tl.y, (wh.x - 1) as _, (wh.y - 1) as _);
            }
        }
    }

    // <<< MODIFIED: This function is now much simpler and more efficient
    pub fn text(&mut self, color: Colour, tl: IVec2, wh: IVec2, lpad: u32, text: &str) {
        if self.fonts.is_empty() || text.is_empty() {
            return;
        }
    
        unsafe {
            let clr = &mut self.colors[color as usize];
            let usedfont = &self.fonts[0];
    
            // Calculate horizontal position with padding
            let x = tl.x + lpad as i32;
    
            // Calculate vertical position for the text baseline to center it
            let font_height = (*usedfont.xfont).ascent + (*usedfont.xfont).descent;
            let y = tl.y + (wh.y - font_height as i32) / 2 + (*usedfont.xfont).ascent as i32;
    
            // Draw the string using the cached xftdraw object
            xft::XftDrawStringUtf8(
                self.xftdraw,
                clr,
                usedfont.xfont,
                x,
                y,
                text.as_ptr() as *const u8,
                text.len() as i32,
            );
        }
    }
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

    pub fn map_drawable(&mut self, win: Window, x: i32, y: i32, w: u32, h: u32) {
        unsafe {
            xlib::XCopyArea(self.dpy, self.drawable, win.0, self.gc, x, y, w, h, x, y);
            xlib::XSync(self.dpy, 0);
        }
    }

    /*
    pub fn intern_atom(&self, atom_name: &str) -> Result<xlib::Atom, XError> {
        let c_str = CString::new(atom_name)
            .map_err(|_| XError::AtomIntern(atom_name.to_string()))?;
        unsafe { Ok(xlib::XInternAtom(self.dpy, c_str.as_ptr(), 0)) }
    }
    */

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

    /*
    pub fn create_font_cursor(&self, shape: u32) -> xlib::Cursor {
        unsafe { xlib::XCreateFontCursor(self.dpy, shape) }
    }

    pub fn set_window_cursor(&self, win: Window, cursor_id: CursorId) {
        unsafe {
            xlib::XDefineCursor(self.dpy, win.0, cursor_id.0);
        }
    }
    */

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
            if xlib::XGetTransientForHint(self.dpy, win.0, &mut transient_win) != 0 && transient_win != 0 {
                Some(Window(transient_win))
            } else {
                None
            }
        }
    }

    pub fn get_window_title(&self, win: Window) -> Option<String> {
        unsafe {
            use std::ffi::{CStr, c_char};
            let mut text_prop: xlib::XTextProperty = std::mem::zeroed();
            let net_wm_name = self.atoms.get(Atom::Net(Net::WMName));

            // First try _NET_WM_NAME (UTF-8)
            if xlib::XGetTextProperty(self.dpy, win.0, &mut text_prop, net_wm_name) != 0
                && !text_prop.value.is_null()
            {
                let mut list: *mut *mut c_char = std::ptr::null_mut();
                let mut count = 0;
                if xlib::Xutf8TextPropertyToTextList(self.dpy, &mut text_prop, &mut list, &mut count)
                    == xlib::Success as i32
                    && count > 0
                    && !list.is_null() && !(*list).is_null()
                {
                    let rust_str = CStr::from_ptr(*list).to_string_lossy().into_owned();
                    xlib::XFreeStringList(list);
                    xlib::XFree(text_prop.value as *mut _);
                    return Some(rust_str);
                }
                if !text_prop.value.is_null() {
                    xlib::XFree(text_prop.value as *mut _);
                }
            }

            // Fallback to WM_NAME (legacy)
            let mut window_name: *mut c_char = std::ptr::null_mut();
            if xlib::XFetchName(self.dpy, win.0, &mut window_name) != 0 {
                if !window_name.is_null() {
                    let rust_str = CStr::from_ptr(window_name).to_string_lossy().into_owned();
                    xlib::XFree(window_name as *mut _);
                    return Some(rust_str);
                }
            }

            None
        }
    }

    pub fn get_wm_normal_hints(&self, win: Window) -> Result<xlib::XSizeHints, ()> {
        unsafe {
            let mut hints: xlib::XSizeHints = std::mem::zeroed();
            let mut supplied: i64 = 0;
            if xlib::XGetWMNormalHints(self.dpy, win.0, &mut hints, &mut supplied) == 0 {
                Err(())
            } else {
                Ok(hints)
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

    pub fn grab_keys(&self, win: Window, numlockmask: u32, keys: &[KeySpecification]) {
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

    pub fn query_pointer_position(&self) -> Option<(i32, i32)> {
        unsafe {
            let mut root_return = 0;
            let mut child_return = 0;
            let mut root_x_return = 0;
            let mut root_y_return = 0;
            let mut win_x_return = 0;
            let mut win_y_return = 0;
            let mut mask_return = 0;

            let screen = self.default_screen();
            let root = self.root_window(screen);

            let result = xlib::XQueryPointer(
                self.dpy,
                root.0,
                &mut root_return,
                &mut child_return,
                &mut root_x_return,
                &mut root_y_return,
                &mut win_x_return,
                &mut win_y_return,
                &mut mask_return,
            );

            if result != 0 {
                Some((root_x_return, root_y_return))
            } else {
                None
            }
        }
    }

    pub fn check_for_other_wm(&mut self) -> Result<(), &str> {
        unsafe {
            X_ERROR_OCCURRED = false;
            self.set_error_handler(Some(x_error_start));
            let root = self.root_window(self.default_screen());
            self.select_input_for_substructure_redirect(root);
            self.sync(false);

            if X_ERROR_OCCURRED {
                return Err("another window manager is already running");
            }
        }
        Ok(())
    }

    pub fn set_default_error_handler(&self) {
        self.set_error_handler(Some(x_error));
    }

    pub fn set_ignore_error_handler(&self) {
        self.set_error_handler(Some(x_error_ignore));
    }

    pub fn stack_windows(&self, windows: &[Window]) {
        unsafe {
            let mut wc: xlib::XWindowChanges = std::mem::zeroed();
            wc.stack_mode = xlib::Above as i32;
            let changes = xlib::CWStackMode | xlib::CWSibling;
            
            for (i, win) in windows.iter().enumerate() {
                if i > 0 {
                    wc.sibling = windows[i - 1].0;
                }
                xlib::XConfigureWindow(self.dpy, win.0, (changes) as u32, &mut wc);
            }
        }
    }

    pub fn clean_mask(&self, mask: u32) -> u32 {
        mask & !(xlib::LockMask | xlib::Mod2Mask)
            & (xlib::ShiftMask
                | xlib::ControlMask
                | xlib::Mod1Mask
                | xlib::Mod3Mask
                | xlib::Mod4Mask
                | xlib::Mod5Mask)
    }

    pub fn send_event(&self, win: Window, proto: xlib::Atom) -> bool {
        let protocols = self.get_wm_protocols(win);
        if protocols.contains(&proto) {
            let mut data = [0; 5];
            data[0] = proto as i64;
            data[1] = xlib::CurrentTime as i64;
            self.send_client_message(win, self.atoms.get(Atom::Wm(WM::Protocols)), data);
            true
        } else {
            false
        }
    }

    pub fn set_window_border(&self, win: Window, color_pixel: u64) {
        unsafe {
            xlib::XSetWindowBorder(self.dpy, win.0, color_pixel);
        }
    }

    pub fn set_window_border_color(&self, win: Window, color: Colour) {
        let pixel = self.colors[color as usize].pixel;
        self.set_window_border(win, pixel);
    }
}


impl Drop for XWrapper {
    fn drop(&mut self) {
        unsafe {
            // <<< ADDED: Destroy the cached XftDraw object
            if !self.xftdraw.is_null() {
                xft::XftDrawDestroy(self.xftdraw);
            }
            xlib::XFreePixmap(self.dpy, self.drawable);
            xlib::XFreeGC(self.dpy, self.gc);
            xlib::XCloseDisplay(self.dpy);
        }
    }
}

#[derive(Debug)]
pub enum XError {
    DisplayOpen,
    AtomIntern(()),
}

pub struct Atoms {
    wmatom: [xlib::Atom; WM::Last as usize],
    netatom: [xlib::Atom; Net::Last as usize],
}

impl Atoms {
    pub fn new(dpy: *mut xlib::Display) -> Result<Self, XError> {
        let mut atoms = Self {
            wmatom: [0; WM::Last as usize],
            netatom: [0; Net::Last as usize],
        };

        let intern = |name: &str| -> Result<xlib::Atom, XError> {
            let c_str = CString::new(name)
                .map_err(|_| XError::AtomIntern(()))?;
            unsafe { Ok(xlib::XInternAtom(dpy, c_str.as_ptr(), 0)) }
        };

        atoms.wmatom[WM::Protocols as usize] = intern("WM_PROTOCOLS")?;
        atoms.wmatom[WM::Delete as usize] = intern("WM_DELETE_WINDOW")?;
        atoms.wmatom[WM::State as usize] = intern("WM_STATE")?;
        atoms.wmatom[WM::TakeFocus as usize] = intern("WM_TAKE_FOCUS")?;
        atoms.netatom[Net::ActiveWindow as usize] = intern("_NET_ACTIVE_WINDOW")?;
        atoms.netatom[Net::Supported as usize] = intern("_NET_SUPPORTED")?;
        atoms.netatom[Net::WMName as usize] = intern("_NET_WM_NAME")?;
        atoms.netatom[Net::WMState as usize] = intern("_NET_WM_STATE")?;
        atoms.netatom[Net::WMCheck as usize] = intern("_NET_SUPPORTING_WM_CHECK")?;
        atoms.netatom[Net::WMFullscreen as usize] = intern("_NET_WM_STATE_FULLSCREEN")?;
        atoms.netatom[Net::WMWindowType as usize] = intern("_NET_WM_WINDOW_TYPE")?;
        atoms.netatom[Net::WMWindowTypeDialog as usize] = intern("_NET_WM_WINDOW_TYPE_DIALOG")?;
        atoms.netatom[Net::ClientList as usize] = intern("_NET_CLIENT_LIST")?;

        Ok(atoms)
    }

    pub fn get(&self, atom: Atom) -> xlib::Atom {
        match atom {
            Atom::Net(net_atom) => self.netatom[net_atom as usize],
            Atom::Wm(wm_atom) => self.wmatom[wm_atom as usize],
        }
    }
    
    pub fn net_atom_ptr(&self) -> *const xlib::Atom {
        self.netatom.as_ptr()
    }
}
