

pub struct Generation(u32);

impl Generation {
    
    pub const fn new() -> Self {
        Self(0)
    }

    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    pub fn value(self) -> u32 {
        self.0
    }
}