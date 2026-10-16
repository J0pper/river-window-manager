use wayland_client::{ Connection, protocol::{wl_display::WlDisplay, wl_registry}, Dispatch, Proxy, QueueHandle };

use crate::river::{river_node_v1::RiverNodeV1, river_seat_v1::RiverSeatV1, river_window_v1::{DecorationHint, Edges, RiverWindowV1}};

use crate::app_data::AppData;
use crate::window_manager::WindowManager;


#[derive(Debug)]
pub struct Window {
    pub proxy: RiverWindowV1,
    pub node: RiverNodeV1,
    pub new: bool,
    pub closed: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub pointer_move_requested: Option<RiverSeatV1>,
    pub pointer_resize_requested: Option<RiverSeatV1>,
    pub pointer_resize_requested_edges: Edges,
    pub pending_manage: PendingManage,
    pub pending_render: PendingRender,
}

#[derive(Debug, Clone, Copy)]
pub struct PendingManage {
    pub width: Option<i32>,
    pub height: Option<i32>,
    fullscreen: bool,
    exit_fullscreen: bool,
    maximize: bool,
    unmaximize: bool,
    pub decoration_hint: DecorationHint,
}

#[derive(Debug)]
pub struct PendingRender {
    x: Option<i32>,
    y: Option<i32>,

}

impl Window {
    // Spawn a new window.
    // The proxy and queuehandle is passed in
    pub fn new(wm: &WindowManager, proxy: RiverWindowV1, qh: &QueueHandle<AppData>) -> Self {
        // get the node in the renderlist that corresponds to the window.
        let node = proxy.get_node(qh, ()); // make this call only once per window

        let (x, y) = wm.focused_output
            .as_ref()
            .and_then(|id| wm.outputs.get(&id))
            .map_or((0, 0), |output| (output.x, output.y));

        let (width, height) = wm.focused_output
            .as_ref()
            .and_then(|id| wm.outputs.get(&id))
            .map_or((0, 0), |output| (output.width, output.height));


        // new window object
        Window {
            proxy,
            node,
            new: true, // window is new before being processed
            closed: false,
            x: x,
            y: y,
            width: width,
            height: height,
            pointer_move_requested: None,
            pointer_resize_requested: None,
            pointer_resize_requested_edges: Edges::None,
            pending_manage: PendingManage {
                width: None,
                height: None,
                fullscreen: false,
                exit_fullscreen: false,
                maximize: false,
                unmaximize: false,
                decoration_hint: DecorationHint::NoPreference,
            },
            pending_render: PendingRender {
                x: None,
                y: None,
            }
        }
    }

    // input a position (x, y) and set the windows and nodes position
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.node.set_position(x, y); // set the absolute position of the node in the compositor's logical coordinate space
        self.x = x;
        self.y = y;
    }

    pub fn set_dimensions(&mut self, width: i32, height: i32) {
        self.proxy.propose_dimensions(width, height);
    }
}
