//! Cell grid — exterior cell coordinates and 9-cell context.

/// Compute neighbor cells for a cell in the exterior grid.
///
/// Gamebryo exterior cells form a 2D grid. Cells are encoded as:
/// `(world_id << 16) | ((y as u16) << 8) | (x as u8)`
///
/// For interior cells, cell ID is the interior form ID — neighbors not relevant.
pub struct CellGrid;

impl CellGrid {
    /// Extract cell coordinates from a cell ID.
    /// Returns (world_id, x, y) for exterior cells, or (0, 0, 0) for interiors.
    pub fn decode_cell(cell: u32) -> (u32, i32, i32) {
        let world_id = cell >> 16;
        let y = ((cell >> 8) & 0xFF) as i32;
        let x = (cell & 0xFF) as i32;
        (world_id, x, y)
    }

    /// Encode a cell ID from world and coordinates.
    pub fn encode_cell(world_id: u32, x: i32, y: i32) -> u32 {
        (world_id << 16) | (((y as u16) as u32) << 8) | ((x as u8) as u32)
    }

    /// Get the 8 neighboring cells + center (9-cell grid).
    /// Interior cells return empty neighbors (only center matters).
    pub fn neighbors(cell: u32) -> [u32; 9] {
        let (world_id, x, y) = Self::decode_cell(cell);

        // Interior cells have no grid neighbors
        if world_id == 0 {
            let mut cells = [0u32; 9];
            cells[4] = cell;
            return cells;
        }

        // Exterior: 3x3 grid
        let offsets = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (0, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];

        let mut result = [0u32; 9];
        for (i, (ox, oy)) in offsets.iter().enumerate() {
            result[i] = Self::encode_cell(world_id, x + ox, y + oy);
        }
        result
    }

    /// Check if two cells are the same or neighbors (within 9-cell context).
    pub fn is_in_context(context: &[u32; 9], cell: u32) -> bool {
        context.contains(&cell)
    }
}

/// 9-cell context around a player.
#[derive(Debug, Clone)]
pub struct CellContext {
    pub cells: [u32; 9],
}

impl CellContext {
    pub fn new(center: u32) -> Self {
        CellContext {
            cells: CellGrid::neighbors(center),
        }
    }

    pub fn update(&mut self, center: u32) -> bool {
        if self.cells[4] == center {
            return false; // no change
        }
        self.cells = CellGrid::neighbors(center);
        true
    }

    pub fn center(&self) -> u32 {
        self.cells[4]
    }

    pub fn contains(&self, cell: u32) -> bool {
        CellGrid::is_in_context(&self.cells, cell)
    }

    /// Compute cells to enter and leave when context changes.
    pub fn diff(&self, other: &CellContext) -> (Vec<u32>, Vec<u32>) {
        let enter: Vec<u32> = other
            .cells
            .iter()
            .filter(|c| !self.cells.contains(c))
            .copied()
            .collect();
        let leave: Vec<u32> = self
            .cells
            .iter()
            .filter(|c| !other.cells.contains(c))
            .copied()
            .collect();
        (enter, leave)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_roundtrip() {
        // byte-coordinate format: x/y fit in u8 (world cells are small grids)
        let cell = CellGrid::encode_cell(0x11, 5, 3);
        assert_eq!(CellGrid::decode_cell(cell), (0x11, 5, 3));
        let zero = CellGrid::encode_cell(0, 0, 0);
        assert_eq!(CellGrid::decode_cell(zero), (0, 0, 0));
    }

    #[test]
    fn interior_cell_has_no_neighbors() {
        // world_id 0 = interior
        let interior = CellGrid::encode_cell(0, 3, 4);
        let (world, _, _) = CellGrid::decode_cell(interior);
        assert_eq!(world, 0);
        let n = CellGrid::neighbors(interior);
        assert_eq!(n[4], interior); // center is itself
        assert!(n.iter().all(|&c| c == 0 || c == interior)); // rest empty
    }

    #[test]
    fn exterior_neighbors_3x3() {
        let center = CellGrid::encode_cell(0x7, 10, 10);
        let n = CellGrid::neighbors(center);
        // all 9 distinct, center at index 4, all share the world id
        let mut uniq: Vec<u32> = n.to_vec();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), 9);
        assert_eq!(n[4], center);
        for &c in &n {
            assert_eq!(CellGrid::decode_cell(c).0, 0x7);
        }
    }

    #[test]
    fn neighbors_are_adjacent() {
        let center = CellGrid::encode_cell(0x7, 10, 10);
        let n = CellGrid::neighbors(center);
        // NW neighbor is (9, 9)
        let nw = CellGrid::encode_cell(0x7, 9, 9);
        assert_eq!(n[0], nw);
        // E neighbor is (11, 10)
        let e = CellGrid::encode_cell(0x7, 11, 10);
        assert_eq!(n[5], e);
    }

    #[test]
    fn context_membership() {
        let center = CellGrid::encode_cell(0x7, 10, 10);
        let ctx = CellGrid::neighbors(center);
        assert!(CellGrid::is_in_context(&ctx, center));
        assert!(CellGrid::is_in_context(&ctx, CellGrid::encode_cell(0x7, 11, 11)));
        assert!(!CellGrid::is_in_context(&ctx, CellGrid::encode_cell(0x7, 20, 20)));
        assert!(!CellGrid::is_in_context(&ctx, CellGrid::encode_cell(0x8, 10, 10))); // other world
    }
}
