use crate::river::{river_window_v1::{RiverWindowV1, Edges}};

// Operations on Seats.
#[derive(Debug, Clone)]
pub enum SeatOp {
    None,
    Move {
        window_proxy: RiverWindowV1,
        start_x: i32,
        start_y: i32,
    },
    Resize {
        window_proxy: RiverWindowV1,
        start_x: i32,
        start_y: i32,
        start_width: i32,
        start_height: i32,
        edges: Edges,
    },
}

