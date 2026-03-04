#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileGrid {
    pub rows: usize,
    pub cols: usize,
}

pub fn compute_tile_grid(count: usize, viewport_width: f32, viewport_height: f32) -> TileGrid {
    if count == 0 {
        return TileGrid { rows: 0, cols: 0 };
    }

    let safe_width = viewport_width.max(1.0);
    let safe_height = viewport_height.max(1.0);

    let mut best = TileGrid { rows: count, cols: 1 };
    let mut best_score = f32::MAX;

    for cols in 1..=count {
        let rows = count.div_ceil(cols);
        let cell_w = safe_width / cols as f32;
        let cell_h = safe_height / rows as f32;

        let cell_aspect = cell_w / cell_h;
        let target_aspect = 1.65;
        let aspect_penalty = (cell_aspect - target_aspect).abs();
        let empty_cells = rows * cols - count;

        let score = (aspect_penalty * 4.0) + (empty_cells as f32 * 0.25) + (rows as f32 * 0.01);
        if score < best_score {
            best_score = score;
            best = TileGrid { rows, cols };
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_terminal_uses_single_cell() {
        let grid = compute_tile_grid(1, 1920.0, 1080.0);
        assert_eq!(grid.rows, 1);
        assert_eq!(grid.cols, 1);
    }

    #[test]
    fn grid_always_has_enough_cells() {
        for count in 1..=20 {
            let grid = compute_tile_grid(count, 1920.0, 1080.0);
            assert!(grid.rows * grid.cols >= count);
        }
    }

    #[test]
    fn tall_viewport_prefers_more_rows() {
        let grid = compute_tile_grid(6, 900.0, 1600.0);
        assert!(grid.rows >= 2);
    }

    #[test]
    fn wide_viewport_prefers_more_columns() {
        let grid = compute_tile_grid(6, 2200.0, 900.0);
        assert!(grid.cols >= 2);
    }

    #[test]
    fn empty_input_has_zero_grid() {
        let grid = compute_tile_grid(0, 1000.0, 700.0);
        assert_eq!(grid, TileGrid { rows: 0, cols: 0 });
    }
}
