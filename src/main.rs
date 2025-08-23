#![allow(warnings)]
use std::ffi::{CString, CStr};
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};
use std::ptr::{null_mut};
use x11::xlib;
use x11::xft;
use x11::keysym;

mod xwrapper;
mod command;
use command::*;
use xwrapper::{KeySpec, SchemeId, Window, XWrapper};

// From <X11/Xproto.h>
const X_SET_INPUT_FOCUS: u8 = 42;
const X_POLY_TEXT8: u8 = 74;
const X_POLY_FILL_RECTANGLE: u8 = 69;
const X_POLY_SEGMENT: u8 = 66;
const X_CONFIGURE_WINDOW: u8 = 12;
const X_GRAB_BUTTON: u8 = 28;
const X_GRAB_KEY: u8 = 33;
const X_COPY_AREA: u8 = 62;

// Simple X11 error handler that ignores non-fatal errors like BadWindow so gmux
// doesn't exit (mirrors dwm's default behaviour).
#[allow(unused_variables)]
unsafe extern "C" fn xerror_ignore(dpy: *mut xlib::Display, ee: *mut xlib::XErrorEvent) -> c_int {
    // Always return 0 to tell Xlib that the error was handled.
    0
}

/* ========= configurable constants (similar to dwm's config.h) ========= */
const FONT: &str = "monospace:size=24"; // change size here to scale bar text
const BORDERPX: i32 = 2;
const BROKEN_UTF8: &str = "";
// ======================================================================

fn ecalloc(nmemb: usize, size: usize) -> *mut std::ffi::c_void {
    unsafe {
        let ptr = libc::calloc(nmemb, size);
        if ptr.is_null() {
            panic!("fatal: could not calloc");
        }
        ptr
    }
}

fn drawbar(state: &mut GmuxState, mon_idx: usize) {
    let is_selmon = state.selmon == mon_idx;
    let mon = &state.mons[mon_idx];
    let ww = mon.ww;
    let bh = state.bh;
    let barwin = mon.barwin;
    let ltsymbol = mon.ltsymbol;
    let clients = &mon.clients;
    let sel = mon.sel;
    let scheme_norm = state.schemes[0];
    let scheme_sel = state.schemes[1];

    let mut x = 0;
    let mut w = 0;
    let mut tw = 0;

    state.xwrapper.rect(scheme_norm, 0, 0, ww as u32, bh as u32, true, true);

    if is_selmon {
        let status = unsafe { CStr::from_ptr(state.stext.as_ptr()).to_str().unwrap_or("") };
        tw = state.xwrapper.text(scheme_norm, ww as i32 - tw, 0, 0, 0, 0, status, false)
            + state.xwrapper.get_font_height() as i32;
    }

    let mut urg: u32 = 0;
    for c in clients {
        urg |= c.tags;
    }
    state.xwrapper.rect(scheme_norm, 0, 0, ww as u32, bh as u32, true, true);

    let ltsymbol_str = unsafe { CStr::from_ptr(ltsymbol.as_ptr()).to_str().unwrap_or("") };
    if !ltsymbol_str.is_empty() {
        w = state.xwrapper.text(scheme_norm, 0, 0, 0, 0, 0, ltsymbol_str, false);
        state.xwrapper.rect(scheme_norm, x, 0, w as u32, bh as u32, true, true);
        state.xwrapper.text(scheme_norm, x, 0, w as u32, bh as u32, 0, ltsymbol_str, false);
        x = w;
    }

    for i in 0..state.tags.len() {
        let mut occupied = false;
        for c in clients {
            if (c.tags & (1 << i)) != 0 {
                occupied = true;
                break;
            }
        }
        w = state.xwrapper.text_width(state.tags[i]) as i32;
        let scheme = if mon.tagset[mon.seltags as usize] & 1 << i != 0 {
            scheme_sel
        } else {
            scheme_norm
        };
        state.xwrapper.rect(scheme, x, 0, w as u32, bh as u32, true, true);
        if urg & (1 << i) != 0 {
            state.xwrapper.rect(scheme, x + 1, 1, (w - 2) as u32, (bh - 2) as u32, false, true);
        }
        state.xwrapper.text(scheme, x, 0, w as u32, bh as u32, 0, state.tags[i], false);
        unsafe {
            if let Some(s_idx) = mon.sel {
                let sel_client = &mon.clients[s_idx];
                if (sel_client.tags & (1 << i)) != 0 {
                    state.xwrapper.rect(scheme_sel, x + 1, 1, (w - 2) as u32, (bh - 2) as u32, false, false);
                }
            }
        }
        x += w;
    }

    w = ww - tw;
    unsafe {
        if let Some(s_idx) = mon.sel {
            let sel_client = &mon.clients[s_idx];
            let name = CStr::from_ptr(sel_client.name.as_ptr()).to_str().unwrap_or(BROKEN_UTF8);
            state.xwrapper.text(scheme_sel, x, 0, w as u32, bh as u32, 0, name, false);
            if sel_client.isfloating {
                state.xwrapper.rect(scheme_sel, x + 5, 5, (w - 10) as u32, (bh - 2) as u32, false, false);
            }
        } else {
            state.xwrapper.rect(scheme_norm, x, 0, w as u32, bh as u32, true, true);
        }
    }

    state.xwrapper.map_drawable(barwin, 0, 0, ww as u32, bh as u32);
}


fn drawbars(state: &mut GmuxState) {
    for i in 0..state.mons.len() {
        drawbar(state, i);
    }
}

fn updatenumlockmask(state: &mut GmuxState) {
    let mut_state = state as *mut GmuxState;
    unsafe {
        let mut i = 0;
        let modmap = xlib::XGetModifierMapping((*mut_state).xwrapper.dpy());
        if modmap.is_null() {
            return;
        }
        let max_keypermod = (*modmap).max_keypermod;
        let mut p = (*modmap).modifiermap;
        while i < 8 {
            let mut j = 0;
            while j < max_keypermod {
                if *p != 0 && xlib::XKeycodeToKeysym((*mut_state).xwrapper.dpy(), *p, 0) as u32 == keysym::XK_Num_Lock {
                    (*mut_state).numlockmask = 1 << i;
                }
                p = p.offset(1);
                j += 1;
            }
            i += 1;
        }
        xlib::XFreeModifiermap(modmap);
    }
}

struct SyncPtr(*const c_char);
unsafe impl Sync for SyncPtr {}

#[derive(Copy, Clone)]
struct SyncVoidPtr(*const c_void);
unsafe impl Sync for SyncVoidPtr {}

// Enums

#[derive(PartialEq, Copy, Clone)]
enum Cur {
    Normal,
    Resize,
    Move,
    Last,
}
#[derive(PartialEq, Copy, Clone)]
enum Net {
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
enum WM {
    Protocols,
    Delete,
    State,
    TakeFocus,
    Last,
}

#[derive(PartialEq, Copy, Clone)]
enum Clk {
    TagBar,
    LtSymbol,
    StatusText,
    WinTitle,
    ClientWin,
    RootWin,
    Last,
}

// Structs

#[derive(Copy, Clone)]
union Arg {
    i: i32,
    ui: u32,
    f: f32,
    v: SyncVoidPtr,
}


struct Button {
    click: Clk,
    mask: c_uint,
    button: c_uint,
    func: unsafe extern "C" fn(arg: &Arg),
    arg: Arg,
}

#[derive(Debug, Clone, Default)]
struct Monitor {
    ltsymbol: [c_char; 16],
    mfact: f32,
    nmaster: i32,
    
