// User can only do one action at a time.
#[derive(Debug, Clone, Copy)]
pub enum Action {
    None,
    SpawnFoot,
    Close,
    FocusNext,
    Move,
    Resize,
    Exit,
}
