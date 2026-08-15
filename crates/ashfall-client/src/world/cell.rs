//! Client cell tracking.
//!
//! ponytail: unused by the stub-mode client; used once cell rendering lands.
#![allow(dead_code)]

pub struct CellTracker {
    pub current_cell: u32,
    pub context: [u32; 9],
}

impl CellTracker {
    pub fn new() -> Self {
        CellTracker {
            current_cell: 0,
            context: [0; 9],
        }
    }

    pub fn update(&mut self, cells: [u32; 9]) {
        self.current_cell = cells[4];
        self.context = cells;
    }
}

impl Default for CellTracker {
    fn default() -> Self {
        Self::new()
    }
}
