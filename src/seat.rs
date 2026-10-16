use std::collections::{HashMap, VecDeque};

use wayland_backend::{ client::ObjectId };
use wayland_client::{ Proxy, QueueHandle };


use crate::river::{
    river_seat_v1::{Modifiers, RiverSeatV1}, river_window_manager_v1::RiverWindowManagerV1, river_window_v1::{Edges, RiverWindowV1}, river_xkb_bindings_v1::RiverXkbBindingsV1,
};

use crate::action::Action;
use crate::xkb_binding::XkbBinding;
use crate::pointer_binding::PointerBinding;
use crate::seat_op::SeatOp;
use crate::app_data::AppData;
use crate::window::Window;

// This object represents a single user's collection of input devices.
// It allows the window manager to route keyboard input to windows, get
// high-level information about pointer input, define pointer bindings, etc.
#[derive(Debug)]
pub struct Seat {
    pub proxy: RiverSeatV1, // proxy used to communicate with the server
    pub new: bool, //
    pub removed: bool,
    focused: Option<RiverWindowV1>,
    pub hovered: Option<RiverWindowV1>,
    pub interacted: Option<RiverWindowV1>,
    pub xkb_bindings: HashMap<ObjectId, XkbBinding>,
    pub pointer_bindings: HashMap<ObjectId, PointerBinding>,
    pub pending_action: Action,
    pub op: SeatOp, // current operation performed by the seat
    pub op_dx: i32,
    pub op_dy: i32,
    pub op_release: bool,
}

impl Seat {
    // spawn new seat. Collection of user input devices: mouse/pointer and keyboard...
    // used to define pointer bindings. Keyboard bindings are handled through XKB
    pub fn new(proxy: RiverSeatV1) -> Self {
        Self {
            proxy,
            new: true,
            removed: false,
            focused: None,
            hovered: None,
            interacted: None,
            xkb_bindings: HashMap::new(),
            pointer_bindings: HashMap::new(),
            pending_action: Action::None,
            op: SeatOp::None,
            op_dx: 0,
            op_dy: 0,
            op_release: false,
        }
    }

    pub fn create_xkb_binding(
        &mut self,
        river_xkb: &RiverXkbBindingsV1,
        qh: &QueueHandle<AppData>,
        mods: Modifiers,
        keysym: u32,
        action: Action,
    ) {
        // define a new xkbcommon key binding using mods and keysym:
        let proxy = river_xkb.get_xkb_binding(&self.proxy, keysym, mods, qh, self.proxy.id());
        proxy.enable(); // enable key binding
        let binding = XkbBinding { proxy, action }; // get binding
        // insert binding into seats hashmap of binding
        self.xkb_bindings.insert(binding.proxy.id(), binding);
    }

    pub fn create_pointer_binding(
        &mut self,
        qh: &QueueHandle<AppData>,
        mods: Modifiers,
        button: u32,
        action: Action,
    ) {
        // define pointer binding in terms of button press and keyboard modifier
        let proxy = self
            .proxy
            .get_pointer_binding(button, mods, qh, self.proxy.id());
        proxy.enable();
        let binding = PointerBinding { proxy, action };
        self.pointer_bindings.insert(binding.proxy.id(), binding);
    }

