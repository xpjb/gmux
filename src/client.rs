use crate::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientHandle(x11::xlib::XID);

#[derive(Debug, Clone)]
pub struct Client {
    pub name: String,
    pub min_aspect: f32,
    pub max_aspect: f32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub oldx: i32,
    pub oldy: i32,
    pub oldw: i32,
    pub oldh: i32,
    pub base_width: i32,
    pub base_height: i32,
    pub width_inc: i32,
    pub height_inc: i32,
    pub max_width: i32,
    pub max_height: i32,
    pub min_width: i32,
    pub min_height: i32,
    pub bw: i32,
    pub _oldbw: i32,
    pub tags: u32,
    pub is_fixed: bool,
    pub is_floating: bool,
    pub is_urgent: bool,
    pub _never_focus: bool,
    pub _old_state: bool,
    pub is_fullscreen: bool,
    pub monitor_idx: usize,
    pub win: Window,
}

impl Client {
    pub fn width(&self) -> i32 {
        self.w + 2 * self.bw
    }

    pub fn is_visible_on(&self, m: &Monitor) -> bool {
        (self.tags & m.tagset[m.selected_tags as usize]) != 0
    }
}