    num: i32,
    
    by: i32,
    
    mx: i32,
    
    my: i32,
    
    mw: i32,
    
    mh: i32,
    wx: i32,
    wy: i32,
    ww: i32,
    wh: i32,
    seltags: u32,
    sellt: u32,
    tagset: [u32; 2],
    
    showbar: bool,
    
    topbar: bool,
    clients: Vec<Client>,
    sel: Option<usize>,
    stack: Vec<usize>,
    barwin: Window,
    lt: [*const Layout; 2],
}

#[derive(Debug, Clone, Copy)]
struct Client {
    
    name: [c_char; 256],
    
    mina: f32,
    
    maxa: f32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    oldx: i32,
    oldy: i32,
    oldw: i32,
    oldh: i32,
    
    basew: i32,
    
    baseh: i32,
    
    incw: i32,
    
    inch: i32,
    
    maxw: i32,
    
    maxh: i32,
    
    minw: i32,
    
    minh: i32,
    bw: i32,
    
    oldbw: i32,
    tags: u32,
    
    isfixed: bool,
    isfloating: bool,
    isurgent: bool,
    
    neverfocus: bool,
    
    oldstate: bool,
    
    isfullscreen: bool,
    next: *mut Client,
    snext: *mut Client,
    mon_idx: usize,
    win: Window,
}


struct Key {
    mask: u32,
    keysym: u32,
    func: unsafe extern "C" fn(&mut GmuxState, &Arg),
    arg: Arg,
}


struct Layout {
    symbol: SyncPtr,
    arrange: Option<unsafe extern "C" fn(&mut GmuxState, usize)>,
}


struct Rule {
    class: *const c_char,
    instance: *const c_char,
    title: *const c_char,
    tags: c_uint,
    isfloating: c_int,
    monitor: c_int,
}

// X11 FFI forward declarations from drw.h
type Clr = xft::XftColor;

// Global state
struct GmuxState {
    
    stext: [c_char; 256],
    screen: c_int,
    sw: c_int,
    sh: c_int,
    bh: c_int,
    
    blw: c_int,
    
    lrpad: c_int,
    numlockmask: c_uint,
    handler: [Option<unsafe extern "C" fn(&mut GmuxState, *mut xlib::XEvent)>; xlib::LASTEvent as usize],
    wmatom: [xlib::Atom; WM::Last as usize],
    netatom: [xlib::Atom; Net::Last as usize],
    running: c_int,
    cursor: [*mut xlib::Cursor; Cur::Last as usize],
    schemes: [SchemeId; 2],
    xwrapper: XWrapper,
    mons: Vec<Monitor>,
    selmon: usize,
    root: Window,
    wmcheckwin: Window,
    xerror: bool,
    tags: [&'static str; 9],
}

impl GmuxState {
    
