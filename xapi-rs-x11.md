# X11 API to `x11` crate Mapping for DWM translation

This document maps the X11 API used in the `dwm` project to their corresponding FFI bindings in the `x11` Rust crate. This is intended to evaluate its suitability for a direct port of `dwm` from C to Rust.

## `transient.c`

### Headers

| X11 Header | `x11` crate equivalent | Notes |
|---|---|---|
| `<X11/Xlib.h>` | `use x11::xlib;` | Provides direct FFI for `libX11`. |
| `<X11/Xutil.h>`| `use x11::xlib;` | Most `Xutil` functions are also in `xlib`.|

### Types

All types are direct FFI type definitions from the C headers. They are typically pointers or structs that must be handled within an `unsafe` context.

| X11 Type | `x11` crate equivalent |
|---|---|
| `Display` | `*mut x11::xlib::Display` |
| `Window` | `x11::xlib::Window` (type alias for `u64`) |
| `XSizeHints` | `x11::xlib::XSizeHints` |
| `XEvent` | `x11::xlib::XEvent` |

### Functions

All function calls must be wrapped in an `unsafe` block.

| X11 Function | `x11` crate equivalent |
|---|---|
| `XOpenDisplay` | `xlib::XOpenDisplay(display_name)` |
| `DefaultRootWindow`| `xlib::XDefaultRootWindow(display)` |
| `XCreateSimpleWindow`| `xlib::XCreateSimpleWindow(...)` |
| `XSetWMNormalHints`| `xlib::XSetWMNormalHints(...)` |
| `XStoreName` | `xlib::XStoreName(...)` |
| `XMapWindow` | `xlib::XMapWindow(...)` |
| `XSelectInput` | `xlib::XSelectInput(...)` |
| `XNextEvent` | `xlib::XNextEvent(...)` |
| `XSetTransientForHint`| `xlib::XSetTransientForHint(...)` |
| `XCloseDisplay` | `xlib::XCloseDisplay(display)` |

## `drw.c`

### Headers

| X11 Header | `x11` crate equivalent | Notes |
|---|---|---|
| `<X11/Xlib.h>` | `use x11::xlib;` | |
| `<X11/Xft/Xft.h>` | `use x11::xft;` | Provides direct FFI for `libXft`. This is the key differentiator. |

### Types

| X11 Type | `x11` crate equivalent |
|---|---|
| `Display` | `*mut x11::xlib::Display` |
| `Window` | `x11::xlib::Window` |
| `Drawable` | `x11::xlib::Drawable` |
| `GC` | `x11::xlib::GC` |
| `Fnt` | `dwm` specific, would wrap `*mut x11::xft::XftFont` |
| `Clr` | `dwm` specific, would wrap `x11::xft::XftColor` |
| `Cur` | `dwm` specific, would wrap `x11::xlib::Cursor` |
| `XftFont` | `*mut x11::xft::XftFont` |
| `FcPattern` | `*mut x11::xft::FcPattern` |
| `XftColor` | `x11::xft::XftColor` |
| `XftDraw` | `*mut x11::xft::XftDraw` |
| `XGlyphInfo`| `x11::xft::XGlyphInfo` |
| `Cursor` | `x11::xlib::Cursor` |

### Functions

