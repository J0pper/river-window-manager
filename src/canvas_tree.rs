use std::collections::HashMap;

use crate::{canvas::{Canvas, CanvasId}, window::Visibility};

#[derive(Debug, Default, Clone)]
pub struct CanvasTree {
    pub canvases: HashMap<CanvasId, Canvas>, // canvas_id -> canvas map
    pub current_canvas: Option<CanvasId>,    // id of current parrent canvas
}

impl CanvasTree {
    pub fn new() -> Self {
        Self {
            canvases: HashMap::new(),
            current_canvas: None,
        }
    }

    pub fn calc_next_canvas_pos(&mut self, canvas_id: CanvasId) -> (i32, i32) {
        let Some(canvas) = self.canvases.get(&canvas_id) else { return (0, 0) };
        let mut curr_x = 0;
        let     curr_y = 0;

        for sub_id in &canvas.sub {
            if let Some(sub_canvas) = self.canvases.get(&sub_id) {
                curr_x += sub_canvas.def_width;
            }

        }
        (curr_x, curr_y)
    }

    pub fn focus_canvas(&mut self, parent_id: CanvasId, target_id: CanvasId) {
        let Some(parent) = self.canvases.get_mut(&parent_id) else { return };
        parent.focus_window(target_id);
    }

    pub fn hide_canvas(&mut self, parent_id: CanvasId, target_id: CanvasId) {
        if let Some(parent) = self.canvases.get_mut(&parent_id) {
            parent.shown_windows.retain(|id| *id != target_id);
            if !parent.hidden_windows.contains(&target_id) {
                parent.hidden_windows.push(target_id);
            }
        }

        if let Some(target_canvas) = self.canvases.get_mut(&target_id) {
            if let Some(window) = &mut target_canvas.window {
                window.visibility = Some(Visibility::Hide);
            }
        }
    }

    pub fn show_canvas(&mut self, parent_id: CanvasId, target_id: CanvasId) {
        if let Some(parent) = self.canvases.get_mut(&parent_id) {
            parent.hidden_windows.retain(|id| *id != target_id);
            if !parent.shown_windows.contains(&target_id) {
                parent.shown_windows.push(target_id);
            }
        }

        if let Some(target_canvas) = self.canvases.get_mut(&target_id) {
            if let Some(window) = &mut target_canvas.window {
                window.visibility = Some(Visibility::Show);
            }
        }
    }
}
