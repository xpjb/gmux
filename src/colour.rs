#![allow(warnings)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Colour {
    BarBackground,
    BarForeground,
    TextNormal,
    TextQuiet,
    WindowActive,
    WindowInactive,
}

impl Colour {
    pub fn get_colour(&self) -> [u8; 4] {
        match self {
            Colour::BarBackground => [50, 50, 50, 255],
            Colour::BarForeground => [128, 128, 255, 255],
            Colour::TextNormal => [255, 255, 255, 255],
            Colour::TextQuiet => [102, 102, 102, 255],
            Colour::WindowActive => [255, 255, 0, 255],
            Colour::WindowInactive => [50, 50, 50, 255],
        }
    }
}

pub const ALL_COLOURS: [Colour; 6] = [
    Colour::BarBackground,
    Colour::BarForeground,
    Colour::TextNormal,
    Colour::TextQuiet,
    Colour::WindowActive,
    Colour::WindowInactive,
];
