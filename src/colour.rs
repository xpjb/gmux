#![allow(warnings)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Colour {
    BarBackground,
    BarForeground,
    TextNormal,
    TextQuiet,
    WindowActive,
    WindowInactive,
    Urgent,
    DebugRed,
}

impl Colour {
    pub fn get_colour(&self) -> [u8; 4] {
        match self {
            Colour::BarBackground => [20, 20, 20, 255],
            Colour::BarForeground => [0, 64, 200, 255],
            Colour::TextNormal => [255, 255, 255, 255],
            Colour::TextQuiet => [122, 122, 122, 255],
            Colour::WindowActive => [255, 255, 0, 255],
            Colour::WindowInactive => [50, 50, 50, 255],
            Colour::Urgent => [255, 0, 0, 255], // Using red for urgent
            Colour::DebugRed => [255, 0, 0, 255],
        }
    }
}

pub const ALL_COLOURS: [Colour; 8] = [
    Colour::BarBackground,
    Colour::BarForeground,
    Colour::TextNormal,
    Colour::TextQuiet,
    Colour::WindowActive,
    Colour::WindowInactive,
    Colour::Urgent,
    Colour::DebugRed,
];