    // handle pending actions from window seat
    pub fn do_action(&mut self, windows: &mut VecDeque<Window>, wm_proxy: &RiverWindowManagerV1) {
        match self.pending_action {
            Action::None => {eprintln!("Action::None")},
            // Don't pass WAYLAND_DEBUG on to children, the added noise makes
            // debugging the window manager itself impractical.
            Action::SpawnFoot => match std::process::Command::new("foot")
                .env_remove("WAYLAND_DEBUG")
                .spawn()
            {
                // do nothing if spawned okay
                Ok(_) => {eprintln!("Successfully spawned foot")},
                // print if error occurred
                Err(e) => eprintln!("Failed to spawn foot: {e}"),
            },
            Action::Close => {
                eprintln!("Closing window");
                // find the proxy for the window focused by the seat
                if let Some(window_proxy) = self.focused.as_ref() {
                    window_proxy.close();
                }
            },
            Action::FocusNext => {
                if !windows.is_empty() {
                    windows.rotate_left(1);
                    self.focus_top(windows);
                }
            },
            Action::Move => {
                if let (Some(window_proxy), SeatOp::None) = (self.hovered.as_ref(), &self.op) {
                    let window = windows
                        .iter()
                        .find(|window| &window.proxy == window_proxy)
                        .expect("Hovered window not found");
                    self.pointer_move(window);
                }
            },
            Action::Resize => {
                if let (Some(window_proxy), SeatOp::None) = (self.hovered.as_ref(), &self.op) {
                    let window = windows
                        .iter()
                        .find(|window| &window.proxy == window_proxy)
                        .expect("Hovered window not found");
                    self.pointer_resize(window, Edges::Bottom.union(Edges::Right));
                }
            },
            Action::Exit => wm_proxy.exit_session(),
        }
        self.pending_action = Action::None;
    }

    // 
    pub fn op_end(&mut self) {
        // pull out window proxy from the seats op variable
        if let SeatOp::Resize { window_proxy, .. } = &self.op {
            window_proxy.inform_resize_end(); // inform window it is no longer being resized
        }
        self.proxy.op_end(); // end interactive operation
        self.op = SeatOp::None; // set seat option to none
    }

    pub fn op_manage(&mut self) {
        match &self.op {
            SeatOp::None | SeatOp::Move { .. } => {},
            SeatOp::Resize {
                window_proxy,
                start_width,
                start_height,
                edges,
                ..
            } => {
                // update dimensions of window when resizing
                let (mut width, mut height) = (*start_width, *start_height);
                if edges.contains(Edges::Left) {
                    width -= self.op_dx;
                }
                if edges.contains(Edges::Right) {
                    width += self.op_dx;
                }
                if edges.contains(Edges::Top) {
                    height -= self.op_dy;
                }
                if edges.contains(Edges::Bottom) {
                    height += self.op_dy;
                }
                // stop width and height from going under 1
                window_proxy.propose_dimensions(width.max(1), height.max(1));
            }
        }
    }

    pub fn focus_top(&mut self, windows: &VecDeque<Window>) {
        // get back element or None if empty
        match windows.back() {
            Some(window) => {
                self.proxy.focus_window(&window.proxy); // plz give keyboard focus
                window.node.place_top(); // place window on top plz
                // clone focused window. Avoids borrowing.
                self.focused = Some(window.proxy.clone()); 
            }
            None => {
                // clear focus of seat if windows is empty
                self.proxy.clear_focus();
                self.focused = None;
            }
        }
    }

    pub fn pointer_move(&mut self, window: &Window) {
        // clone interacted window. Avoids borrowing.
        self.interacted = Some(window.proxy.clone());
        // start interactive pointer operation. op_delta events are sent
        self.proxy.op_start_pointer();
        // set seats operation to move
        self.op = SeatOp::Move {
            window_proxy: window.proxy.clone(),
            start_x: window.x,
            start_y: window.y,
        };
        // set deltas to zero on op start
        self.op_dx = 0;
        self.op_dy = 0;
    }

    pub fn pointer_resize(&mut self, window: &Window, edges: Edges) {
        // clone interacted window. Avoids borrowing.
        self.interacted = Some(window.proxy.clone());
        // start interactive pointer operation. op_delta events are sent
        self.proxy.op_start_pointer();
        // inform window that it is being resized.
        // WM has the reponsibility to handle position and size
        window.proxy.inform_resize_start();
        self.op = SeatOp::Resize {
            window_proxy: window.proxy.clone(),
            start_x: window.x,
            start_y: window.y,
            start_width: window.width,
            start_height: window.height,
            edges
        };
        // set deltas to zero on op start
        self.op_dx = 0;
        self.op_dy = 0;
    }
}