    unsafe fn wintomon(&mut self, w: xlib::Window) -> usize {
        let mut x = 0;
        let mut y = 0;
        let wrapped_w = Window(w);
        if wrapped_w == self.root {
            unsafe {
                if getrootptr(self, &mut x, &mut y) {
                    return self.recttomon(x, y, 1, 1);
                }
            }
        }
        for (i, m) in self.mons.iter().enumerate() {
            if m.barwin == wrapped_w {
                return i;
            }
        }
        if let Some((mon_idx, _)) = unsafe { wintoclient_idx(self, w) } {
            return mon_idx;
        }
        self.selmon
    }

    
    fn recttomon(&self, x: i32, y: i32, w: i32, h: i32) -> usize {
        let mut r = self.selmon;
        let mut area = 0;
        for (i, m) in self.mons.iter().enumerate() {
            let a = intersect(x, y, w, h, m);
            if a > area {
                area = a;
                r = i;
            }
        }
        r
    }

    
    unsafe fn arrange(&mut self, mon_idx: Option<usize>) {
        if let Some(idx) = mon_idx {
            if let Some(mon) = self.mons.get_mut(idx) {
                let stack = mon.stack.clone();
                show_hide(self, idx, &stack);
                unsafe {
                    self.arrange_mon(idx);
                    self.restack(idx);
                }
            }
        } else {
            for i in 0..self.mons.len() {
                let stack = self.mons[i].stack.clone();
                show_hide(self, i, &stack);
                unsafe { self.arrange_mon(i) };
            }
            for i in 0..self.mons.len() {
                unsafe { self.restack(i) };
            }
        }
    }

    
    unsafe fn arrange_mon(&mut self, mon_idx: usize) {
        if let Some(mon) = self.mons.get(mon_idx) {
            if let Some(layout) = mon.lt.get(mon.sellt as usize) {
                if let Some(arrange_fn) = unsafe { (**layout).arrange } {
                    unsafe { arrange_fn(self, mon_idx) };
                }
            }
        }
    }

    
    unsafe fn restack(&mut self, mon_idx: usize) {
        let dpy = self.xwrapper.dpy();
        drawbar(self, mon_idx);

        if let Some(m) = self.mons.get_mut(mon_idx) {
            if m.sel.is_none() {
                return;
            }
            let sel_idx = m.sel.unwrap();
            let sel = &m.clients[sel_idx];
            if sel.isfloating || m.lt.get(m.sellt as usize).is_none() {
                self.xwrapper.raise_window(sel.win);
            }
            if m.lt.get(m.sellt as usize).is_some() {
                let mut wc: xlib::XWindowChanges = unsafe { std::mem::zeroed() };
                wc.stack_mode = xlib::Below as i32;
                wc.sibling = m.barwin.0;
                let m_stack = m.stack.clone();
                for &c_idx in &m_stack {
                    let c = &m.clients[c_idx];
                    if !c.isfloating && is_visible(c, m) {
                        let win = c.win;
                        let cf = xlib::CWStackMode | xlib::CWSibling;
                        self.xwrapper.configure_window(
                            win,
                            c.x,
                            c.y,
                            c.w,
                            c.h,
                            c.bw,
                        );
                    }
                }
            }
            let mut wc: xlib::XWindowChanges = unsafe { std::mem::zeroed() };
            let sel_win = m.clients[sel_idx].win;
            wc.sibling = sel_win.0;
            wc.stack_mode = xlib::Above as i32;
            let cf = xlib::CWStackMode | xlib::CWSibling;

            let m_stack = m.stack.clone();
            for &c_idx in m_stack.iter().rev() {
                let c = &m.clients[c_idx];
                if c.isfloating {
                    let win = c.win;
                    self.xwrapper.configure_window(
                        win,
                        c.x,
                        c.y,
                        c.w,
                        c.h,
                        c.bw,
                    );
                }
            }
        }
    }
}

unsafe extern "C" fn xerror_start(
    _dpy: *mut xlib::Display,
    _ee: *mut xlib::XErrorEvent,
) -> c_int {
    die("gmux: another window manager is already running");
    // Unreachable, but necessary for the function signature
    0
}

/// There's no way to check accesses to destroyed windows, thus those cases are
/// ignored (especially on UnmapNotify's). Other types of errors call Xlibs
/// default error handler, which may call exit.
unsafe extern "C" fn xerror(dpy: *mut xlib::Display, ee: *mut xlib::XErrorEvent) -> c_int {
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


fn checkotherwm(state: &mut GmuxState) {
    unsafe extern "C" fn xerror_dummy(_dpy: *mut xlib::Display, _ee: *mut xlib::XErrorEvent) -> i32 {
        0
    }

    let state_ptr = state as *mut GmuxState;

    unsafe extern "C" fn xerror_start_handler(dpy: *mut xlib::Display, ee: *mut xlib::XErrorEvent) -> i32 {
        let state = unsafe { &mut *(dpy as *mut GmuxState) };
        state.xerror = true;
        unsafe { xerror_start(dpy, ee) }
    }

    unsafe extern "C" fn xerror_handler(dpy: *mut xlib::Display, ee: *mut xlib::XErrorEvent) -> i32 {
        let state = unsafe { &mut *(dpy as *mut GmuxState) };
        unsafe { xerror(dpy, ee) }
    }

    unsafe {
        state.xwrapper.set_error_handler(Some(xerror_start_handler));
        let root = state.xwrapper.root_window(state.xwrapper.default_screen());
        state
            .xwrapper
            .select_input_for_substructure_redirect(root);
        state.xwrapper.sync(false);

        let display_ptr = state.xwrapper.dpy() as *mut GmuxState;
        std::ptr::write(display_ptr, std::ptr::read(state_ptr));

        if (*state_ptr).xerror {
            die("gmux: another window manager is already running");
        }

        state.xwrapper.set_error_handler(Some(xerror_handler));
        state.xwrapper.sync(false);
    }
}

fn setup(state: &mut GmuxState) {
    unsafe {
        state.screen = state.xwrapper.default_screen();
        state.sw = state.xwrapper.display_width(state.screen);
        state.sh = state.xwrapper.display_height(state.screen);
        state.root = state.xwrapper.root_window(state.screen);
        
        let fonts = &["monospace:size=12"]; // TODO: configurable
        if !state.xwrapper.fontset_create(fonts) {
            die("no fonts could be loaded.");
        }

        // derive bar height and lrpad from font height like dwm
        let h = state.xwrapper.get_font_height() as i32;
        if h > 0 {
            state.bh = h + 2;
            state.lrpad = h + 2;
        }

        // color arrays are [ColFg, ColBg, ColBorder] following dwm
        let colors = &[
            &["#bbbbbb", "#222222", "#444444"], // SchemeNorm
            &["#eeeeee", "#005577", "#005577"], // SchemeSel
        ];
        state.schemes[0] = state.xwrapper.scm_create(colors[0]);
        state.schemes[1] = state.xwrapper.scm_create(colors[1]);

        // initialise status text sample
        let sample_status = b"gmux";
        for (i, b) in sample_status.iter().enumerate() {
            state.stext[i] = *b as i8;
        }

        drawbars(state);

        state.wmatom[WM::Protocols as usize] = state.xwrapper.intern_atom("WM_PROTOCOLS").unwrap();
        state.wmatom[WM::Delete as usize] = state.xwrapper.intern_atom("WM_DELETE_WINDOW").unwrap();
        state.wmatom[WM::State as usize] = state.xwrapper.intern_atom("WM_STATE").unwrap();
        state.wmatom[WM::TakeFocus as usize] = state.xwrapper.intern_atom("WM_TAKE_FOCUS").unwrap();
        state.netatom[Net::ActiveWindow as usize] = state.xwrapper.intern_atom("_NET_ACTIVE_WINDOW").unwrap();
        state.netatom[Net::Supported as usize] = state.xwrapper.intern_atom("_NET_SUPPORTED").unwrap();
        state.netatom[Net::WMName as usize] = state.xwrapper.intern_atom("_NET_WM_NAME").unwrap();
        state.netatom[Net::WMState as usize] = state.xwrapper.intern_atom("_NET_WM_STATE").unwrap();
        state.netatom[Net::WMCheck as usize] = state.xwrapper.intern_atom("_NET_SUPPORTING_WM_CHECK").unwrap();
        state.netatom[Net::WMFullscreen as usize] = state.xwrapper.intern_atom("_NET_WM_STATE_FULLSCREEN").unwrap();
        state.netatom[Net::WMWindowType as usize] = state.xwrapper.intern_atom("_NET_WM_WINDOW_TYPE").unwrap();
        state.netatom[Net::WMWindowTypeDialog as usize] = state.xwrapper.intern_atom("_NET_WM_WINDOW_TYPE_DIALOG").unwrap();
        state.netatom[Net::ClientList as usize] = state.xwrapper.intern_atom("_NET_CLIENT_LIST").unwrap();

        // Create a monitor
        let mut mon = Monitor::default();
        mon.tagset = [1, 1];
        mon.mfact = 0.55;
        mon.nmaster = 1;
        mon.showbar = true;
        mon.topbar = true;
        // Calculate window area accounting for the bar height
        if mon.showbar {
            mon.by = if mon.topbar { 0 } else { state.sh - state.bh };
            mon.wy = if mon.topbar { state.bh } else { 0 };
            mon.wh = state.sh - state.bh;
        } else {
            mon.by = -state.bh;
            mon.wy = 0;
            mon.wh = state.sh;
        }
        mon.lt[0] = &LAYOUTS[0];
        mon.lt[1] = &LAYOUTS[1];
        let symbol = CStr::from_ptr(LAYOUTS[0].symbol.0).to_str().unwrap();
        let c_symbol = CString::new(symbol).unwrap();
        let dest = mon.ltsymbol.as_mut_ptr();
        let src = c_symbol.as_ptr();
        std::ptr::copy_nonoverlapping(src, dest, std::cmp::min(15, c_symbol.as_bytes().len()));
        mon.wx = 0;
        mon.ww = state.sw;
        let mut wa: xlib::XSetWindowAttributes = std::mem::zeroed();
        wa.override_redirect = 1;
        wa.background_pixmap = xlib::ParentRelative as u64;
        wa.event_mask = xlib::ButtonPressMask | xlib::ExposureMask;
        let valuemask = xlib::CWOverrideRedirect | xlib::CWBackPixmap | xlib::CWEventMask;
        let barwin = state.xwrapper.create_window(
            state.root,
            mon.wx,
            mon.by,
            mon.ww as u32,
            state.bh as u32,
            0,
            state.xwrapper.default_depth(state.screen),
            xlib::InputOutput as u32,
            state.xwrapper.default_visual(state.screen),
            valuemask as u64,
            &mut wa,
        );
        mon.barwin = barwin;
        state.xwrapper.map_raised(mon.barwin);
        state.mons.push(mon);
        state.selmon = state.mons.len() - 1;

        state.cursor[Cur::Normal as usize] = drw_cur_create(state, 68); 
        state.cursor[Cur::Resize as usize] = drw_cur_create(state, 120);
        state.cursor[Cur::Move as usize] = drw_cur_create(state, 52);
        
        state.wmcheckwin = state.xwrapper.create_simple_window(state.root, 0, 0, 1, 1, 0, 0, 0);
        let wmcheckwin_val = state.wmcheckwin.0;
        state.xwrapper.change_property(state.wmcheckwin, state.netatom[Net::WMCheck as usize], xlib::XA_WINDOW, 32,
            xlib::PropModeReplace, &wmcheckwin_val as *const u64 as *const c_uchar, 1);

        let dwm_name = CString::new("dwm").unwrap();
        state.xwrapper.change_property(state.wmcheckwin, state.netatom[Net::WMName as usize], xlib::XA_STRING, 8,
            xlib::PropModeReplace, dwm_name.as_ptr() as *const c_uchar, 3);
        state.xwrapper.change_property(state.root, state.netatom[Net::WMCheck as usize], xlib::XA_WINDOW, 32,
            xlib::PropModeReplace, &wmcheckwin_val as *const u64 as *const c_uchar, 1);

        state.xwrapper.change_property(state.root, state.netatom[Net::Supported as usize], xlib::XA_ATOM, 32,
            xlib::PropModeReplace, state.netatom.as_ptr() as *const c_uchar, Net::Last as i32);
        state.xwrapper.delete_property(state.root, state.netatom[Net::ClientList as usize]);

        let mut wa: xlib::XSetWindowAttributes = std::mem::zeroed();
        wa.cursor = *state.cursor[Cur::Normal as usize];
        wa.event_mask = (xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask
            | xlib::ButtonPressMask | xlib::PointerMotionMask | xlib::EnterWindowMask
            | xlib::LeaveWindowMask | xlib::StructureNotifyMask | xlib::PropertyChangeMask
            | xlib::KeyPressMask) as i64;
        state.xwrapper.change_window_attributes(state.root, (xlib::CWEventMask | xlib::CWCursor) as u64, &mut wa);
        state.xwrapper.select_input(state.root, wa.event_mask);

        // Update NumLockMask and grab global keys
        state.numlockmask = state.xwrapper.get_numlock_mask();
        let keys = grabkeys(state);
        let key_specs: Vec<KeySpec> = keys
            .iter()
            .map(|k| KeySpec {
                mask: k.mask,
                keysym: k.keysym,
            })
            .collect();
        state
            .xwrapper
            .grab_keys(state.root, state.numlockmask, &key_specs);

        state.handler[xlib::ButtonPress as usize] = Some(buttonpress);
        state.handler[xlib::MotionNotify as usize] = Some(motionnotify);
        state.handler[xlib::KeyPress as usize] = Some(keypress_wrapper);
        state.handler[xlib::MapRequest as usize] = Some(maprequest);
        state.handler[xlib::DestroyNotify as usize] = Some(destroy_notify);
        state.handler[xlib::EnterNotify as usize] = Some(enter_notify);

        focus(state, 0, None);
    }
}

fn die(s: &str) {
    eprintln!("{}", s);
    std::process::exit(1);
}

unsafe fn drw_cur_create(state: &mut GmuxState, shape: i32) -> *mut xlib::Cursor {
    let cur = ecalloc(1, std::mem::size_of::<xlib::Cursor>()) as *mut xlib::Cursor;
    unsafe {
        *cur = state.xwrapper.create_font_cursor(shape as u32);
    }
    cur
}

fn grabkeys(_state: &mut GmuxState) -> Vec<Key> {
    let mut keys: Vec<Key> = vec![];
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_p, func: spawn, arg: Arg { v: SyncVoidPtr(&Command::Dmenu as *const _ as *const c_void) } });
    keys.push(Key { mask: xlib::Mod1Mask | xlib::ShiftMask, keysym: keysym::XK_Return, func: spawn, arg: Arg { v: SyncVoidPtr(&Command::Terminal as *const _ as *const c_void) } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_b, func: togglebar, arg: Arg { i: 0 } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_j, func: focusstack, arg: Arg { i: 1 } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_k, func: focusstack, arg: Arg { i: -1 } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_i, func: incnmaster, arg: Arg { i: 1 } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_d, func: incnmaster, arg: Arg { i: -1 } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_h, func: setmfact, arg: Arg { f: -0.05 } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_l, func: setmfact, arg: Arg { f: 0.05 } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_Return, func: zoom, arg: Arg { i: 0 } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_Tab, func: view, arg: Arg { i: 0 } });
    keys.push(Key { mask: xlib::Mod1Mask | xlib::ShiftMask, keysym: keysym::XK_c, func: killclient, arg: Arg { i: 0 } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_t, func: setlayout, arg: Arg { v: SyncVoidPtr(&LAYOUTS[0] as *const _ as *const c_void) } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_f, func: setlayout, arg: Arg { v: SyncVoidPtr(&LAYOUTS[1] as *const _ as *const c_void) } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_m, func: setlayout, arg: Arg { v: SyncVoidPtr(&LAYOUTS[2] as *const _ as *const c_void) } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_space, func: setlayout, arg: Arg { v: SyncVoidPtr(null_mut()) } });
    keys.push(Key { mask: xlib::Mod1Mask | xlib::ShiftMask, keysym: keysym::XK_space, func: togglefloating, arg: Arg { i: 0 } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_0, func: view, arg: Arg { ui: !0 } });
    keys.push(Key { mask: xlib::Mod1Mask | xlib::ShiftMask, keysym: keysym::XK_0, func: tag, arg: Arg { ui: !0 } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_comma, func: focusmon, arg: Arg { i: -1 } });
    keys.push(Key { mask: xlib::Mod1Mask, keysym: keysym::XK_period, func: focusmon, arg: Arg { i: 1 } });
    keys.push(Key { mask: xlib::Mod1Mask | xlib::ShiftMask, keysym: keysym::XK_comma, func: tagmon, arg: Arg { i: -1 } });
    keys.push(Key { mask: xlib::Mod1Mask | xlib::ShiftMask, keysym: keysym::XK_period, func: tagmon, arg: Arg { i: 1 } });
    keys.push(Key { mask: xlib::Mod1Mask | xlib::ShiftMask, keysym: keysym::XK_q, func: quit, arg: Arg { i: 0 } });
    keys.push(Key { mask: 0, keysym: keysym::XK_Print, func: spawn, arg: Arg { v: SyncVoidPtr(&Command::Screenshot as *const _ as *const c_void) } });

