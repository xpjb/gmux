use std::ops::{Add, Sub};

#[derive(Debug, Clone, Copy)]
pub struct IVec2 {
    pub x: i32,
    pub y: i32,
}

pub fn ivec2(x: i32, y: i32) -> IVec2 {
    IVec2 { x, y }
}

impl IVec2 {
    pub fn projx(&self) -> Self {
        ivec2(self.x, 0)
    }

    pub fn projy(&self) -> Self {
        ivec2(0, self.y)
    }
}

impl Add for IVec2 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl Sub for IVec2 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}