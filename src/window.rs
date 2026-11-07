use wayland_client::{ Connection, Dispatch, Proxy, QueueHandle };

use crate::{river::{river_node_v1::RiverNodeV1, river_seat_v1::RiverSeatV1, river_window_v1::{DecorationHint, Edges, RiverWindowV1}}};

use crate::app_data::AppData;
use crate::window_manager::WindowManager;

#[derive(Debug, Clone)]
pub struct WindowId(String);


#[derive(Debug, Clone)]
pub struct Window {
// pub wm: &WindowManager,
    pub proxy: RiverWindowV1,
    pub node: RiverNodeV1,

    pub app_id: Option<String>,
    pub title: Option<String>,
    pub unreliable_pid: Option<i32>,
    pub id: Option<WindowId>,

    pub new: bool,
    pub closed: bool,

    pub min_width: i32,
    pub min_height: i32,
    pub max_width: i32,
    pub max_height: i32,

    pub parent: Option<RiverWindowV1>,

    pub pointer_move_requested: Option<RiverSeatV1>,
    pub pointer_resize_requested: Option<RiverSeatV1>,
    pub pointer_resize_requested_edges: Edges,

    // pub pending_manage: Option<PendingManage>,
    // pub pending_render: Option<PendingRender>,

    pub width: Option<i32>,
    pub height: Option<i32>,
    pub maximize_state: Option<MaximizeState>,
    pub fullscreen_state: Option<FullscreenState>,
    pub minimize: Option<bool>,

    pub decoration_hint: Option<DecorationHint>,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub visibility: Option<Visibility>,
}

#[derive(Debug, Clone)]
pub enum Visibility {
    Hide,
    Show,
}

#[derive(Debug, Clone)]
pub enum FullscreenState {
    Fullscreen,
    ExitFullscreen,
}

#[derive(Debug, Clone)]
pub enum MaximizeState {
    Maximize,
    Unmaximize,
}

impl Window {
    // Spawn a new window.
    // The proxy and queuehandle is passed in
    pub fn create(proxy: RiverWindowV1, qh: &QueueHandle<AppData>) -> Self {
        // get the node in the renderlist that corresponds to the window.
        let node = proxy.get_node(qh, ()); // make this call only once per window

        // new window object
        Self {
            // wm: wm,
            proxy: proxy,
            node: node,
            app_id: None,
            title: None,
            unreliable_pid: None,
            id: None,
            new: true, // window is new before being processed
            closed: false,
            min_width: 0,
            min_height: 0,
            max_width: 0,
            max_height: 0,
            parent: None,
            pointer_move_requested: None,
            pointer_resize_requested: None,
            pointer_resize_requested_edges: Edges::None,
            width: None,
            height: None,
            maximize_state: None,
            fullscreen_state: None,
            minimize: None,
            decoration_hint: None,
            x: None,
            y: None,
            visibility: Some(Visibility::Show),
        }
    }

    pub fn destroy(&mut self) {
        self.node.destroy();
        self.proxy.destroy();
    }

    // input a position (x, y) and set the windows and nodes position
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.x = Some(x);
        self.y = Some(y);
    }

    pub fn set_dimensions(&mut self, width: i32, height: i32) {
        self.width  = Some(width);
        self.height = Some(height);
    }

    pub fn set_dimension_hints(&mut self, min_width: i32, max_width: i32, min_height: i32, max_height: i32) {
        self.min_width  = min_width;
        self.min_height = min_height;
        self.max_width  = max_width;
        self.max_height = max_height;
        self.has_fixed_size();
        // todo!("Implement proper floating windows.");
    }

    fn has_fixed_size(&mut self) -> bool {
        ((self.min_width, self.min_height) != (0, 0)) &&
        (self.min_width == self.max_width || self.min_height == self.max_height)
    }

    pub fn hide(&mut self) {
        self.proxy.hide();
    }

    pub fn show(&mut self) {
        self.proxy.show();
    }
}

impl Dispatch<RiverWindowV1, ()> for AppData {
    fn event(
        state: &mut Self,
        proxy: &RiverWindowV1,
        event: <RiverWindowV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use crate::river::river_window_v1::Event;
        let Some(canvas_id) = state.wm.proxy_index.get(proxy)     else { return };
        let Some(canvas)    = state.wm.canvases.get_mut(canvas_id) else { return };
        let Some(window)    = &mut canvas.window                   else { return };
        match event {
            Event::Closed => {
                window.closed = true;
            },
            Event::DimensionsHint {
                min_width,
                min_height,
                max_width,
                max_height,
            } => {
                window.set_dimension_hints(min_width, min_height, max_width, max_height);
            }
            Event::Dimensions { width, height } => window.set_dimensions(width, height),
            Event::AppId { app_id } => window.app_id = app_id,
            Event::Title { title } => window.title = title,
            Event::Parent { parent } => window.parent = parent,
            Event::DecorationHint { hint } => {
                window.decoration_hint = match hint.into_result() {
                    Ok(val) => Some(val),
                    Err(_) => Some(DecorationHint::NoPreference)
                }
            },
            Event::PointerMoveRequested { seat } => window.pointer_move_requested = Some(seat),
            Event::PointerResizeRequested { seat, edges } => {
                window.pointer_resize_requested = Some(seat);
                window.pointer_resize_requested_edges =
                    edges.into_result().expect("Invalid edges for resize");
            },
            Event::ShowWindowMenuRequested { x: _, y: _ } => {},
            Event::MaximizeRequested => window.maximize_state = Some(MaximizeState::Maximize),
            Event::UnmaximizeRequested => window.maximize_state = Some(MaximizeState::Unmaximize),
            Event::FullscreenRequested { output: _ } => window.fullscreen_state = Some(FullscreenState::Fullscreen),
            Event::ExitFullscreenRequested => window.fullscreen_state = Some(FullscreenState::ExitFullscreen),
            Event::MinimizeRequested => window.minimize = Some(true),
            Event::UnreliablePid { unreliable_pid } => window.unreliable_pid = Some(unreliable_pid),
            Event::PresentationHint { .. } => {},
            Event::Identifier { identifier } => window.id = Some(identifier),
            Event::CaptureSessions { count: _ } => {},
            Event::TouchMoveRequested { seat: _, touch_point: _ } => {},
            Event::TouchResizeRequested { seat: _, touch_point: _, edges: _ } => {},
        }
    }
}
