use crate::canvas::CanvasId;

// User can only do one action at a time.
#[derive(Clone, Copy)]
pub union Action {
    pub window: WindowAction,
    pub canvas: CanvasAction,
    pub output: OutputAction,
    pub exit: bool,

    // None,
    // Overview,
    pub spawn_foot: bool,
    // Close,
    // FocusNext,
    // Move,
    // Resize,
    // Exit,
    //
}

#[derive(Copy, Clone)]
pub union WindowAction {
    focus: WindowFocus,
    swap: WindowSwap,
    send: OutputTarget,
    attach: CanvasId,
    detach: CanvasId,
    close: CanvasId,
}

#[derive(Copy, Clone)]
pub union WindowFocus {
    direction: Direction,
    // window_id: | window or canvas id?
}

#[derive(Copy, Clone)]
pub union WindowSwap {
    direction: Direction,
    // ids: | window or canvas ids?
}

#[derive(Copy, Clone)]
pub union CanvasAction {
    zoom: Zoom,
    drill: Drill,
    close: bool,
    // rename: works like argen
}

#[derive(Copy, Clone)]
pub union Drill {
    direction: Direction,
    id: CanvasId,
}

#[derive(Copy, Clone)]
pub union Zoom {
    direction: Direction,
    scale: f32,
}

#[derive(Copy, Clone)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    In,
    Out,
}

#[derive(Copy, Clone)]
pub union OutputAction {
    focus: OutputTarget,

}

#[derive(Copy, Clone)]
pub union OutputTarget {
    direction: Direction,
    // id: ,
}

impl Action {
    pub fn parse() {

    }
}
