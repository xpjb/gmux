# Compatibility Notes: x11rb vs. x11 crate for DWM Port

This document outlines the compatibility findings and discrepancies discovered while evaluating Rust X11 libraries for the purpose of porting `dwm`.

## Core Issue: Font Rendering (Xft)

The most significant gap identified is the handling of font rendering. `dwm` uses `Xft` for this purpose, but the two primary Rust X11 libraries have very different approaches:

*   **`x11rb`**: This is a modern, pure-Rust implementation of the X11 protocol. It does **not** provide bindings for external libraries like `Xft`. To handle font rendering with `x11rb`, we would need to:
    1.  Integrate a separate Rust font-loading and rasterization crate (e.g., `rusttype`, `pango`).
    2.  Render the text glyphs into an in-memory buffer.
    3.  Create an X11 Pixmap and copy the rendered glyphs into it for display.
    This approach is more idiomatic to Rust but requires re-implementing the entire drawing layer from scratch, which is a substantial effort.

*   **`x11` crate**: This is a more traditional FFI (Foreign Function Interface) wrapper around the system's C libraries, including `libX11`, `libXft`, and `libXinerama`. It appears to provide direct, 1-to-1 bindings for the C functions `dwm` already uses. This would allow for a much more direct translation of the existing C code in `drw.c`.

## Library Comparison

Here is a summary of the trade-offs between the two libraries:

### `x11rb`

*   **Pros**:
    *   **Idiomatic Rust**: Uses `Result` for error handling, RAII for resource management (e.g., connections, windows), and avoids `unsafe` code for core protocol operations.
    *   **Type Safety**: The protocol is defined in XML and generated at compile time, providing strong type safety.
    *   **Modern Features**: Includes `async` support.
    *   **Pure Rust**: The core connection is implemented in Rust, removing the dependency on `libxcb` for the basic protocol.

*   **Cons**:
    *   **Missing Abstractions**: Lacks high-level library bindings for `Xft`, `Xinerama` helpers, etc. While it has protocol support for extensions, it doesn't wrap the C libraries.
    *   **Higher Porting Effort**: Requires significant architectural changes to `dwm`'s drawing and font-handling code.

### `x11` crate

*   **Pros**:
    *   **Complete Coverage**: Provides FFI bindings for the full suite of libraries used by `dwm`, including `Xlib`, `Xft`, and `Xinerama`.
    *   **Direct Translation**: Allows for a near-direct port of the existing C code, which dramatically reduces the initial effort.

*   **Cons**:
    *   **`unsafe` Code**: Virtually every call will be wrapped in an `unsafe` block.
    *   **C Idioms**: Exposes raw pointers, requires manual memory management (e.g., `XFree`), and uses C-style error handling (e.g., returning `0` or `null`).
    *   **System Dependencies**: Requires the corresponding C libraries (`libX11`, `libXft`, etc.) to be installed on the user's system.

## Recommendation and Next Steps

The analysis is now complete. Given the goal of a functional port of `dwm` without a major architectural rewrite, the `x11` crate is the recommended path.

**Decision**: Proceed with the `x11` crate.

**Rationale**:
*   It provides direct, 1-to-1 bindings for the most critical and complex parts of the `dwm` codebase, especially `drw.c` which relies on `Xft`.
*   This approach minimizes the initial porting effort and allows for an incremental translation of the existing C logic into Rust.
*   The architectural simplicity of `dwm` makes the downsides of an FFI-heavy approach (heavy use of `unsafe`, manual memory management) more manageable than in a larger, more complex project.
*   The primary API gap, `fontconfig`, can be filled with an additional FFI crate (e.g., `fontconfig-sys`).

**Next Steps**:
1.  Set up a new Rust project structure.
2.  Add `x11` as a dependency.
3.  Find and add a suitable `fontconfig` FFI crate to cover the remaining function calls.
4.  Begin the direct port, starting with `main` in `dwm.c`, wrapping C calls in `unsafe` blocks.
