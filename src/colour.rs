#![allow(warnings)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Colour {
    BarBackground,
    BarForeground,
    TextNormal,
    TextQuiet,
    WindowActive,
    WindowInactive,
    DebugRed,
}

impl Colour {
    pub fn get_colour(&self) -> [u8; 4] {
        match self {
            Colour::BarBackground => [50, 50, 50, 255],
            Colour::BarForeground => [0, 128, 255, 255],
            Colour::TextNormal => [255, 255, 255, 255],
            Colour::TextQuiet => [102, 102, 102, 255],
            Colour::WindowActive => [255, 255, 0, 255],
            Colour::WindowInactive => [50, 50, 50, 255],
            Colour::DebugRed => [255, 0, 0, 255],
        }
    }
}

pub const ALL_COLOURS: [Colour; 7] = [
    Colour::BarBackground,
    Colour::BarForeground,
    Colour::TextNormal,
    Colour::TextQuiet,
    Colour::WindowActive,
    Colour::WindowInactive,
    Colour::DebugRed,
];
