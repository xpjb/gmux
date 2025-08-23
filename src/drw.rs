use std::os::raw::{c_int, c_uint};
use std::ptr::null_mut;
use x11::{xlib, xft};

use crate::{ecalloc, die};
use std::ffi::CString;
use fontconfig::{self};
// direct Fontconfig sys calls removed

pub struct Drw {
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

pub struct Fnt {
    pub dpy: *mut xlib::Display,
    pub h: c_uint,
    pub xfont: *mut xft::XftFont,
    // Currently we don't keep a Fontconfig pattern pointer; fallback handling TBD.
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

pub type Clr = xft::XftColor;


#[derive(PartialEq, Copy, Clone)]
pub enum Scheme {
    Norm,
    Sel,
    Urg,
}

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
            let drawable = xlib::XCreatePixmap(dpy, root, w, h, xlib::XDefaultDepth(dpy, screen) as u32);
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
            // Ensure fontconfig has been initialised. This is a cheap call if already done.
            let _fc_handle = fontconfig::Fontconfig::new();

            // Convert Rust &str to C string for the various C APIs.
            let cstr = match CString::new(font_name) {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("error, invalid font name '{}': contains NUL", font_name);
                    return false;
                }
            };

            // Open an Xft font from the name.
            let xfont = xft::XftFontOpenName(self.dpy, self.screen, cstr.as_ptr());
            if xfont.is_null() {
                eprintln!("error, cannot load font from name: '{}'", font_name);
                return false;
            }

            // Height is ascent + descent like dwm.
            let h = ((*xfont).ascent + (*xfont).descent) as c_uint;
            // Construct Fnt instance and store in vector.
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
        let clrs = ecalloc(clr_names.len(), std::mem::size_of::<Clr>()) as *mut Clr;
        for (i, clr_name) in clr_names.iter().enumerate() {
            let cstr = CString::new(*clr_name).expect("color name contains NUL");
            unsafe {
                if xft::XftColorAllocName(
                    self.dpy,
                    xlib::XDefaultVisual(self.dpy, self.screen),
                    xlib::XDefaultColormap(self.dpy, self.screen),
                    cstr.as_ptr(),
                    &mut *clrs.add(i),
                ) == 0
                {
                    die("cannot allocate color");
                }
            }
        }
        clrs
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

    pub fn text(&self, x: i32, y: i32, w: u32, h: u32, lpad: u32, text: &str, _invert: bool) -> i32 {
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
                y + (h as i32 - ((*usedfont.xfont).ascent + (*usedfont.xfont).descent) as i32) / 2 + (*usedfont.xfont).ascent as i32,
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
