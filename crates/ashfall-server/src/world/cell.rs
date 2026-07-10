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
        let world_id = (cell >> 16) as u32;
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
            (-1, -1), (0, -1), (1, -1),
            (-1,  0), (0,  0), (1,  0),
            (-1,  1), (0,  1), (1,  1),
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
