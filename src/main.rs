
use std::ffi::{CString, CStr};
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};
use std::ptr::{null_mut};
use x11::xlib;
use x11::xft;
use x11::keysym;

mod command;
mod drw;
use command::*;
use drw::{Drw, Scheme};

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
// doesn’t exit (mirrors dwm’s default behaviour).
#[allow(unused_variables)]
unsafe extern "C" fn xerror_ignore(dpy: *mut xlib::Display, ee: *mut xlib::XErrorEvent) -> c_int {
    // Always return 0 to tell Xlib that the error was handled.
    0
}

/* ========= configurable constants (similar to dwm's config.h) ========= */
const FONT: &str = "monospace:size=24"; // change size here to scale bar text
const BORDERPX: i32 = 2;
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

fn drawbar(state: &mut GmuxState, m: *mut Monitor) {
    let monitor = unsafe { &mut *m };

    // variables similar to dwm
    let mut x = 0;
    let mut tw = 0;

    // base bar background
    unsafe { state.drw.scheme = *state.scheme.add(Scheme::Norm as usize) };
    // filled, invert=true so rectangle uses ColBg, matching dwm background
    state.drw.rect(0, 0, monitor.ww as u32, state.bh as u32, true, true);

    // draw status text on selected monitor first so tags can overdraw if needed
    if m == state.selmon {
        unsafe { state.drw.scheme = *state.scheme.add(Scheme::Norm as usize) };
        let status = unsafe {
            CStr::from_ptr(state.stext.as_ptr()).to_str().unwrap_or("")
        };
        tw = state.drw.text_width(status) + 2; // 2px right padding
        state.drw.text(monitor.ww - tw as i32, 0, tw, state.bh as u32, 0, status, false);
    }

    // tags
    for i in 0..TAGS.len() {
        let sel = (monitor.tagset[monitor.seltags as usize] & 1 << i) != 0;
        unsafe { state.drw.scheme = *state.scheme.add(if sel { Scheme::Sel } else { Scheme::Norm } as usize) };
        let w = state.drw.text_width(TAGS[i]) + state.lrpad as u32;
        state.drw.text(x, 0, w, state.bh as u32, (state.lrpad / 2) as u32, TAGS[i], false);
        x += w as i32;
    }

    // layout symbol
    let ltsymbol = unsafe { CStr::from_ptr(monitor.ltsymbol.as_ptr()).to_str().unwrap_or("") };
    unsafe { state.drw.scheme = *state.scheme.add(Scheme::Norm as usize) };
    let w = state.drw.text_width(ltsymbol) + state.lrpad as u32;
    state.drw.text(x, 0, w, state.bh as u32, (state.lrpad / 2) as u32, ltsymbol, false);

    // map final pixmap
    state.drw.map(monitor.barwin, 0, 0, monitor.ww as u32, state.bh as u32);
}

fn drawbars(state: &mut GmuxState) {
    let mut m = state.mons;
    while !m.is_null() {
        drawbar(state, m);
        m = unsafe { (*m).next };
    }
}