    const TAGKEYS: &[(u32, u32)] = &[
        (keysym::XK_1, 0),
        (keysym::XK_2, 1),
        (keysym::XK_3, 2),
        (keysym::XK_4, 3),
        (keysym::XK_5, 4),
        (keysym::XK_6, 5),
        (keysym::XK_7, 6),
        (keysym::XK_8, 7),
        (keysym::XK_9, 8),
    ];

    for &(keysym, tag_idx) in TAGKEYS {
        keys.push(Key { mask: xlib::Mod1Mask, keysym, func: view, arg: Arg { ui: 1 << tag_idx } });
        keys.push(Key { mask: xlib::Mod1Mask | xlib::ControlMask, keysym, func: toggleview, arg: Arg { ui: 1 << tag_idx } });
        keys.push(Key { mask: xlib::Mod1Mask | xlib::ShiftMask, keysym, func: tag, arg: Arg { ui: 1 << tag_idx } });
        keys.push(Key { mask: xlib::Mod1Mask | xlib::ControlMask | xlib::ShiftMask, keysym, func: toggletag, arg: Arg { ui: 1 << tag_idx } });
    }

    keys
}

// Statically-known strings

const TAGS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

const TAG_MASK: u32 = (1 << TAGS.len()) - 1;
const LOCK_MASK: u32 = xlib::LockMask;


unsafe extern "C" fn spawn(_state: &mut GmuxState, arg: &Arg) {
    let cmd_ptr = unsafe { arg.v.0 };
    if cmd_ptr.is_null() {
        return;
    }
    let cmd = unsafe { &*(cmd_ptr as *const Command) };
    if unsafe { libc::fork() } == 0 {
        unsafe {
            libc::setsid();
            let shell = CString::new("/bin/sh").unwrap();
            let c_flag = CString::new("-c").unwrap();
            let cmd_str = CString::new(cmd.str()).unwrap();
            libc::execlp(shell.as_ptr(), shell.as_ptr(), c_flag.as_ptr(), cmd_str.as_ptr(), null_mut::<c_char>());
        }
    }
}
// stubs for all the functions in the keymap

unsafe extern "C" fn togglebar(_state: &mut GmuxState, _arg: &Arg) {}

unsafe extern "C" fn focusstack(state: &mut GmuxState, arg: &Arg) {
    let selmon_idx = state.selmon;
    let selmon = &mut state.mons[selmon_idx];
    if selmon.sel.is_none() {
        return;
    }
    let sel_idx = selmon.sel.unwrap();
    let mut c_idx: usize = 0;

    let visible_clients_indices: Vec<usize> = selmon.clients.iter().enumerate()
        .filter(|(_, c)| is_visible(c, selmon))
        .map(|(i, _)| i)
        .collect();
    if visible_clients_indices.is_empty() {
        return;
    }

    if let Some(pos) = visible_clients_indices.iter().position(|&i| i == sel_idx) {
        c_idx = if arg.i > 0 {
            visible_clients_indices[(pos + 1) % visible_clients_indices.len()]
        } else {
            visible_clients_indices[(pos + visible_clients_indices.len() - 1) % visible_clients_indices.len()]
        };
    } else if !visible_clients_indices.is_empty() {
        c_idx = visible_clients_indices[0];
    }
    
    if c_idx < selmon.clients.len() {
        focus(state, selmon_idx, Some(c_idx));
        state.restack(selmon_idx);
    }
}

unsafe extern "C" fn incnmaster(state: &mut GmuxState, arg: &Arg) {
    let selmon_idx = state.selmon;
    let selmon = &mut state.mons[selmon_idx];
    selmon.nmaster = std::cmp::max(selmon.nmaster + arg.i, 0);
    state.arrange(Some(selmon_idx));
}

unsafe extern "C" fn setmfact(state: &mut GmuxState, arg: &Arg) {
    let selmon_idx = state.selmon;
    let selmon = &mut state.mons[selmon_idx];
    if selmon.lt.get(selmon.sellt as usize).is_none() {
        return;
    }
    let f = if arg.f < 1.0 {
        arg.f + selmon.mfact
    } else {
        arg.f - 1.0
    };
    if f < 0.05 || f > 0.95 {
        return;
    }
    selmon.mfact = f;
    state.arrange(Some(selmon_idx));
}

unsafe extern "C" fn zoom(state: &mut GmuxState, _arg: &Arg) {
    let selmon_idx = state.selmon;
    if let Some(sel_idx) = state.mons[selmon_idx].sel {
        let c = &state.mons[selmon_idx].clients[sel_idx];
        if unsafe { (*(*state.mons[selmon_idx].lt.get_unchecked(state.mons[selmon_idx].sellt as usize))).arrange.is_none() } || c.isfloating {
            return;
        }

        let tiled_clients_indices: Vec<usize> = state.mons[selmon_idx].clients.iter().enumerate()
            .filter(|(_, cl)| !cl.isfloating && is_visible(cl, &state.mons[selmon_idx]))
            .map(|(i, _)| i)
            .collect();
        if let Some(pos) = tiled_clients_indices.iter().position(|&i| i == sel_idx) {
            if pos == 0 {
                if tiled_clients_indices.len() > 1 {
                    pop(state, selmon_idx, tiled_clients_indices[1]);
                }
            } else {
                pop(state, selmon_idx, sel_idx);
            }
        }
    }
}

unsafe extern "C" fn view(state: &mut GmuxState, arg: &Arg) {
    let selmon = &mut state.mons[state.selmon];
    if (arg.ui & TAG_MASK) != 0 {
        selmon.seltags = 0;
        selmon.tagset[selmon.seltags as usize] = arg.ui & TAG_MASK;
    }
    state.arrange(Some(state.selmon));
}

#[allow(dead_code)]
unsafe extern "C" fn killclient(state: &mut GmuxState, _arg: &Arg) {
    let selmon_idx = state.selmon;
    if let Some(sel_idx) = state.mons[selmon_idx].sel {
        let sel_client_win = state.mons[selmon_idx].clients[sel_idx].win;
        let sel_client_tags = state.mons[selmon_idx].clients[sel_idx].tags;
        let sel_client = Client { win: sel_client_win, tags: sel_client_tags, ..state.mons[selmon_idx].clients[sel_idx] };
        if !sendevent(state, &sel_client, state.wmatom[WM::Delete as usize]) {
            state.xwrapper.grab_server();
            state.xwrapper.set_error_handler(Some(xerror_dummy));
            state.xwrapper.set_close_down_mode(xlib::DestroyAll);
            state.xwrapper.kill_client(sel_client.win);
            state.xwrapper.sync(false);
            state.xwrapper.set_error_handler(Some(xerror));
            state.xwrapper.ungrab_server();
        }
    }
}


unsafe extern "C" fn xerror_dummy(_dpy: *mut xlib::Display, _ee: *mut xlib::XErrorEvent) -> c_int {
    0
}


fn sendevent(state: &mut GmuxState, c: &Client, proto: xlib::Atom) -> bool {
    let protocols = state.xwrapper.get_wm_protocols(c.win);
    if protocols.contains(&proto) {
        let mut data = [0; 5];
        data[0] = proto as i64;
        data[1] = xlib::CurrentTime as i64;
        state
            .xwrapper
            .send_client_message(c.win, state.wmatom[WM::Protocols as usize], data);
        true
    } else {
        false
    }
}


unsafe extern "C" fn setlayout(state: &mut GmuxState, arg: &Arg) {
    let v_ptr = unsafe { arg.v.0 };
    let selmon_idx = state.selmon;
    if v_ptr.is_null() {
        state.mons[selmon_idx].sellt ^= 1;
    } else {
        let sellt = state.mons[selmon_idx].sellt as usize;
        state.mons[selmon_idx].lt[sellt] = v_ptr as *const Layout;
    }
    let selmon = &mut state.mons[selmon_idx];
    let symbol = unsafe { CStr::from_ptr((*selmon.lt[selmon.sellt as usize]).symbol.0).to_str().unwrap() };
    let c_symbol = CString::new(symbol).unwrap();
    let dest = selmon.ltsymbol.as_mut_ptr();
    let src = c_symbol.as_ptr();
    unsafe {
        std::ptr::copy_nonoverlapping(src, dest, std::cmp::min(15, c_symbol.as_bytes().len()));
        selmon.ltsymbol[15] = 0;
    }
    if selmon.sel.is_some() {
        state.arrange(Some(selmon_idx));
    }
}

unsafe extern "C" fn togglefloating(_state: &mut GmuxState, _arg: &Arg) {}

unsafe extern "C" fn tag(state: &mut GmuxState, arg: &Arg) {
    let selmon_idx = state.selmon;
    if let Some(sel_idx) = state.mons[selmon_idx].sel {
        if (arg.ui & TAG_MASK) != 0 {
            state.mons[selmon_idx].clients[sel_idx].tags = arg.ui & TAG_MASK;
            state.arrange(Some(selmon_idx));
        }
    }
}

unsafe extern "C" fn toggleview(state: &mut GmuxState, arg: &Arg) {
    let selmon = &mut state.mons[state.selmon];
    let newtags = selmon.tagset[selmon.seltags as usize] ^ (arg.ui & TAG_MASK);

    if newtags != 0 {
        selmon.tagset[selmon.seltags as usize] = newtags;
        state.arrange(Some(state.selmon));
    }
}

unsafe extern "C" fn toggletag(state: &mut GmuxState, arg: &Arg) {
    let selmon_idx = state.selmon;
    if let Some(sel_idx) = state.mons[selmon_idx].sel {
        let newtags = state.mons[selmon_idx].clients[sel_idx].tags ^ (arg.ui & TAG_MASK);
        if newtags != 0 {
            state.mons[selmon_idx].clients[sel_idx].tags = newtags;
            state.arrange(Some(selmon_idx));
        }
    }
}

unsafe extern "C" fn focusmon(_state: &mut GmuxState, _arg: &Arg) {}

unsafe extern "C" fn tagmon(_state: &mut GmuxState, _arg: &Arg) {}

unsafe extern "C" fn quit(state: &mut GmuxState, _arg: &Arg) {
    state.running = 0;
}
static LAYOUTS: [Layout; 3] = [
    Layout { symbol: SyncPtr(b"[]=\0".as_ptr() as *const c_char), arrange: Some(tile) },
    Layout { symbol: SyncPtr(b"><>\0".as_ptr() as *const c_char), arrange: Some(monocle) },
    Layout { symbol: SyncPtr(b"[M]\0".as_ptr() as *const c_char), arrange: Some(monocle) },
];


unsafe extern "C" fn tile(state: &mut GmuxState, mon_idx: usize) {
    let mon = &state.mons[mon_idx];
    let tiled_client_indices: Vec<usize> = mon.clients.iter().enumerate()
        .filter(|(_, c)| !c.isfloating && is_visible(c, mon))
        .map(|(i, _)| i)
        .collect();
    let n = tiled_client_indices.len();
    if n == 0 {
        return;
    }

    let nmaster = mon.nmaster;
    let mfact = mon.mfact;
    let ww = mon.ww;
    let wh = mon.wh;
    let wx = mon.wx;
    let wy = mon.wy;

    let mw = if n > nmaster as usize {
        if nmaster > 0 {
            (ww as f32 * mfact) as i32
        } else {
            0
        }
    } else {
        ww
    };
    
    let mut my = 0;
    let mut ty = 0;

    for (i, &client_idx) in tiled_client_indices.iter().enumerate() {
        let client_bw = state.mons[mon_idx].clients[client_idx].bw;
        
        if i < nmaster as usize {
            let h = (wh - my) / (std::cmp::min(n, nmaster as usize) - i) as i32;
            resize(
                state,
                mon_idx,
                client_idx,
                wx,
                wy + my,
                mw - (2 * client_bw),
                h - (2 * client_bw),
                false,
            );
            if my + h < wh {
                my += h;
            }
        } else {
            let h = (wh - ty) / (n - i) as i32;
            resize(
                state,
                mon_idx,
                client_idx,
                wx + mw,
                wy + ty,
                ww - mw - (2 * client_bw),
                h - (2 * client_bw),
                false,
            );
            if ty + h < wh {
                ty += h;
            }
        }
    }
}


unsafe extern "C" fn monocle(state: &mut GmuxState, mon_idx: usize) {
    let mon = &state.mons[mon_idx];
    let tiled_client_indices: Vec<usize> = mon.clients.iter().enumerate()
        .filter(|(_, c)| !c.isfloating && is_visible(c, mon))
        .map(|(i, _)| i)
        .collect();

    let wx = mon.wx;
    let wy = mon.wy;
    let ww = mon.ww;
    let wh = mon.wh;

    for &client_idx in &tiled_client_indices {
        let client_bw = state.mons[mon_idx].clients[client_idx].bw;
        resize(state, mon_idx, client_idx, wx, wy, ww - 2 * client_bw, wh - 2 * client_bw, false);
    }
}


fn show_hide(state: &mut GmuxState, mon_idx: usize, stack: &[usize]) {
    for &c_idx in stack.iter().rev() {
        let c = &state.mons[mon_idx].clients[c_idx];
        if is_visible(c, &state.mons[c.mon_idx]) {
            state.xwrapper.move_window(c.win, c.x, c.y);
            if state.mons[c.mon_idx].lt.get(state.mons[c.mon_idx].sellt as usize).is_none()
                || c.isfloating && !c.isfullscreen
            {
                unsafe { resize(state, c.mon_idx, c_idx, c.x, c.y, c.w, c.h, false) };
            }
        }
    }

    for &c_idx in stack {
        let c = &state.mons[mon_idx].clients[c_idx];
        if !is_visible(c, &state.mons[c.mon_idx]) {
            state.xwrapper.move_window(c.win, -2 * client_width(c), c.y);
        }
    }
}


unsafe fn unmanage(state: &mut GmuxState, mon_idx: usize, client_idx: usize, destroyed: bool) {
    let client = if let Some(c) = detach(state, mon_idx, client_idx) {
        c
    } else {
        return;
    };
    detachstack(state, mon_idx, client_idx);
    
    if !destroyed {
        state.xwrapper.unmanage_window(client.win);
    }
    
    let new_sel = state.mons[mon_idx].sel;
    focus(state, mon_idx, new_sel);
    state.arrange(Some(mon_idx));
}


unsafe fn pop(state: &mut GmuxState, mon_idx: usize, client_idx: usize) {
    if let Some(client) = detach(state, mon_idx, client_idx) {
        let new_c_idx = attach(state, client);
        focus(state, mon_idx, Some(new_c_idx));
        state.arrange(Some(mon_idx));
    }
}


unsafe fn detach(state: &mut GmuxState, mon_idx: usize, client_idx: usize) -> Option<Client> {
    let mon = &mut state.mons[mon_idx];
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


unsafe fn attach(state: &mut GmuxState, c: Client) -> usize {
    let mon_idx = c.mon_idx;
    let mon = &mut state.mons[mon_idx];

    if let Some(sel) = mon.sel.as_mut() {
        *sel += 1;
    }
    for s in mon.stack.iter_mut() {
        *s += 1;
    }

    mon.clients.insert(0, c);
    0
}


unsafe fn attachstack(state: &mut GmuxState, mon_idx: usize, c_idx: usize) {
    state.mons[mon_idx].stack.insert(0, c_idx);
}


unsafe fn detachstack(state: &mut GmuxState, mon_idx: usize, c_idx: usize) {
    let mon = &mut state.mons[mon_idx];
    mon.stack.retain(|&x| x != c_idx);
}

// DestroyNotify handler to unmanage windows
unsafe extern "C" fn destroy_notify(state: &mut GmuxState, e: *mut xlib::XEvent) {
    let ev = unsafe { &*(e as *mut xlib::XDestroyWindowEvent) };
    if let Some((mon_idx, client_idx)) = wintoclient_idx(state, ev.window) {
        unmanage(state, mon_idx, client_idx, true);
    }
}


unsafe fn focus(state: &mut GmuxState, mon_idx: usize, c_idx: Option<usize>) {
    let selmon_idx = state.selmon;

    if let Some(old_sel_idx) = state.mons[selmon_idx].sel {
        if c_idx.is_none() || mon_idx != selmon_idx || old_sel_idx != c_idx.unwrap() {
            unfocus(state, selmon_idx, old_sel_idx, false);
        }
    }

    if let Some(idx) = c_idx {
        let new_mon_idx = state.mons[mon_idx].clients[idx].mon_idx;
        if new_mon_idx != selmon_idx {
            state.selmon = new_mon_idx;
        }
        let c_win = state.mons[new_mon_idx].clients[idx].win;
        let c_isurgent = state.mons[new_mon_idx].clients[idx].isurgent;
        if c_isurgent {
            // seturgent(c, 0);
        }
        // detachstack(c);
        // attachstack(c);
        grabbuttons(state, new_mon_idx, idx, true);
        let keys = grabkeys(state);
        let key_specs: Vec<KeySpec> = keys
            .iter()
            .map(|k| KeySpec {
                mask: k.mask,
                keysym: k.keysym,
            })
            .collect();
        state
            .xwrapper
            .grab_keys(c_win, state.numlockmask, &key_specs);
        // XSetWindowBorder(dpy, c->win, scheme[SchemeSel][ColBorder].pixel);
        state
            .xwrapper
            .set_input_focus(c_win, xlib::RevertToPointerRoot);
    } else {
        state
            .xwrapper
            .set_input_focus(state.root, xlib::RevertToPointerRoot);
        // XDeleteProperty(dpy, root, netatom[NetActiveWindow]);
    }
    state.mons[state.selmon].sel = c_idx;
    drawbars(state);
}



#[allow(dead_code)]
unsafe fn unfocus(state: &mut GmuxState, mon_idx: usize, c_idx: usize, setfocus: bool) {
    if c_idx >= state.mons[mon_idx].clients.len() {
        return;
    }
    let c_win = state.mons[mon_idx].clients[c_idx].win;
    grabbuttons(state, mon_idx, c_idx, false);
    state.xwrapper.ungrab_keys(c_win);
    // XSetWindowBorder(dpy, c->win, scheme[SchemeNorm][ColBorder].pixel);
    if setfocus {
        state
            .xwrapper
            .set_input_focus(state.root, xlib::RevertToPointerRoot);
        // XDeleteProperty(dpy, root, netatom[NetActiveWindow]);
    }
}


unsafe extern "C" fn buttonpress(state: &mut GmuxState, e: *mut xlib::XEvent) {
    let ev = unsafe { &mut (*(e as *mut xlib::XButtonPressedEvent)) };
    let mut _click = Clk::RootWin;
    let m = unsafe { state.wintomon(ev.window) };
    if m != state.selmon {
        unfocus(state, state.selmon, state.mons[state.selmon].sel.unwrap(), true);
        state.selmon = m;
        focus(state, m, None);
    }
    if ev.window == state.mons[state.selmon].barwin.0 {
        let mut i = 0;
        let mut x = 0;
        let mut arg = Arg { i: 0 };
        let selmon = &state.mons[state.selmon];

        for (tag_idx, &tag) in state.tags.iter().enumerate() {
            let w = state.xwrapper.text_width(tag) as i32;
            x += w as i32;
            if ev.x > x {
                i = tag_idx + 1;
            } else {
                break;
            }
        }
        if i < state.tags.len() {
            _click = Clk::TagBar;
            arg.ui = 1 << i;
        } else if ev.x < x + state.blw {
            _click = Clk::LtSymbol;
        } else if ev.x > selmon.ww - state.xwrapper.text_width(&unsafe { CStr::from_ptr(&state.stext as *const c_char).to_string_lossy() }) as i32 {
            _click = Clk::StatusText;
        } else {
            _click = Clk::WinTitle;
        }
    } else if let Some((mon_idx, client_idx)) = wintoclient_idx(state, ev.window) {
        focus(state, mon_idx, Some(client_idx));
        state.restack(state.selmon);
        state.xwrapper.allow_events(xlib::ReplayPointer);
        _click = Clk::ClientWin;
    }
}


unsafe extern "C" fn motionnotify(state: &mut GmuxState, e: *mut xlib::XEvent) {
    let ev = unsafe { &mut (*(e as *mut xlib::XMotionEvent)) };
    if ev.window != state.root.0 {
        return;
    }
    let m = state.recttomon(ev.x_root, ev.y_root, 1, 1);
    if m != state.selmon {
        if let Some(sel_idx) = state.mons[state.selmon].sel {
            unfocus(state, state.selmon, sel_idx, true);
        }
        state.selmon = m;
        focus(state, m, None);
    }
}

// Focus follows mouse when pointer enters a client window

unsafe extern "C" fn enter_notify(state: &mut GmuxState, e: *mut xlib::XEvent) {
    let ev = unsafe { &*(e as *mut xlib::XCrossingEvent) };
    // ignore non-normal or inferior events (same filtering as dwm)
    if (ev.mode != xlib::NotifyNormal as i32) || ev.detail == xlib::NotifyInferior as i32 {
        return;
    }
    // when entering root, ignore; bar handled elsewhere
    if ev.window == state.root.0 {
        return;
    }
    if let Some((mon_idx, client_idx)) = wintoclient_idx(state, ev.window) {
        if Some(client_idx) != state.mons[mon_idx].sel {
            focus(state, mon_idx, Some(client_idx));
        }
    }
}


unsafe extern "C" fn maprequest(state: &mut GmuxState, e: *mut xlib::XEvent) {
    let ev = unsafe { &mut (*(e as *mut xlib::XMapRequestEvent)) };
    if let Ok(mut wa) = state.xwrapper.get_window_attributes(Window(ev.window)) {
        if wa.override_redirect != 0 {
            return;
        }
        if unsafe { wintoclient_idx(state, ev.window) }.is_none() {
            unsafe { manage(state, ev.window, &mut wa) };
        }
    }
}


unsafe fn manage(state: &mut GmuxState, w: xlib::Window, wa: &mut xlib::XWindowAttributes) {
    let mut client = Client {
        win: Window(w),
        name: [0; 256],
        mina: 0.0,
        maxa: 0.0,
        x: wa.x,
        y: wa.y,
        w: wa.width,
        h: wa.height,
        oldx: wa.x,
        oldy: wa.y,
        oldw: wa.width,
        oldh: wa.height,
        basew: 0,
        baseh: 0,
        incw: 0,
        inch: 0,
        maxw: 0,
        maxh: 0,
        minw: 0,
        minh: 0,
        bw: BORDERPX,
        oldbw: wa.border_width,
        tags: 0,
        isfixed: false,
        isfloating: false,
        isurgent: false,
        neverfocus: false,
        oldstate: false,
        isfullscreen: false,
        next: std::ptr::null_mut(),
        snext: std::ptr::null_mut(),
        mon_idx: state.selmon,
    };
    // Assign to currently selected tag set so client is visible
    client.tags = state.mons[state.selmon].tagset[state.mons[state.selmon].seltags as usize];
    client.mon_idx = state.selmon;

    // updatetitle(c);
    // XGetTransientForHint
    // applyrules(c);

    unsafe {
        let win_copy = client.win;
        let c_idx = attach(state, client);
        attachstack(state, state.selmon, c_idx);
        // Recalculate tiling/layout with the newly added client
        state.arrange(Some(state.selmon));
        let sel_client_idx = state.mons[state.selmon].clients.iter().position(|c| c.win.0 == win_copy.0).unwrap();
        let sel_client = &state.mons[state.selmon].clients[sel_client_idx];
        state.xwrapper.map_window(sel_client.win);
        focus(state, state.selmon, Some(sel_client_idx));
    }

    // ... More logic to come ...
}


unsafe fn wintoclient_idx(state: &GmuxState, w: xlib::Window) -> Option<(usize, usize)> {
    for (mon_idx, m) in state.mons.iter().enumerate() {
        if let Some(client_idx) = m.clients.iter().position(|c| c.win.0 == w) {
            return Some((mon_idx, client_idx));
        }
    }
    None
}


unsafe fn getrootptr(state: &mut GmuxState, x: &mut i32, y: &mut i32) -> bool {
    let mut di = 0;
    let mut dui = 0;
    let mut dummy = 0;
    unsafe {
        xlib::XQueryPointer(
            state.xwrapper.dpy(),
            state.root.0,
            &mut dummy,
            &mut dummy,
            x,
            y,
            &mut di,
            &mut di,
            &mut dui,
        ) != 0
    }
}

fn intersect(x: i32, y: i32, w: i32, h: i32, m: &Monitor) -> i32 {
    std::cmp::max(
        0,
        std::cmp::min(x + w, m.wx + m.ww) - std::cmp::max(x, m.wx),
    ) * std::cmp::max(
        0,
        std::cmp::min(y + h, m.wy + m.wh) - std::cmp::max(y, m.wy),
    )
}



fn grabbuttons(_state: &mut GmuxState, _mon_idx: usize, _c_idx: usize, _focused: bool) {
    // For now, this is a stub
}


// Helper functions for layouts

unsafe fn resize(
    state: &mut GmuxState,
    mon_idx: usize,
    client_idx: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    _interact: bool,
) {
    let client = &mut state.mons[mon_idx].clients[client_idx];
    client.oldx = client.x;
    client.x = x;
    client.oldy = client.y;
    client.y = y;
    client.oldw = client.w;
    client.w = w;
    client.oldh = client.h;
    client.h = h;
    state.xwrapper.configure_window(
        client.win,
        client.x,
        client.y,
        client.w,
        client.h,
        BORDERPX,
    );
}


fn is_visible(c: &Client, m: &Monitor) -> bool {
    (c.tags & m.tagset[m.seltags as usize]) != 0
}


fn scan(state: &mut GmuxState) {
    if let Ok((_, _, wins)) = state.xwrapper.query_tree(state.root) {
        for &win in &wins {
            if let Ok(wa) = state.xwrapper.get_window_attributes(win) {
                if wa.override_redirect != 0 || state.xwrapper.get_transient_for_hint(win).is_some() {
                    continue;
                }
                // Potentially manage the window here if it's not already managed
            }
        }
    }
}


fn run(state: &mut GmuxState) {
    state.xwrapper.sync(false);
    while state.running != 0 {
        if let Some(mut ev) = state.xwrapper.next_event() {
            let event_type = unsafe { ev.get_type() };
            if (event_type as usize) < state.handler.len() {
                if let Some(h) = state.handler[event_type as usize] {
                    unsafe { h(state, &mut ev) };
                }
            }
        }
    }
}


fn cleanup(state: &mut GmuxState) {
    for i in 0..state.mons.len() {
        while !state.mons[i].stack.is_empty() {
            let c_idx = state.mons[i].stack.pop().unwrap();
            unsafe { unmanage(state, i, c_idx, false) };
        }
    }
    state.xwrapper.ungrab_key(state.root);
}


unsafe extern "C" fn keypress_wrapper(state: &mut GmuxState, e: *mut xlib::XEvent) {
    let keys = grabkeys(state);
    unsafe { keypress(state, &mut *e, keys.as_ptr(), keys.len()) };
}


unsafe extern "C" fn keypress(state: &mut GmuxState, e: &mut xlib::XEvent, keys: *const Key, keys_len: usize) {
    let ev = unsafe { &*(e as *mut xlib::XEvent as *mut xlib::XKeyEvent) };
    let keys_slice = unsafe { std::slice::from_raw_parts(keys, keys_len) };
    let keysym = state.xwrapper.keycode_to_keysym(ev.keycode) as u32;
    for key in keys_slice.iter() {
        if keysym == key.keysym
            && clean_mask(key.mask) == clean_mask(ev.state)
        {
            unsafe { (key.func)(state, &key.arg) };
        }
    }
}

fn clean_mask(mask: u32) -> u32 {
    mask & !(LOCK_MASK | xlib::Mod2Mask) & (xlib::ShiftMask | xlib::ControlMask | xlib::Mod1Mask | xlib::Mod3Mask | xlib::Mod4Mask | xlib::Mod5Mask)
}

// === NEW HELPERS ===

fn client_width(c: &Client) -> i32 {
    c.w + 2 * c.bw
}

fn update_client_pointers(state: &mut GmuxState) {
    for mon_idx in 0..state.mons.len() {
        let mon = &mut state.mons[mon_idx];
        if let Some(sel_ptr) = mon.sel {
            let sel_win = mon.clients[sel_ptr].win;
            mon.sel = mon.clients.iter().position(|c| c.win == sel_win);
        }

        let mut new_stack = Vec::new();
        for &stack_ptr in &mon.stack {
            let stack_win = mon.clients[stack_ptr].win;
            if let Some(new_idx) = mon.clients.iter().position(|c| c.win == stack_win) {
                new_stack.push(new_idx);
            }
        }
        mon.stack = new_stack;
    }
}

fn main() {
    println!("Starting gmux...");
    let mut state = GmuxState {
        stext: [0; 256],
        screen: 0,
        sw: 0,
        sh: 0,
        bh: 0,
        blw: 0,
        lrpad: 0,
        numlockmask: 0,
        handler: [None; xlib::LASTEvent as usize],
        wmatom: [0; WM::Last as usize],
        netatom: [0; Net::Last as usize],
        running: 1,
        cursor: [null_mut(); Cur::Last as usize],
        schemes: [SchemeId(0), SchemeId(0)],
        xwrapper: XWrapper::connect().expect("Failed to open display"),
        mons: Vec::new(),
        selmon: 0,
        root: Window(0),
        wmcheckwin: Window(0),
        xerror: false,
        tags: ["1", "2", "3", "4", "5", "6", "7", "8", "9"],
    };
    
    state.handler[xlib::KeyPress as usize] = Some(keypress_wrapper);
    
    unsafe {
        let locale = CString::new("").unwrap();
        if libc::setlocale(libc::LC_CTYPE, locale.as_ptr()).is_null()
            || xlib::XSupportsLocale() == 0
        {
            eprintln!("warning: no locale support");
        }

        state.xwrapper.set_error_handler(Some(xerror_ignore));
        
        // checkotherwm(&mut state);
        setup(&mut state);
        scan(&mut state);
        run(&mut state);
        
        cleanup(&mut state);
    }
}
