use crate::river::{river_window_manager_v1::RiverWindowManagerV1, river_xkb_bindings_v1::RiverXkbBindingsV1};

use crate::window_manager::WindowManager;

#[derive(Debug, Default)]
pub struct AppData {
    pub river_wm: Option<RiverWindowManagerV1>,
    pub river_xkb: Option<RiverXkbBindingsV1>,
    pub wm: WindowManager,
}

