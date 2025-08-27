use crate::*;

#[derive(Clone, Debug)]
pub struct Clickable {
    pub pos: IVec2,
    pub size: IVec2,
    pub action: Action,
}

#[derive(Debug, Clone)]
pub struct Monitor {
    pub lt_symbol: String,
    pub mfact: f32,
    pub nmaster: i32,
    pub _num: i32,
    pub by: i32,
    pub _mx: i32,
    pub _my: i32,
    pub _mw: i32,
    pub _mh: i32,
    pub wx: i32,
    pub wy: i32,
    pub ww: i32,
    pub wh: i32,
    pub selected_tags: u32,
    pub selected_lt: u32,
    pub tagset: [u32; 2],
    pub show_bar: bool,
    pub top_bar: bool,
    pub clients: Vec<Client>,
    pub clickables: Vec<Clickable>,
    pub sel: Option<usize>,
    pub stack: Vec<usize>,
    pub bar_window: Window,
    pub lt: [&'static Layout; 2],
}

impl Monitor {
    pub fn intersect_area(&self, x: i32, y: i32, w: i32, h: i32) -> i32 {
        std::cmp::max(
            0,
            std::cmp::min(x + w, self.wx + self.ww) - std::cmp::max(x, self.wx),
        ) * std::cmp::max(
            0,
            std::cmp::min(y + h, self.wy + self.wh) - std::cmp::max(y, self.wy),
        )
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Monitor {
            lt_symbol: String::new(),
            mfact: 0.0,
            nmaster: 0,
            _num: 0,
            by: 0,
            _mx: 0,
            _my: 0,
            _mw: 0,
            _mh: 0,
            wx: 0,
            wy: 0,
            ww: 0,
            wh: 0,
            selected_tags: 0,
            selected_lt: 0,
            tagset: [0, 0],
            show_bar: false,
            top_bar: false,
            clients: Vec::new(),
            clickables: Vec::new(),
            sel: None,
            stack: Vec::new(),
            bar_window: Window(0),
            lt: [&crate::layouts::LAYOUTS[0], &crate::layouts::LAYOUTS[1]],
        }
    }
}