fn updatenumlockmask(state: &mut GmuxState) {
    let mut_state = state as *mut GmuxState;
    unsafe {
        let mut i = 0;
        let modmap = xlib::XGetModifierMapping((*mut_state).dpy);
        if modmap.is_null() {
            return;
        }
        let max_keypermod = (*modmap).max_keypermod;
        let mut p = (*modmap).modifiermap;
        while i < 8 {
            let mut j = 0;
            while j < max_keypermod {
                if *p != 0 && xlib::XKeycodeToKeysym((*mut_state).dpy, *p, 0) as u32 == keysym::XK_Num_Lock {
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
#[derive(Copy, Clone)]
union Arg {
    i: i32,
    ui: u32,
    f: f32,
    v: SyncVoidPtr,
}

#[allow(dead_code)]
struct Button {
    click: Clk,
    mask: c_uint,
    button: c_uint,
    func: unsafe extern "C" fn(arg: &Arg),
    arg: Arg,
}

#[derive(Debug, Clone, Copy)]
struct Monitor {
    ltsymbol: [c_char; 16],
    mfact: f32,
    nmaster: i32,
    #[allow(dead_code)]
    num: i32,
    #[allow(dead_code)]
    by: i32,
    #[allow(dead_code)]
    mx: i32,
    #[allow(dead_code)]
    my: i32,
    #[allow(dead_code)]
    mw: i32,
    #[allow(dead_code)]
    mh: i32,
    wx: i32,
    wy: i32,
    ww: i32,
    wh: i32,
    seltags: u32,
    sellt: u32,
    tagset: [u32; 2],
    #[allow(dead_code)]
    showbar: bool,
    #[allow(dead_code)]
    topbar: bool,
    clients: *mut Client,
    sel: *mut Client,
    stack: *mut Client,
    next: *mut Monitor,
    barwin: xlib::Window,
    lt: [*const Layout; 2],
}

#[derive(Debug, Clone, Copy)]
struct Client {
    #[allow(dead_code)]
    name: [c_char; 256],
    #[allow(dead_code)]
    mina: f32,
    #[allow(dead_code)]
    maxa: f32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    oldx: i32,
    oldy: i32,
    oldw: i32,
    oldh: i32,
    #[allow(dead_code)]
    basew: i32,
    #[allow(dead_code)]
    baseh: i32,
    #[allow(dead_code)]
    incw: i32,
    #[allow(dead_code)]
    inch: i32,
    #[allow(dead_code)]
    maxw: i32,
    #[allow(dead_code)]
    maxh: i32,
    #[allow(dead_code)]
    minw: i32,
    #[allow(dead_code)]
    minh: i32,
    bw: i32,
    #[allow(dead_code)]
    oldbw: i32,
    tags: u32,
    #[allow(dead_code)]
    isfixed: bool,
    isfloating: bool,
    isurgent: bool,
    #[allow(dead_code)]
    neverfocus: bool,
    #[allow(dead_code)]
    oldstate: bool,
    #[allow(dead_code)]
    isfullscreen: bool,
    next: *mut Client,
    snext: *mut Client,
    mon: *mut Monitor,
    win: xlib::Window,
}

#[allow(dead_code)]
struct Key {
    mask: u32,
    keysym: u32,
    func: unsafe extern "C" fn(&mut GmuxState, &Arg),
    arg: Arg,
}

#[allow(dead_code)]
struct Layout {
    symbol: SyncPtr,
    arrange: unsafe extern "C" fn(&mut GmuxState, *mut Monitor),
}

#[allow(dead_code)]
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
    #[allow(dead_code)]
    stext: [c_char; 256],
    screen: c_int,
    sw: c_int,
    sh: c_int,
    bh: c_int,
    #[allow(dead_code)]
    blw: c_int,
    #[allow(dead_code)]
    lrpad: c_int,
    numlockmask: c_uint,
    handler: [Option<unsafe extern "C" fn(&mut GmuxState, *mut xlib::XEvent)>; xlib::LASTEvent as usize],
    wmatom: [xlib::Atom; WM::Last as usize],
    netatom: [xlib::Atom; Net::Last as usize],
    running: c_int,
    cursor: [*mut xlib::Cursor; Cur::Last as usize],
    #[allow(dead_code)]
    scheme: *mut *mut Clr,
    dpy: *mut xlib::Display,
    drw: Drw,
    mons: *mut Monitor,
    selmon: *mut Monitor,
    root: xlib::Window,
    wmcheckwin: xlib::Window,
    xerror: bool,
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
unsafe extern "C" fn xerror(_state: &mut GmuxState, _dpy: *mut xlib::Display, ee: *mut xlib::XErrorEvent) -> c_int {
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

#[allow(dead_code)]
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
        unsafe { xerror(state, dpy, ee) }
    }

    unsafe {
        xlib::XSetErrorHandler(Some(xerror_start_handler));
        xlib::XSelectInput(
            state.dpy,
            xlib::XDefaultRootWindow(state.dpy),
            xlib::SubstructureRedirectMask,
        );
        xlib::XSync(state.dpy, 0);
        
        let display_ptr = state.dpy as *mut GmuxState;
        std::ptr::write(display_ptr, std::ptr::read(state_ptr));

        if (*state_ptr).xerror {
            die("gmux: another window manager is already running");
        }

        xlib::XSetErrorHandler(Some(xerror_handler));
        xlib::XSync(state.dpy, 0);
    }
}

fn setup(state: &mut GmuxState) {
    unsafe {
        state.screen = xlib::XDefaultScreen(state.dpy);
        state.sw = xlib::XDisplayWidth(state.dpy, state.screen);
        state.sh = xlib::XDisplayHeight(state.dpy, state.screen);
        state.root = xlib::XRootWindow(state.dpy, state.screen);
        state.drw = Drw::create(state.dpy, state.screen, state.root, state.sw as u32, state.sh as u32);
        
        let fonts = &[FONT];
        if !state.drw.fontset_create(fonts) {
            die("no fonts could be loaded.");
        }

        // derive bar height and lrpad from font height like dwm
        if !state.drw.fonts.is_empty() {
            let h = state.drw.fonts[0].h as i32;
            state.bh = h + 2;
            state.lrpad = h + 2;
        }

        // color arrays are [ColFg, ColBg, ColBorder] following dwm
        let colors = &[
            &["#bbbbbb", "#222222", "#444444"], // SchemeNorm
            &["#eeeeee", "#005577", "#005577"], // SchemeSel
        ];
        state.scheme = ecalloc(colors.len(), std::mem::size_of::<*mut Clr>()) as *mut *mut Clr;
        for i in 0..colors.len() {
            *state.scheme.add(i) = state.drw.scm_create(colors[i]);
        }

        // initialise status text sample
        let sample_status = b"gmux";
        for (i, b) in sample_status.iter().enumerate() {
            state.stext[i] = *b as i8;
        }

        drawbars(state);

        let _utf8_string_name = CString::new("UTF8_STRING").unwrap();
        let wm_protocols_name = CString::new("WM_PROTOCOLS").unwrap();
        let wm_delete_name = CString::new("WM_DELETE_WINDOW").unwrap();
        let wm_state_name = CString::new("WM_STATE").unwrap();
        let wm_take_focus_name = CString::new("WM_TAKE_FOCUS").unwrap();
        let net_active_window_name = CString::new("_NET_ACTIVE_WINDOW").unwrap();
        let net_supported_name = CString::new("_NET_SUPPORTED").unwrap();
        let net_wm_name_name = CString::new("_NET_WM_NAME").unwrap();
        let net_wm_state_name = CString::new("_NET_WM_STATE").unwrap();
        let net_wm_check_name = CString::new("_NET_SUPPORTING_WM_CHECK").unwrap();
        let net_wm_fullscreen_name = CString::new("_NET_WM_STATE_FULLSCREEN").unwrap();
        let net_wm_window_type_name = CString::new("_NET_WM_WINDOW_TYPE").unwrap();
        let net_wm_window_type_dialog_name = CString::new("_NET_WM_WINDOW_TYPE_DIALOG").unwrap();
        let net_client_list_name = CString::new("_NET_CLIENT_LIST").unwrap();

        state.wmatom[WM::Protocols as usize] = unsafe { xlib::XInternAtom(state.dpy, wm_protocols_name.as_ptr(), 0) };
        state.wmatom[WM::Delete as usize] = unsafe { xlib::XInternAtom(state.dpy, wm_delete_name.as_ptr(), 0) };
        state.wmatom[WM::State as usize] = unsafe { xlib::XInternAtom(state.dpy, wm_state_name.as_ptr(), 0) };
        state.wmatom[WM::TakeFocus as usize] = unsafe { xlib::XInternAtom(state.dpy, wm_take_focus_name.as_ptr(), 0) };
        state.netatom[Net::ActiveWindow as usize] = unsafe { xlib::XInternAtom(state.dpy, net_active_window_name.as_ptr(), 0) };
        state.netatom[Net::Supported as usize] = unsafe { xlib::XInternAtom(state.dpy, net_supported_name.as_ptr(), 0) };
        state.netatom[Net::WMName as usize] = xlib::XInternAtom(state.dpy, net_wm_name_name.as_ptr(), 0);
        state.netatom[Net::WMState as usize] = xlib::XInternAtom(state.dpy, net_wm_state_name.as_ptr(), 0);
        state.netatom[Net::WMCheck as usize] = xlib::XInternAtom(state.dpy, net_wm_check_name.as_ptr(), 0);
        state.netatom[Net::WMFullscreen as usize] = xlib::XInternAtom(state.dpy, net_wm_fullscreen_name.as_ptr(), 0);
        state.netatom[Net::WMWindowType as usize] = xlib::XInternAtom(state.dpy, net_wm_window_type_name.as_ptr(), 0);
        state.netatom[Net::WMWindowTypeDialog as usize] = xlib::XInternAtom(state.dpy, net_wm_window_type_dialog_name.as_ptr(), 0);
        state.netatom[Net::ClientList as usize] = xlib::XInternAtom(state.dpy, net_client_list_name.as_ptr(), 0);

        // Create a monitor
        let mon = ecalloc(1, std::mem::size_of::<Monitor>()) as *mut Monitor;
        let monitor = &mut *mon;
        monitor.tagset = [1, 1];
        monitor.mfact = 0.55;
        monitor.nmaster = 1;
        monitor.showbar = true;
        monitor.topbar = true;
        // Calculate window area accounting for the bar height
        if monitor.showbar {
            monitor.by = if monitor.topbar { 0 } else { state.sh - state.bh };
            monitor.wy = if monitor.topbar { state.bh } else { 0 };
            monitor.wh = state.sh - state.bh;
        } else {
            monitor.by = -state.bh;
            monitor.wy = 0;
            monitor.wh = state.sh;
        }
        monitor.lt[0] = &LAYOUTS[0];
        monitor.lt[1] = &LAYOUTS[1];
        monitor.ltsymbol = [0; 16];
        let symbol = CStr::from_ptr(LAYOUTS[0].symbol.0).to_str().unwrap();
        let c_symbol = CString::new(symbol).unwrap();
        let dest = monitor.ltsymbol.as_mut_ptr();
        let src = c_symbol.as_ptr();
        std::ptr::copy_nonoverlapping(src, dest, std::cmp::min(15, c_symbol.as_bytes().len()));
        monitor.wx = 0;
        monitor.ww = state.sw;
        let mut wa: xlib::XSetWindowAttributes = std::mem::zeroed();
        wa.override_redirect = 1;
        wa.background_pixmap = xlib::ParentRelative as u64;
        wa.event_mask = xlib::ButtonPressMask | xlib::ExposureMask;
        monitor.barwin = xlib::XCreateWindow(
            state.dpy,
            state.root,
            monitor.wx,
            monitor.by,
            monitor.ww as u32,
            state.bh as u32,
            0,
            xlib::XDefaultDepth(state.dpy, state.screen),
            xlib::InputOutput as u32,
            xlib::XDefaultVisual(state.dpy, state.screen),
            (xlib::CWOverrideRedirect | xlib::CWBackPixmap | xlib::CWEventMask) as u64,
            &mut wa,
        );
        xlib::XMapRaised(state.dpy, monitor.barwin);
        state.mons = mon;
        state.selmon = mon;

        state.cursor[Cur::Normal as usize] = drw_cur_create(state, 68); 
        state.cursor[Cur::Resize as usize] = drw_cur_create(state, 120);
        state.cursor[Cur::Move as usize] = drw_cur_create(state, 52);
        
        state.wmcheckwin = xlib::XCreateSimpleWindow(state.dpy, state.root, 0, 0, 1, 1, 0, 0, 0);
        let wmcheckwin_val = state.wmcheckwin;
        xlib::XChangeProperty(state.dpy, state.wmcheckwin, state.netatom[Net::WMCheck as usize], xlib::XA_WINDOW, 32,
            xlib::PropModeReplace, &wmcheckwin_val as *const u64 as *const c_uchar, 1);

        let dwm_name = CString::new("dwm").unwrap();
        xlib::XChangeProperty(state.dpy, state.wmcheckwin, state.netatom[Net::WMName as usize], xlib::XA_STRING, 8,
            xlib::PropModeReplace, dwm_name.as_ptr() as *const c_uchar, 3);
        xlib::XChangeProperty(state.dpy, state.root, state.netatom[Net::WMCheck as usize], xlib::XA_WINDOW, 32,
            xlib::PropModeReplace, &wmcheckwin_val as *const u64 as *const c_uchar, 1);

        xlib::XChangeProperty(state.dpy, state.root, state.netatom[Net::Supported as usize], xlib::XA_ATOM, 32,
            xlib::PropModeReplace, state.netatom.as_ptr() as *const c_uchar, Net::Last as i32);
        xlib::XDeleteProperty(state.dpy, state.root, state.netatom[Net::ClientList as usize]);

        let mut wa: xlib::XSetWindowAttributes = std::mem::zeroed();
        wa.cursor = *state.cursor[Cur::Normal as usize];
        wa.event_mask = (xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask
            | xlib::ButtonPressMask | xlib::PointerMotionMask | xlib::EnterWindowMask
            | xlib::LeaveWindowMask | xlib::StructureNotifyMask | xlib::PropertyChangeMask
            | xlib::KeyPressMask) as i64;
        xlib::XChangeWindowAttributes(state.dpy, state.root, (xlib::CWEventMask | xlib::CWCursor) as u64, &mut wa);
        xlib::XSelectInput(state.dpy, state.root, wa.event_mask);

        // Update NumLockMask and grab global keys
        updatenumlockmask(state);
        unsafe { register_grabkeys(state); }

        state.handler[xlib::ButtonPress as usize] = Some(buttonpress);
        state.handler[xlib::MotionNotify as usize] = Some(motionnotify);
        state.handler[xlib::KeyPress as usize] = Some(keypress_wrapper);
        state.handler[xlib::MapRequest as usize] = Some(maprequest);
        state.handler[xlib::DestroyNotify as usize] = Some(destroy_notify);
        state.handler[xlib::EnterNotify as usize] = Some(enter_notify);

        focus(state, null_mut());
    }
}

fn die(s: &str) {
    eprintln!("{}", s);
    std::process::exit(1);
}

unsafe fn drw_cur_create(state: &mut GmuxState, shape: i32) -> *mut xlib::Cursor {
    let cur = ecalloc(1, std::mem::size_of::<xlib::Cursor>()) as *mut xlib::Cursor;
    unsafe {
        *cur = xlib::XCreateFontCursor(state.drw.dpy, shape as c_uint);
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
    keys
}

// Statically-known strings
#[allow(dead_code)]
const TAGS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];
#[allow(dead_code)]
const TAG_MASK: u32 = (1 << TAGS.len()) - 1;
const LOCK_MASK: u32 = xlib::LockMask;

#[allow(dead_code)]
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
#[allow(dead_code)]
unsafe extern "C" fn togglebar(_state: &mut GmuxState, _arg: &Arg) {}
#[allow(dead_code)]
unsafe extern "C" fn focusstack(state: &mut GmuxState, arg: &Arg) {
    let selmon = &mut *state.selmon;
    if selmon.sel.is_null() {
        return;
    }
    let mut c: *mut Client = std::ptr::null_mut();
    if arg.i > 0 {
        // forward direction
        c = (*selmon.sel).next;
        while !c.is_null() && !is_visible(&*c) {
            c = (*c).next;
        }
        if c.is_null() {
            // wrap around
            c = selmon.clients;
            while !c.is_null() && !is_visible(&*c) {
                c = (*c).next;
            }
        }
    } else {
        // backward direction – we need previous visible client
        let mut i_ptr = selmon.clients;
        while i_ptr != selmon.sel {
            if is_visible(&*i_ptr) {
                c = i_ptr;
            }
            i_ptr = (*i_ptr).next;
        }
        if c.is_null() {
            // none before sel, iterate from sel onwards to end
            while !i_ptr.is_null() {
                if is_visible(&*i_ptr) {
                    c = i_ptr;
                }
                i_ptr = (*i_ptr).next;
            }
        }
    }
    if !c.is_null() {
        focus(state, c);
        restack(state, state.selmon);
    }
}
#[allow(dead_code)]
unsafe extern "C" fn incnmaster(state: &mut GmuxState, arg: &Arg) {
    let selmon = unsafe { &mut *state.selmon };
    selmon.nmaster = std::cmp::max(selmon.nmaster + unsafe { arg.i }, 0);
    unsafe { arrange(state, state.selmon) };
}
#[allow(dead_code)]
unsafe extern "C" fn setmfact(state: &mut GmuxState, arg: &Arg) {
    let selmon = unsafe { &mut *state.selmon };
    if (unsafe { &*selmon.lt[selmon.sellt as usize] }.arrange as usize) == 0 {
        return;
    }
    let f = unsafe {
        if arg.f < 1.0 {
            arg.f + selmon.mfact
        } else {
            arg.f - 1.0
        }
    };
    if f < 0.05 || f > 0.95 {
        return;
    }
    selmon.mfact = f;
    unsafe { arrange(state, state.selmon) };
}
#[allow(dead_code)]
unsafe extern "C" fn zoom(state: &mut GmuxState, _arg: &Arg) {
    let selmon = unsafe { &mut *state.selmon };
    let c = selmon.sel;
    if c.is_null() {
        return;
    }
    if (unsafe { &*selmon.lt[selmon.sellt as usize] }.arrange as usize) == 0 || unsafe { (*c).isfloating } {
        return;
    }
    if c == unsafe { next_tiled(selmon.clients) } {
        let next = unsafe { next_tiled((*c).next) };
        if next.is_null() {
            return;
        }
        unsafe { pop(state, next) };
    } else {
        unsafe { pop(state, c) };
    }
}
#[allow(dead_code)]
unsafe extern "C" fn view(_state: &mut GmuxState, _arg: &Arg) {}
#[allow(dead_code)]
unsafe extern "C" fn killclient(state: &mut GmuxState, _arg: &Arg) {
    let selmon = &*state.selmon;
    if selmon.sel.is_null() {
        return;
    }
    if !sendevent(state, selmon.sel, state.wmatom[WM::Delete as usize]) {
        xlib::XGrabServer(state.dpy);
        // xlib::XSetErrorHandler(xerrordummy);
        xlib::XSetCloseDownMode(state.dpy, xlib::DestroyAll);
        xlib::XKillClient(state.dpy, (*selmon.sel).win);
        xlib::XSync(state.dpy, 0);
        // xlib::XSetErrorHandler(xerror);
        xlib::XUngrabServer(state.dpy);
    }
}

#[allow(dead_code)]
unsafe fn sendevent(state: &mut GmuxState, c: *mut Client, proto: xlib::Atom) -> bool {
    let mut n = 0;
    let mut protocols: *mut xlib::Atom = std::ptr::null_mut();
    let mut exists = false;

    if xlib::XGetWMProtocols(state.dpy, (*c).win, &mut protocols, &mut n) != 0 {
        let mut i = n;
        while !exists && i > 0 {
            i -= 1;
            exists = *protocols.offset(i as isize) == proto;
        }
        xlib::XFree(protocols as *mut c_void);
    }

    if exists {
        let mut ev: xlib::XEvent = std::mem::zeroed();
        ev.client_message.type_ = xlib::ClientMessage;
        ev.client_message.window = (*c).win;
        ev.client_message.message_type = state.wmatom[WM::Protocols as usize];
        ev.client_message.format = 32;
        ev.client_message.data.set_long(0, proto as i64);
        ev.client_message.data.set_long(1, xlib::CurrentTime as i64);
        xlib::XSendEvent(state.dpy, (*c).win, 0, xlib::NoEventMask, &mut ev);
    }

    exists
}

#[allow(dead_code)]
unsafe extern "C" fn setlayout(state: &mut GmuxState, arg: &Arg) {
    let v_ptr = unsafe { arg.v.0 };
    if v_ptr.is_null() {
        let selmon = unsafe { &mut *state.selmon };
        selmon.sellt ^= 1;
    } else {
        let selmon = unsafe { &mut *state.selmon };
        selmon.lt[selmon.sellt as usize] = v_ptr as *const Layout;
    }

    let selmon = unsafe { &*state.selmon };
    let symbol = unsafe { CStr::from_ptr((*selmon.lt[selmon.sellt as usize]).symbol.0).to_str().unwrap() };
    let c_symbol = CString::new(symbol).unwrap();
    let dest = unsafe { (*state.selmon).ltsymbol.as_mut_ptr() };
    let src = c_symbol.as_ptr();
    unsafe {
        std::ptr::copy_nonoverlapping(src, dest, std::cmp::min(15, c_symbol.as_bytes().len()));
        (*state.selmon).ltsymbol[15] = 0;
    }

    if !unsafe { (*state.selmon).sel.is_null() } {
        unsafe { arrange(state, state.selmon) };
    } else {
        // drawbar(state.selmon);
    }
}
#[allow(dead_code)]
unsafe extern "C" fn togglefloating(_state: &mut GmuxState, _arg: &Arg) {}
#[allow(dead_code)]
unsafe extern "C" fn tag(_state: &mut GmuxState, _arg: &Arg) {}
#[allow(dead_code)]
unsafe extern "C" fn focusmon(_state: &mut GmuxState, _arg: &Arg) {}
#[allow(dead_code)]
unsafe extern "C" fn tagmon(_state: &mut GmuxState, _arg: &Arg) {}
#[allow(dead_code)]
unsafe extern "C" fn quit(state: &mut GmuxState, _arg: &Arg) {
    state.running = 0;
}
static LAYOUTS: [Layout; 3] = [
    Layout { symbol: SyncPtr(b"[]=\0".as_ptr() as *const c_char), arrange: tile },
    Layout { symbol: SyncPtr(b"><>\0".as_ptr() as *const c_char), arrange: monocle },
    Layout { symbol: SyncPtr(b"[M]\0".as_ptr() as *const c_char), arrange: monocle },
];

#[allow(dead_code)]
unsafe extern "C" fn tile(state: &mut GmuxState, m: *mut Monitor) {
    let mon = unsafe { &mut *m };
    let mut n = 0;
    let mut c = unsafe { next_tiled(mon.clients) };
    while !c.is_null() {
        n += 1;
        c = unsafe { next_tiled((*c).next) };
    }
    if n == 0 {
        return;
    }

    let mw = if n > mon.nmaster {
        if mon.nmaster > 0 {
            (mon.ww as f32 * mon.mfact) as i32
        } else {
            0
        }
    } else {
        mon.ww
    };

    let mut i = 0;
    let mut my = 0;
    let mut ty = 0;
    c = unsafe { next_tiled(mon.clients) };
    while !c.is_null() {
        let client = unsafe { &mut *c };
        if i < mon.nmaster {
            let h = (mon.wh - my) / (std::cmp::min(n, mon.nmaster) - i);
            unsafe {
                resize(
                    state,
                    client,
                    mon.wx,
                    mon.wy + my,
                    mw - (2 * client.bw),
                    h - (2 * client.bw),
                    false,
                )
            };
            if my + client.h < mon.wh {
                my += client.h;
            }
        } else {
            let h = (mon.wh - ty) / (n - i);
            unsafe {
                resize(
                    state,
                    client,
                    mon.wx + mw,
                    mon.wy + ty,
                    mon.ww - mw - (2 * client.bw),
                    h - (2 * client.bw),
                    false,
                )
            };
            if ty + client.h < mon.wh {
                ty += client.h;
            }
        }
        i += 1;
        c = unsafe { next_tiled(client.next) };
    }
}

#[allow(dead_code)]
unsafe extern "C" fn monocle(state: &mut GmuxState, m: *mut Monitor) {
    let mon = unsafe { &mut *m };
    let mut n = 0;
    let mut c = unsafe { next_tiled(mon.clients) };
    while !c.is_null() {
        n += 1;
        c = unsafe { next_tiled((&*c).next) };
    }

    if n > 0 {
        let symbol = format!("[{}]", n);
        let c_symbol = CString::new(symbol).unwrap();
        unsafe {
            let dest = mon.ltsymbol.as_mut_ptr();
            let src = c_symbol.as_ptr();
            std::ptr::copy_nonoverlapping(src, dest, std::cmp::min(15, c_symbol.as_bytes().len()));
            mon.ltsymbol[15] = 0;
        }
    }

    c = unsafe { next_tiled(mon.clients) };
    while !c.is_null() {
        let client = unsafe { &mut *c };
        unsafe {
            resize(
                state,
                client,
                mon.wx,
                mon.wy,
                mon.ww - 2 * client.bw,
                mon.wh - 2 * client.bw,
                false,
            )
        };
        c = unsafe { next_tiled(client.next) };
    }
}

#[allow(dead_code)]
unsafe fn arrange(state: &mut GmuxState, m: *mut Monitor) {
    if !m.is_null() {
        show_hide(state, unsafe { (*m).stack });
    } else {
        let mut mon = state.mons;
        while !mon.is_null() {
            show_hide(state, unsafe { (*mon).stack });
            mon = unsafe { (*mon).next };
        }
    }
    if !m.is_null() {
        unsafe { arrange_mon(state, m) };
        unsafe { restack(state, m) };
    } else {
        let mut mon = state.mons;
        while !mon.is_null() {
            unsafe { arrange_mon(state, mon) };
            mon = unsafe { (*mon).next };
        }
    }
}

#[allow(dead_code)]
unsafe fn arrange_mon(state: &mut GmuxState, m: *mut Monitor) {
    if m.is_null() {
        return;
    }
    // call current layout's arrange
    let mon = unsafe { &mut *m };
    if mon.lt[mon.sellt as usize].is_null() {
        return;
    }
    let layout = unsafe { &*mon.lt[mon.sellt as usize] };
    (layout.arrange)(state, m);
}

#[allow(dead_code)]
unsafe fn show_hide(state: &mut GmuxState, c: *mut Client) {
    if c.is_null() {
        return;
    }
    if is_visible(&*c) {
        // Show clients top down
        unsafe { xlib::XMoveWindow(state.dpy, (*c).win, (*c).x, (*c).y); }
        if ((*(*c).mon).lt[(*(*c).mon).sellt as usize]).is_null()
            || (*c).isfloating
                && !(*c).isfullscreen
        {
            // For floating or unmanaged layout, ensure correct size
            unsafe { resize(state, c, (*c).x, (*c).y, (*c).w, (*c).h, false); }
        }
        show_hide(state, (*c).snext);
    } else {
        // Hide clients bottom up
        show_hide(state, (*c).snext);
        unsafe { xlib::XMoveWindow(state.dpy, (*c).win, -2 * client_width(&*c), (*c).y); }
    }
}

#[allow(dead_code)]
unsafe fn restack(state: &mut GmuxState, m: *mut Monitor) {
    if m.is_null() {
        return;
    }
    drawbar(state, m);
    if (*m).sel.is_null() {
        return;
    }
    if (*(*m).sel).isfloating || ((*m).lt[(*m).sellt as usize]).is_null() {
        xlib::XRaiseWindow(state.dpy, (*(*m).sel).win);
    }

    if !((*m).lt[(*m).sellt as usize]).is_null() {
        let mut wc: xlib::XWindowChanges = std::mem::zeroed();
        wc.stack_mode = xlib::Below as i32;
        wc.sibling = (*m).barwin;
        let mut c = (*m).stack;
        while !c.is_null() {
            if !(*c).isfloating && is_visible(&*c) {
                xlib::XConfigureWindow(state.dpy, (*c).win, (xlib::CWSibling | xlib::CWStackMode) as u32, &mut wc);
                wc.sibling = (*c).win;
            }
            c = (*c).snext;
        }
    }
    xlib::XSync(state.dpy, 0);
}

#[allow(dead_code)]
unsafe fn unmanage(state: &mut GmuxState, c: *mut Client, _destroyed: bool) {
    if c.is_null() {
        return;
    }
    detach(state, c);
    detachstack(state, c);
    arrange(state, (*c).mon);
    xlib::XUngrabButton(state.dpy, xlib::AnyButton as u32, xlib::AnyModifier as u32, (*c).win);
    xlib::XSetWindowBorder(state.dpy, (*c).win, 0);
    xlib::XRemoveFromSaveSet(state.dpy, (*c).win);
    xlib::XDestroyWindow(state.dpy, (*c).win);
}

#[allow(dead_code)]
unsafe fn pop(state: &mut GmuxState, c: *mut Client) {
    unsafe { detach(state, c) };
    unsafe { attach(state, c) };
    unsafe { focus(state, c) };
    unsafe { arrange(state, (*c).mon) };
}

#[allow(dead_code)]
unsafe fn detach(_state: &mut GmuxState, c: *mut Client) {
    let mut tc = unsafe { &mut (*(*c).mon).clients };
    while !(*tc).is_null() && *tc != c {
        tc = unsafe { &mut (**tc).next };
    }
    *tc = unsafe { (*c).next };
}

#[allow(dead_code)]
unsafe fn attach(_state: &mut GmuxState, c: *mut Client) {
    let mon = unsafe { &mut *(*c).mon };
    unsafe { (*c).next = mon.clients };
    mon.clients = c;
}

#[allow(dead_code)]
unsafe fn attachstack(_state: &mut GmuxState, c: *mut Client) {
    let mon = unsafe { &mut *(*c).mon };
    unsafe { (*c).snext = mon.stack };
    mon.stack = c;
}

#[allow(dead_code)]
unsafe fn detachstack(_state: &mut GmuxState, c: *mut Client) {
    let mon = &mut *(*c).mon;
    let mut tc = &mut mon.stack;
    while !(*tc).is_null() && *tc != c {
        tc = &mut (**tc).snext;
    }
    *tc = (*c).snext;
}

// DestroyNotify handler to unmanage windows
unsafe extern "C" fn destroy_notify(state: &mut GmuxState, e: *mut xlib::XEvent) {
    let ev = &*(e as *mut xlib::XDestroyWindowEvent);
    let c = wintoclient(state, ev.window);
    if !c.is_null() {
        unmanage(state, c, true);
    }
}

#[allow(dead_code)]
unsafe fn focus(state: &mut GmuxState, c: *mut Client) {
    if c.is_null() || !is_visible(unsafe { &*c }) {
        let mut temp_c = unsafe { (*state.selmon).stack };
        while !temp_c.is_null() && !is_visible(unsafe { &*temp_c }) {
            temp_c = unsafe { (*temp_c).snext };
        }
    }
    let selmon = unsafe { &mut *state.selmon };
    if !selmon.sel.is_null() && selmon.sel != c {
        unsafe { unfocus(state, selmon.sel, false) };
    }
    if !c.is_null() {
        if unsafe { (*c).mon } != state.selmon {
            state.selmon = unsafe { (*c).mon };
        }
        if unsafe { (*c).isurgent } {
            // seturgent(c, 0);
        }
        // detachstack(c);
        // attachstack(c);
        grabbuttons(state, c, true);
        updatenumlockmask(state);
        let keys = grabkeys(state);
        let modifiers = [0, xlib::LockMask, state.numlockmask, state.numlockmask | xlib::LockMask];
        for key in keys.iter() {
            unsafe {
                let code = xlib::XKeysymToKeycode(state.dpy, key.keysym as u64);
                if code != 0 {
                    for modifier in modifiers.iter() {
                        xlib::XGrabKey(
                            state.dpy,
                            code as c_int,
                            key.mask | *modifier,
                            (*c).win,
                            1,
                            xlib::GrabModeAsync,
                            xlib::GrabModeAsync,
                        );
                    }
                }
            }
        }
        // XSetWindowBorder(dpy, c->win, scheme[SchemeSel][ColBorder].pixel);
        unsafe { xlib::XSetInputFocus(state.dpy, (*c).win, xlib::RevertToPointerRoot, xlib::CurrentTime) };
    } else {
        unsafe {
            xlib::XSetInputFocus(state.dpy, state.root, xlib::RevertToPointerRoot, xlib::CurrentTime)
        };
        // XDeleteProperty(dpy, root, netatom[NetActiveWindow]);
    }
    selmon.sel = c;
    drawbars(state);
}


#[allow(dead_code)]
unsafe fn unfocus(state: &mut GmuxState, c: *mut Client, setfocus: bool) {
    if c.is_null() {
        return;
    }
    grabbuttons(state, c, false);
    unsafe { xlib::XUngrabKey(state.dpy, xlib::AnyKey, xlib::AnyModifier, (*c).win) };
    // XSetWindowBorder(dpy, c->win, scheme[SchemeNorm][ColBorder].pixel);
    if setfocus {
        unsafe {
            xlib::XSetInputFocus(state.dpy, state.root, xlib::RevertToPointerRoot, xlib::CurrentTime)
        };
        // XDeleteProperty(dpy, root, netatom[NetActiveWindow]);
    }
}

#[allow(dead_code)]
unsafe extern "C" fn buttonpress(state: &mut GmuxState, e: *mut xlib::XEvent) {
    let ev = unsafe { &mut (*(e as *mut xlib::XButtonPressedEvent)) };
    let mut _click = Clk::RootWin;
    let m = unsafe { wintomon(state, ev.window) };
    if m != state.selmon {
        unsafe { unfocus(state, (*state.selmon).sel, true) };
        state.selmon = m;
        unsafe { focus(state, null_mut()) };
    }
    if ev.window == unsafe { (*state.selmon).barwin } {
        let _i = 0;
        let _x = 0;
        let _arg = Arg { i: 0 };
        // let tags = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];
        // for tag in tags.iter() {
        //     x += TEXTW(tag);
        //     if ev.x > x {
        //         i += 1;
        //     }
        // }
        // if i < tags.len() {
        //     click = Clk::TagBar;
        //     arg.ui = 1 << i;
        // } else if ev.x < x + blw {
        //     click = Clk::LtSymbol;
        // } else if ev.x > unsafe { (*state.selmon).ww } - TEXTW(stext) {
        //     click = Clk::StatusText;
        // } else {
        //     click = Clk::WinTitle;
        // }
    } else {
        let c = unsafe { wintoclient(state, ev.window) };
        if !c.is_null() {
            unsafe { focus(state, c) };
            unsafe { restack(state, state.selmon) };
            unsafe { xlib::XAllowEvents(state.dpy, xlib::ReplayPointer, xlib::CurrentTime) };
            _click = Clk::ClientWin;
        }
    }
}

#[allow(dead_code)]
unsafe extern "C" fn motionnotify(state: &mut GmuxState, e: *mut xlib::XEvent) {
    let ev = unsafe { &mut (*(e as *mut xlib::XMotionEvent)) };
    if ev.window != state.root {
        return;
    }
    let m = unsafe { recttomon(state, ev.x_root, ev.y_root, 1, 1) };
    if m != state.selmon {
        unsafe { unfocus(state, (*state.selmon).sel, true) };
        state.selmon = m;
        unsafe { focus(state, null_mut()) };
    }
}

// Focus follows mouse when pointer enters a client window
#[allow(dead_code)]
unsafe extern "C" fn enter_notify(state: &mut GmuxState, e: *mut xlib::XEvent) {
    let ev = &*(e as *mut xlib::XCrossingEvent);
    // ignore non-normal or inferior events (same filtering as dwm)
    if (ev.mode != xlib::NotifyNormal as i32) || ev.detail == xlib::NotifyInferior as i32 {
        return;
    }
    // when entering root, ignore; bar handled elsewhere
    if ev.window == state.root {
        return;
    }
    let c = wintoclient(state, ev.window);
    if !c.is_null() && c != (*state.selmon).sel {
        focus(state, c);
    }
}

#[allow(dead_code)]
unsafe extern "C" fn maprequest(state: &mut GmuxState, e: *mut xlib::XEvent) {
    let ev = unsafe { &mut (*(e as *mut xlib::XMapRequestEvent)) };
    let mut wa: xlib::XWindowAttributes = unsafe { std::mem::zeroed() };
    if unsafe { xlib::XGetWindowAttributes(state.dpy, ev.window, &mut wa) } == 0 {
        return;
    }
    if wa.override_redirect != 0 {
        return;
    }
    if unsafe { wintoclient(state, ev.window) }.is_null() {
        unsafe { manage(state, ev.window, &mut wa) };
    }
}

#[allow(dead_code)]
unsafe fn manage(state: &mut GmuxState, w: xlib::Window, wa: &mut xlib::XWindowAttributes) {
    let c = ecalloc(1, std::mem::size_of::<Client>()) as *mut Client;
    let client = unsafe { &mut *c };
    client.win = w;
    client.x = wa.x;
    client.y = wa.y;
    client.w = wa.width;
    client.h = wa.height;
    client.oldx = wa.x;
    client.oldy = wa.y;
    client.oldw = wa.width;
    client.oldh = wa.height;
    client.oldbw = wa.border_width;
    // Assign to currently selected tag set so client is visible
    client.tags = unsafe { (*state.selmon).tagset[(*state.selmon).seltags as usize] };
    client.mon = state.selmon;

    // updatetitle(c);
    // XGetTransientForHint
    // applyrules(c);

    unsafe {
        attach(state, c);
        attachstack(state, c);
        // Recalculate tiling/layout with the newly added client
        unsafe { arrange(state, state.selmon) };
    }

    // ... More logic to come ...

    unsafe {
        xlib::XMapWindow(state.dpy, client.win);
        focus(state, c);
    }
}

#[allow(dead_code)]
unsafe fn wintomon(state: &mut GmuxState, w: xlib::Window) -> *mut Monitor {
    let mut x = 0;
    let mut y = 0;
    if w == state.root && unsafe { getrootptr(state, &mut x, &mut y) } {
        return unsafe { recttomon(state, x, y, 1, 1) };
    }
    let mut m = state.mons;
    while !m.is_null() {
        if w == unsafe { (*m).barwin } {
            return m;
        }
        m = unsafe { (*m).next };
    }
    let c = unsafe { wintoclient(state, w) };
    if !c.is_null() {
        return unsafe { (*c).mon };
    }
    state.selmon
}


#[allow(dead_code)]
unsafe fn wintoclient(state: &mut GmuxState, w: xlib::Window) -> *mut Client {
    let mut m = state.mons;
    while !m.is_null() {
        let mut c = unsafe { (*m).clients };
        while !c.is_null() {
            if unsafe { (*c).win } == w {
                return c;
            }
            c = unsafe { (*c).next };
        }
        m = unsafe { (*m).next };
    }
    null_mut()
}

#[allow(dead_code)]
unsafe fn getrootptr(state: &mut GmuxState, x: &mut i32, y: &mut i32) -> bool {
    let mut di = 0;
    let mut dui = 0;
    let mut dummy = 0;
    unsafe {
        xlib::XQueryPointer(
            state.dpy,
            state.root,
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

#[allow(dead_code)]
unsafe fn recttomon(state: &mut GmuxState, x: i32, y: i32, w: i32, h: i32) -> *mut Monitor {
    let mut r = state.selmon;
    let mut area = 0;
    let mut m = state.mons;
    while !m.is_null() {
        let mon = unsafe { &*m };
        let a = intersect(x, y, w, h, mon);
        if a > area {
            area = a;
            r = m;
        }
        m = unsafe { (*m).next };
    }
    r
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


#[allow(dead_code)]
fn grabbuttons(_state: &mut GmuxState, _c: *mut Client, _focused: bool) {
    // For now, this is a stub
}


// Helper functions for layouts
#[allow(dead_code)]
unsafe fn next_tiled(mut c: *mut Client) -> *mut Client {
    while !c.is_null() {
        let client = unsafe { &*c };
        if !client.isfloating && is_visible(client) {
            break;
        }
        c = client.next;
    }
    c
}

#[allow(dead_code)]
unsafe fn resize(
    state: &mut GmuxState,
    c: *mut Client,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    _interact: bool,
) {
    let client = unsafe { &mut *c };
    // For now, we'll just resize without applying size hints
    // applysizehints logic will be added later.
    unsafe { resize_client(state, client, x, y, w, h) };
}

#[allow(dead_code)]
unsafe fn resize_client(state: &mut GmuxState, c: *mut Client, x: i32, y: i32, w: i32, h: i32) {
    let client = unsafe { &mut *c };
    client.oldx = client.x;
    client.x = x;
    client.oldy = client.y;
    client.y = y;
    client.oldw = client.w;
    client.w = w;
    client.oldh = client.h;
    client.h = h;
    let win = client.win;
    let mut wc: xlib::XWindowChanges = std::mem::zeroed();
    wc.x = client.x;
    wc.y = client.y;
    wc.width = client.w;
    wc.height = client.h;
    wc.border_width = BORDERPX;
    xlib::XConfigureWindow(state.dpy, win, (xlib::CWX | xlib::CWY | xlib::CWWidth | xlib::CWHeight | xlib::CWBorderWidth) as u32, &mut wc);
    xlib::XMoveResizeWindow(state.dpy, win, client.x, client.y, client.w as u32, client.h as u32);
}

#[allow(dead_code)]
fn is_visible(c: &Client) -> bool {
    unsafe { is_visible_on_mon(c, &*c.mon) }
}

fn is_visible_on_mon(c: &Client, m: &Monitor) -> bool {
    (c.tags & m.tagset[m.seltags as usize]) != 0
}

#[allow(dead_code)]
fn scan(state: &mut GmuxState) {
    unsafe {
        let mut _i: c_uint;
        let mut num: c_uint = 0;
        let mut d1: xlib::Window = 0;
        let mut d2: xlib::Window = 0;
        let mut wins: *mut xlib::Window = null_mut();
        let mut wa: xlib::XWindowAttributes = std::mem::zeroed();

        if xlib::XQueryTree(state.dpy, state.root, &mut d1, &mut d2, &mut wins, &mut num) != 0 {
            for i in 0..num {
                if xlib::XGetWindowAttributes(state.dpy, *wins.offset(i as isize), &mut wa) == 0
                    || wa.override_redirect != 0
                    || xlib::XGetTransientForHint(state.dpy, *wins.offset(i as isize), &mut d1) != 0
                {
                    continue;
                }
            }
            for i in 0..num {
                if xlib::XGetWindowAttributes(state.dpy, *wins.offset(i as isize), &mut wa) == 0 {
                    continue;
                }
                if xlib::XGetTransientForHint(state.dpy, *wins.offset(i as isize), &mut d1) != 0 {
                }
            }

            if !wins.is_null() {
                xlib::XFree(wins as *mut c_void);
            }
        }
    }
}

#[allow(dead_code)]
fn run(state: &mut GmuxState) {
    unsafe {
        let mut ev: xlib::XEvent = std::mem::zeroed();
        xlib::XSync(state.dpy, 0);
        while state.running != 0 && xlib::XNextEvent(state.dpy, &mut ev) == 0 {
            let event_type = ev.get_type();
            if (event_type as usize) < state.handler.len() {
                if let Some(h) = state.handler[event_type as usize] {
                    h(state, &mut ev);
                }
            }
        }
    }
}

#[allow(dead_code)]
fn cleanup(state: &mut GmuxState) {
    unsafe {
        let _a = Arg { ui: !0 };
        let mut m = state.mons;
        while !m.is_null() {
            while !(*m).stack.is_null() {
            }
            m = (*m).next;
        }

        xlib::XUngrabKey(state.dpy, xlib::AnyKey, xlib::AnyModifier, state.root);
        while !state.mons.is_null() {
        }

        for _i in 0..Cur::Last as usize {
        }
        state.drw.free();
        xlib::XDestroyWindow(state.dpy, state.wmcheckwin);
        xlib::XSync(state.dpy, 0);
        xlib::XSetInputFocus(state.dpy, xlib::PointerRoot as u64, xlib::RevertToPointerRoot, xlib::CurrentTime);
        xlib::XDeleteProperty(state.dpy, state.root, state.netatom[Net::ActiveWindow as usize]);
    }
}

#[allow(dead_code)]
unsafe extern "C" fn keypress_wrapper(state: &mut GmuxState, e: *mut xlib::XEvent) {
    let keys = grabkeys(state);
    unsafe { keypress(state, &mut *e, keys.as_ptr(), keys.len()) };
}

#[allow(dead_code)]
unsafe extern "C" fn keypress(state: &mut GmuxState, e: &mut xlib::XEvent, keys: *const Key, keys_len: usize) {
    let ev = unsafe { &*(e as *mut xlib::XEvent as *mut xlib::XKeyEvent) };
    let keys_slice = unsafe { std::slice::from_raw_parts(keys, keys_len) };
    let keysym = unsafe { xlib::XKeycodeToKeysym(state.dpy, ev.keycode as u8, 0) as u32 };
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
#[allow(dead_code)]
fn client_width(c: &Client) -> i32 {
    c.w + 2 * c.bw
}

// Registers all keyboard shortcuts on the root window, similar to dwm's grabkeys
unsafe fn register_grabkeys(state: &mut GmuxState) {
    // Build the key list (same one used elsewhere)
    let keys = grabkeys(state);

    // Prepare modifier combinations (with and without NumLock/CapsLock)
    let modifiers: [u32; 4] = [0, xlib::LockMask, state.numlockmask, state.numlockmask | xlib::LockMask];

    // Clear previous grabs
    xlib::XUngrabKey(state.dpy, xlib::AnyKey, xlib::AnyModifier, state.root);

    // Register all keys
    for key in keys {
        let code = xlib::XKeysymToKeycode(state.dpy, key.keysym as u64);
        if code == 0 {
            continue;
        }
        for m in modifiers.iter() {
            xlib::XGrabKey(
                state.dpy,
                code as c_int,
                key.mask | *m,
                state.root,
                1,
                xlib::GrabModeAsync,
                xlib::GrabModeAsync,
            );
        }
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
        scheme: null_mut(),
        dpy: null_mut(),
        drw: Drw {
            w: 0,
            h: 0,
            dpy: null_mut(),
            screen: 0,
            root: 0,
            drawable: 0,
            gc: null_mut(),
            scheme: null_mut(),
            fonts: Vec::new(),
        },
        mons: null_mut(),
        selmon: null_mut(),
        root: 0,
        wmcheckwin: 0,
        xerror: false,
    };
    
    state.handler[xlib::KeyPress as usize] = Some(keypress_wrapper);
    
    unsafe {
        let locale = CString::new("").unwrap();
        if libc::setlocale(libc::LC_CTYPE, locale.as_ptr()).is_null()
            || xlib::XSupportsLocale() == 0
        {
            eprintln!("warning: no locale support");
        }

        state.dpy = xlib::XOpenDisplay(null_mut());
        if state.dpy.is_null() {
            panic!("dwm: cannot open display");
        }
        // Install permissive X error handler so expected errors (BadWindow, etc.)
        // don’t terminate the WM when closing windows.
        xlib::XSetErrorHandler(Some(xerror_ignore));
        
        // checkotherwm(&mut state);
        setup(&mut state);
        scan(&mut state);
        run(&mut state);
        cleanup(&mut state);
        
        xlib::XCloseDisplay(state.dpy);
    }
}
