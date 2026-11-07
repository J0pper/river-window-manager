use crate::window::WindowId;

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub struct CanvasId(u32);
static mut CANVAS_ID_COUNTER: Option<u32> = None;

#[derive(Debug, Clone)]
pub struct Canvas {
    pub focused_window: Option<WindowId>, // alt: active_window
    pub sub_canvases: Vec<CanvasId>,
    pub shown_windows: Vec<CanvasId>,
    pub hidden_windows: Vec<CanvasId>,

    pub def_width: i32,
    pub def_height: i32,
    pub def_x: i32,
    pub def_y: i32,

    pub curr_width: i32,
    pub curr_height: i32,
    pub curr_x: i32,
    pub curr_y: i32,

    pub parent: Option<CanvasId>,
    pub id: CanvasId,
    pub scale_x: f32,
    pub scale_y: f32,
}


impl Canvas {
    pub fn create(parent: Option<CanvasId>, window_id: Option<WindowId>) -> Self {
        Canvas {
            focused_window: window_id,
            sub_canvases: Vec::new(),
            shown_windows: Vec::new(),
            hidden_windows: Vec::new(),
            def_width: 0,
            def_height: 0,
            def_x: 0,
            def_y: 0,
            curr_width: 0,
            curr_height: 0,
            curr_x: 0,
            curr_y: 0,
            parent: parent,
            id: CanvasId::new(),
            scale_x: 1.0,
            scale_y: 1.0,
        }
    }

    // pub fn destroy(&mut self) {
    //     if let Some(window) = &mut self.window {
    //         window.destroy();
    //     }
    // }


    pub fn set_default_position(&mut self, x: i32, y: i32) {
        (self.def_x, self.def_y) = (x, y);

        self.set_current_position(x, y);
    }

    pub fn set_current_position(&mut self, x: i32, y: i32) {
        (self.curr_x, self.curr_y) = (x, y);

        // if let Some(window) = &mut self.window {
        //     (window.x, window.y) = (Some(x), Some(y));
        // }
    }

    pub fn set_default_dimensions(&mut self, width: i32, height: i32) {
        (self.def_width, self.def_height) = (width, height);

        self.set_current_dimensions(width, height);
    }

    pub fn set_current_dimensions(&mut self, width: i32, height: i32) {
        (self.curr_width, self.curr_height) = (width, height);

        // if let Some(window) = &mut self.window {
        //     (window.width, window.height) = (Some(width), Some(height));
        // }
    }

    pub fn focus_window(&mut self, _canvas_id: CanvasId) {
        self.set_current_position(0, 0);

        // for id in self.shown_windows.clone() {
        //     self.hide(id);
        // }
        //
        // self.show(canvas_id);
    }

}

impl CanvasId {
    pub fn new() -> Self {
        unsafe {
            match CANVAS_ID_COUNTER {
                Option::Some(counter) => CANVAS_ID_COUNTER = Some(counter + 1),
                Option::None          => CANVAS_ID_COUNTER = Some(0),
            }
            Self(CANVAS_ID_COUNTER.expect("Invalid CANVAS_ID_COUNTER value"))
        }
    }
}
