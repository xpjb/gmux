# DWM Rust Port: Plan

This document outlines the progress of porting dwm to Rust and the plan for completing the project.

## Progress

Based on an initial analysis, the following components of `dwm` have been ported to Rust:

*   **Core Data Structures:** Structs for `Monitor`, `Client`, `Layout`, `Key`, and `Arg` are defined.
*   **X11 Connection & Setup:** The application can connect to the X server, set up the root window, and handle basic window properties.
*   **Event Handling:** A basic event handling loop is in place, with handlers for `ButtonPress`, `MotionNotify`, and `KeyPress`.
*   **Key Grabbing:** Key bindings are defined and grabbed from the X server.
*   **Basic Layouts:** `tile` and `monocle` layouts have been implemented.
*   **Window Management:** Core functions for managing windows, such as `focus`, `unfocus`, `attach`, `detach`, and `restack` have been ported.
*   **Spawning Processes:** The `spawn` function for launching external commands is implemented.

## Missing Features / Plan

The following key features are still missing from the Rust port. The plan is to implement them in the order listed below.

### 1. Event Handling

The current event handling is incomplete. The following event handlers need to be implemented:

*   [ ] `maprequest`
*   [ ] `destroynotify`
*   [ ] `configurerequest`
*   [ ] `clientmessage`
*   [ ] `propertynotify`
*   [ ] `unmapnotify`
*   [ ] `expose`
*   [ ] `focusin`
*   [ ] `mappingnotify`
*   [ ] `enternotify`

### 2. Drawing and Rendering

The current implementation is missing all drawing and rendering functionality. This is a critical component for displaying the bar, window titles, and other visual elements.

*   [ ] Implement a drawing context (similar to `drw` in the C code).
*   [ ] Port the `drawbar` function.
*   [ ] Port the `drawbars` function.
*   [ ] Implement text rendering with `Xft`.
*   [ ] Implement color schemes.

### 3. Bar

The bar is a key component of `dwm` and is currently missing.

*   [ ] Create the bar window.
*   [ ] Implement the `updatebarpos` and `updatebars` functions.
*   [ ] Display the bar with the correct information (tags, layout symbol, window title, status text).

### 4. Window Management

While some window management functions have been ported, many are still missing or incomplete.

*   [ ] Implement `applyrules` to handle window rules from `config.h`.
*   [ ] Implement `applysizehints` to respect window size hints.
*   [ ] Implement `sendmon` and `tagmon` for multi-monitor support.
*   [ ] Implement `toggleview` and `toggletag`.
*   [ ] Implement `movemouse` and `resizemouse`.

### 5. Configuration

The configuration is currently hardcoded. We need to implement a system for reading the configuration from a file, similar to `config.h`.

*   [ ] Create a `config.rs` file to store configuration options.
*   [ ] Implement a parser for the configuration file.
*   [ ] Use the parsed configuration in the application.

### 6. Miscellaneous

*   [ ] Implement `sigchld` handler for cleaning up zombie processes.
*   [ ] Implement `updategeom` for handling screen size changes.
*   [ ] Implement Xinerama support for multi-monitor setups.
*   [ ] Implement EWMH support for better integration with other applications.
*   [ ] Add more layouts (e.g., floating, spiral, dwindle).
*   [ ] Add support for gaps between windows.
*   [ ] Implement a system for handling status text from external scripts.
*   [ ] Write tests for the application.
*   [ ] Document the code.
