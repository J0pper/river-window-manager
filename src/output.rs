use crate::river::{river_output_v1::RiverOutputV1};

// A logical output. Represents the displayable area of the screen.
// Perhaps a physical monitor or a virtual display.
#[derive(Debug)]
pub struct Output {
    pub proxy: RiverOutputV1, // proxy to communicate with object. See WindowManager, remove_outputs().
    pub removed: bool,        // bool to decide whether output is active or removed.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Output {
    // create new output. Pass in the proxy
    pub fn new(proxy: RiverOutputV1) -> Self {
        // defaults to not removed
        Self {
            proxy,
            removed: false,
            x: 0,
            y: 0,
            width: 0,
            height: 0
        }
    }
}
