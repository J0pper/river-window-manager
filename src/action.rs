use crate::canvas::CanvasId;

// User can only do one action at a time.
#[derive(Clone, Copy, Debug)]
pub enum Action {
    None,
    Exit,
    SpawnFoot,
    Canvas(CanvasAction),
    Output(OutputAction),
    // Overview,
    // Close,
    // FocusNext,
    // Move,
    // Resize,
}

#[derive(Clone, Copy, Debug)]
pub enum CanvasFocus {
    Direction(Direction),
    // window_id: | window or canvas id?
}

#[derive(Clone, Copy, Debug)]
pub enum WindowSwap {
    Direction(Direction),
    // ids: | window or canvas ids?
}

#[derive(Clone, Copy, Debug)]
pub enum CanvasAction {
    Focus(CanvasFocus),
    Swap(WindowSwap),
    Send(OutputTarget),
    Zoom { scale: f32 },
    Drill { step: Depth, id: Option<CanvasId> },
    Rename, // works like argen
    Close(Option<CanvasId>),
    CloseFocused,
    Attach, // would need IPC for this as we need to send information from one process
    Detach, // (the one selecting the windows) to the window manager process.
}

// #[derive(Copy, Clone)]
// pub enum Drill {
//     Direction(Depth),
//     // id: CanvasId,
// }

// #[derive(Copy, Clone)]
// pub enum Zoom {
//     Direction(Depth),
//     Scale(f32),
// }

#[derive(Clone, Copy, Debug)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug)]
pub enum Depth {
    In,
    Out,
}

#[derive(Clone, Copy, Debug)]
pub enum OutputAction {
    Focus(Direction),
}

#[derive(Clone, Copy, Debug)]
pub enum OutputTarget {
    Direction(Direction),
    // id: ,
}