| X11 Function | `x11` crate equivalent |
|---|---|
| `XCreatePixmap` | `xlib::XCreatePixmap(...)` |
| `XCreateGC` | `xlib::XCreateGC(...)` |
| `XSetLineAttributes` | `xlib::XSetLineAttributes(...)` |
| `XFreePixmap` | `xlib::XFreePixmap(...)` |
| `XftFontOpenName` | `xft::XftFontOpenName(...)` |
| `FcNameParse` | (Not in `x11` crate) Would need `fontconfig` FFI crate. |
| `XftFontClose` | `xft::XftFontClose(...)` |
| `XftFontOpenPattern`| `xft::XftFontOpenPattern(...)` |
| `FcPatternGetBool` | (Not in `x11` crate) |
| `FcPatternDestroy` | (Not in `x11` crate) |
| `XftColorAllocName`| `xft::XftColorAllocName(...)` |
| `DefaultVisual` | `xlib::XDefaultVisual(display, screen_number)` |
| `DefaultColormap`| `xlib::XDefaultColormap(display, screen_number)` |
| `XSetForeground` | `xlib::XSetForeground(...)` |
| `XFillRectangle` | `xlib::XFillRectangle(...)` |
| `XDrawRectangle` | `xlib::XDrawRectangle(...)` |
| `XftDrawCreate` | `xft::XftDrawCreate(...)` |
| `XftCharExists` | `xft::XftCharExists(...)` |
| `XftDrawStringUtf8`| `xft::XftDrawStringUtf8(...)` |
| `FcCharSetCreate` | (Not in `x11` crate) |
| `FcCharSetAddChar` | (Not in `x11` crate) |
| `FcPatternDuplicate`| (Not in `x11` crate) |
| `FcPatternAddCharSet`| (Not in `x11` crate) |
| `FcPatternAddBool` | (Not in `x11` crate) |
| `FcConfigSubstitute`| (Not in `x11` crate) |
| `FcDefaultSubstitute`| (Not in `x11` crate) |
| `XftFontMatch` | `xft::XftFontMatch(...)` |
| `XftDrawDestroy` | `xft::XftDrawDestroy(...)` |
| `XCopyArea` | `xlib::XCopyArea(...)` |
| `XSync` | `xlib::XSync(...)` |
| `XftTextExtentsUtf8`| `xft::XftTextExtentsUtf8(...)` |
| `XCreateFontCursor` | `xlib::XCreateFontCursor(...)` |
| `XFreeCursor` | `xlib::XFreeCursor(...)` |

## `dwm.c`

### Headers

| X11 Header | `x11` crate equivalent |
|---|---|
| `<X11/cursorfont.h>` | Constants are in `x11::xcursor`. |
| `<X11/keysym.h>` | Constants are in `x11::keysym`. |
| `<X11/Xatom.h>` | Atoms are in `x11::xlib`. |
| `<X11/Xlib.h>` | `use x11::xlib;` |
| `<X11/Xproto.h>` | Protocol constants are in `x11::xlib`. |
| `<X11/Xutil.h>` | `use x11::xlib;` |
| `<X11/extensions/Xinerama.h>` | `use x11::xinerama;` |
| `<X11/Xft/Xft.h>` | `use x11::xft;` |

### Types

| X11 Type | `x11` crate equivalent |
|---|---|
| `Display` | `*mut xlib::Display` |
| `Window` | `xlib::Window` |
| `XEvent` | `xlib::XEvent` |
| `KeySym` | `xlib::KeySym` |
| `Atom` | `xlib::Atom` |
| `XWindowAttributes` | `xlib::XWindowAttributes` |
| `XClassHint` | `xlib::XClassHint` |
| `XConfigureEvent` | `xlib::XConfigureEvent` |
| ... (All event types) | `xlib::X...Event` |
| `XWindowChanges` | `xlib::XWindowChanges` |
| `XTextProperty` | `xlib::XTextProperty` |
| `KeyCode` | `xlib::KeyCode` |
| `XineramaScreenInfo`| `xinerama::XineramaScreenInfo` |
| `XModifierKeymap` | `*mut xlib::XModifierKeymap` |
| `XSizeHints` | `xlib::XSizeHints` |
| `XWMHints` | `xlib::XWMHints` |

### Functions

| X11 Function | `x11` crate equivalent |
|---|---|
| `XGetClassHint` | `xlib::XGetClassHint(...)` |
| `XFree` | `xlib::XFree(...)` |
| `XAllowEvents` | `xlib::XAllowEvents(...)` |
| `XSetErrorHandler` | `xlib::XSetErrorHandler(...)` |
| ... (All `X...` functions) | `xlib::X...(...)` or `xinerama::X...(...)` |
| `XineramaIsActive` | `xinerama::XineramaIsActive(...)` |
| `XineramaQueryScreens` | `xinerama::XineramaQueryScreens(...)` |
| `Fc...` functions | Require a separate `fontconfig-sys` crate. |

## Conclusion

The `x11` crate provides a near-complete, 1-to-1 mapping for the `Xlib`, `Xft`, and `Xinerama` functions used by `dwm`. The primary gap is the lack of `fontconfig` bindings, which would need to be supplied by another FFI crate.

This library would allow for a very direct port of the C code, but it comes at the cost of idiomatic Rust. The resulting code would be heavily reliant on `unsafe` blocks and manual memory management.
