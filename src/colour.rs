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
            Colour::BarBackground => [0x14, 0x14, 0x15, 0xFF],
            Colour::BarForeground => [0x25, 0x25, 0x30, 0xFF],
            Colour::TextNormal => [0xCD, 0xCD, 0xCD, 0xFF],
            Colour::TextQuiet => [0x60, 0x60, 0x79, 0xFF],
            Colour::WindowActive => [0xE0, 0xA3, 0x63, 0xFF],
            Colour::WindowInactive => [0x25, 0x25, 0x30, 0xFF],
            Colour::Urgent => [0xD8, 0x64, 0x7E, 0xFF], 
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
