use std::collections::{HashMap, VecDeque};

use wayland_backend::{ client::ObjectId };
use wayland_client::{ Connection, protocol::{wl_display::WlDisplay, wl_registry}, Dispatch, Proxy, QueueHandle };


use crate::river::{
    river_node_v1::RiverNodeV1, river_output_v1::RiverOutputV1, river_pointer_binding_v1::RiverPointerBindingV1, river_seat_v1::{Modifiers, RiverSeatV1}, river_window_manager_v1::RiverWindowManagerV1, river_window_v1::{DecorationHint, Edges, RiverWindowV1}, river_xkb_binding_v1::RiverXkbBindingV1, river_xkb_bindings_v1::RiverXkbBindingsV1,
};

mod river {
    pub extern crate wayland_client;
    pub use wayland_client::protocol::*;

    mod interfaces {
        pub(super) mod rwm {
            pub use wayland_client::protocol::__interfaces::*;
            wayland_scanner::generate_interfaces!("./protocol/river-window-management-v1.xml");
        }

        pub(super) mod rxkb {
            use super::rwm::*;
            wayland_scanner::generate_interfaces!("./protocol/river-xkb-bindings-v1.xml");
        }
    }

    use self::interfaces::rwm::*;
    use self::interfaces::rxkb::*;
    wayland_scanner::generate_client_code!("./protocol/river-window-management-v1.xml");
    wayland_scanner::generate_client_code!("./protocol/river-xkb-bindings-v1.xml");
}

// User can only do one action at a time.
#[derive(Debug, Clone, Copy)]
enum Action {
    None,
    SpawnFoot,
    Close,
    FocusNext,
    Move,
    Resize,
    Exit,
}

// Operations on Seats.
#[derive(Debug, Clone)]
enum SeatOp {
    None,
    Move {
        window_proxy: RiverWindowV1,
        start_x: i32,
        start_y: i32,
    },
    Resize {
        window_proxy: RiverWindowV1,
        start_x: i32,
        start_y: i32,
        start_width: i32,
        start_height: i32,
        edges: Edges,
    },
}

#[derive(Debug, Default)]
struct AppData {
    river_wm: Option<RiverWindowManagerV1>,
    river_xkb: Option<RiverXkbBindingsV1>,
    wm: WindowManager,
}

#[derive(Debug, Default)]
struct WindowManager {
    windows: VecDeque<Window>,
    outputs: HashMap<ObjectId, Output>,
    seats: HashMap<ObjectId, Seat>,
}

#[derive(Debug)]
struct Window {
    proxy: RiverWindowV1,
    node: RiverNodeV1,
    new: bool,
    closed: bool,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    pointer_move_requested: Option<RiverSeatV1>,
    pointer_resize_requested: Option<RiverSeatV1>,
    pointer_resize_requested_edges: Edges,
}

// A logical output. Represents the displayable area of the screen.
// Perhaps a physical monitor or a virtual display.
#[derive(Debug)]
struct Output {
    proxy: RiverOutputV1, // proxy to communicate with object. See WindowManager, remove_outputs().
    removed: bool,        // bool to decide whether output is active or removed.
}

// This object represents a single user's collection of input devices.
// It allows the window manager to route keyboard input to windows, get
// high-level information about pointer input, define pointer bindings, etc.
#[derive(Debug)]
struct Seat {
    proxy: RiverSeatV1, // proxy used to communicate with the server
    new: bool, //
    removed: bool,
    focused: Option<RiverWindowV1>,
    hovered: Option<RiverWindowV1>,
    interacted: Option<RiverWindowV1>,
    xkb_bindings: HashMap<ObjectId, XkbBinding>,
    pointer_bindings: HashMap<ObjectId, PointerBinding>,
    pending_action: Action,
    op: SeatOp, // current operation performed by the seat
    op_dx: i32,
    op_dy: i32,
    op_release: bool,
}

#[derive(Debug)]
struct XkbBinding {
    proxy: RiverXkbBindingV1,
    action: Action,
}

#[derive(Debug)]
struct PointerBinding {
    proxy: RiverPointerBindingV1,
    action: Action,
}

impl WindowManager {
    // 
    fn handle_manage_start(
        &mut self,                      // mutable reference to the WindowManager struct. Borrows the object to write to it.
        proxy: &RiverWindowManagerV1,   // 
        river_xkb: &RiverXkbBindingsV1, //
        qh: &QueueHandle<AppData>,      //
    ) {
        self.remove_outputs();
        self.remove_windows();
        self.remove_seats();
        self.init_new_windows();
        self.init_new_seats(river_xkb, qh);
        self.manage_windows();
        self.manage_seats(proxy);
        proxy.manage_finish();
    }

    fn handle_render_start(&mut self, proxy: &RiverWindowManagerV1) {
        for seat in &mut self.seats.values_mut() {
            match &seat.op {
                SeatOp::None => {}
                SeatOp::Move {
                    window_proxy,
                    start_x,
                    start_y,
                } => {
                    if let Some(window) = self
                        .windows
                        .iter_mut()
                        .find(|window| &window.proxy == window_proxy)
                    {
                        window.set_position(start_x + seat.op_dx, start_y + seat.op_dy);
                    }
                }
                SeatOp::Resize {
                    window_proxy,
                    start_x,
                    start_y,
                    start_width,
                    start_height,
                    edges,
                } => {
                    if let Some(window) = self
                        .windows
                        .iter_mut()
                        .find(|window| &window.proxy == window_proxy)
                    {
                        let (mut x, mut y) = (*start_x, *start_y);
                        if edges.contains(Edges::Left) {
                            x += start_width - window.width;
                        }
                        if edges.contains(Edges::Top) {
                            y += start_height - window.height;
                        }
                        window.set_position(x, y);
                    }
                }
            }
        }

        proxy.render_finish();
    }

    // function to destroy all unused outputs and remove them
    // from the objects 'outputs' HashMap
    fn remove_outputs(&mut self) {
        // retain only outputs which has not been removed
        // the 'removed' field is presumably set to false
        // elsewhere when an output is no longer in use
        self.outputs.retain(|_, output| {
            if output.removed {
                output.proxy.destroy();
                return false; // don't retain
            }
            true              // retain
        });
    }


    fn remove_windows(&mut self) {
        // takes the window list. self.windows is reset to T::default()
        let old_windows = std::mem::take(&mut self.windows);
        // sets self.windows to 
        self.windows = old_windows
            .into_iter() // turns old_windows into an iterable
            .filter(|window| { // filter windows
                if window.closed {
                    for seat in self.seats.values_mut() {
                        // if let acts as a 'match'. matching the current
                        // seat operation with SeatOp move or resize
                        // matches if seat.op = (move | resize)
                        if let SeatOp::Move { window_proxy, .. }
                        | SeatOp::Resize { window_proxy, .. } = &seat.op
                        {
                            if window_proxy == &window.proxy { // SeatOp window the same as iterated windows?
                                seat.op_end();                 // cancel current drag/resize of seat
                            }
                        }
                    }
                    return false; // reject all old windows
                }
                true
            })
            .collect(); // build a (empty) collection
    }

    // remove all unused seats
    fn remove_seats(&mut self) {
        self.seats.retain(|_, seat| {
            // if seat is removed, use the proxies to destroy the seat and bindings
            if seat.removed {
                seat.xkb_bindings
                    .values_mut()
                    .for_each(|binding| binding.proxy.destroy()); // destroy each xkb binding
                seat.pointer_bindings
                    .values_mut()
                    .for_each(|binding| binding.proxy.destroy()); // destroy each pointer binding
                seat.proxy.destroy(); // destroy the seat itself
                return false; // don't retain destroyed seat
            } 
            true // seat should not be destroyed. retain
        });
    }

    // initialize all new windows
    fn init_new_windows(&mut self) {
        // loop over all new windows
        for window in self.windows.iter_mut().filter(|w| w.new) {
            window.set_position(window.x, window.y); // set the position of the windows node
            window.proxy.propose_dimensions(window.width, window.height); // use the proxy to
                                                                          // propose dimensions
            window.new = false; // window has been initialized and is no longer new
        }
    }

    // initialize all new seats
    fn init_new_seats(&mut self, river_xkb: &RiverXkbBindingsV1, qh: &QueueHandle<AppData>) {
        // See xkbcommon/xkbcommon-keysyms.h
        // predefine keycodes for all buttons used by the WM
        // my guess is that River handles key input for the windows
        // all of these are for WM related things (such as bindings)
        const SPACE:     u32 = 0x20;
        const N:         u32 = 0x6e;
        const Q:         u32 = 0x71;
        const ESC:       u32 = 0xff1b;
        const BTN_LEFT:  u32 = 0x110;
        const BTN_RIGHT: u32 = 0x111;
        const MODS: Modifiers = Modifiers::Mod4; // define the modifier key (super)

        for seat in self.seats.values_mut() {
            if seat.new {
                seat.create_xkb_binding(river_xkb, qh, MODS, SPACE, Action::SpawnFoot); // come back
                seat.create_xkb_binding(river_xkb, qh, MODS, Q, Action::Close); // come back
                seat.create_xkb_binding(river_xkb, qh, MODS, N, Action::FocusNext);
                seat.create_xkb_binding(river_xkb, qh, MODS, ESC, Action::Exit);
                seat.create_pointer_binding(qh, MODS, BTN_LEFT, Action::Move);
                seat.create_pointer_binding(qh, MODS, BTN_RIGHT, Action::Resize);
                seat.new = false; // seat has been initialized and is no longer new
            }
        }
    }

    fn manage_windows(&mut self) {
        // iterate over windows mutably
        for window in self.windows.iter_mut() {
            // get the seat move proxy
            if let Some(seat_proxy) = window.pointer_move_requested.take() {
                // find the seat corresponding to its ID held by the proxy
                let seat = self
                    .seats
                    .get_mut(&seat_proxy.id())
                    .expect("Seat not found");
                seat.pointer_move(window); // call pointer_move function on seat come back
            }
            // get the seat resize proxy
            if let Some(seat_proxy) = window.pointer_resize_requested.take() {
                // find the seat corresponding to its ID held by the proxy
                let seat = self
                    .seats
                    .get_mut(&seat_proxy.id())
                    .expect("Seat not found");
                seat.pointer_resize(window, window.pointer_resize_requested_edges); // call pointer_move function on seat come back
            }
        }
    }

    // manage seats
    fn manage_seats(&mut self, wm_proxy: &RiverWindowManagerV1) {
        // iterate over all seats mutably
        for seat in self.seats.values_mut() {
            // new variable window_proxy if the pattern matches
            if let Some(window_proxy) = seat.interacted.take() {
                // find the index in self.windows of the window the seat interacted with
                let i = self
                    .windows
                    .iter()
                    .position(|window| window.proxy == window_proxy)
                    .expect("Interacted window not found");
                let window = self.windows.remove(i).unwrap();
                self.windows.push_back(window);
            }
            seat.focus_top(&self.windows); // set the window as the seats top most. come back
            seat.do_action(&mut self.windows, wm_proxy); // perform action on seats window. come back
            // if seat is under release operation
            if seat.op_release {
                seat.op_end(); // come back
                seat.op_release = false; // been released
            } else {
                seat.op_manage(); // come back
            }
        }
    }
}

impl Window {
    // Spawn a new window.
    // The proxy and queuehandle is passed in
    fn new(proxy: RiverWindowV1, qh: &QueueHandle<AppData>) -> Self {
        // get the node in the renderlist that corresponds to the window.
        let node = proxy.get_node(qh, ()); // make this call only once per window

        // new window object
        Window {
            proxy,
            node,
            new: true, // window is new before being processed
            closed: false,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            pointer_move_requested: None,
            pointer_resize_requested: None,
            pointer_resize_requested_edges: Edges::None,
        }
    }

    // input a position (x, y) and set the windows and nodes position
    fn set_position(&mut self, x: i32, y: i32) {
        self.node.set_position(x, y); // set the absolute position of the node in the compositor's logical coordinate space
        self.x = x;
        self.y = y;
    }
}

impl Output {
    // create new output. Pass in the proxy
    fn new(proxy: RiverOutputV1) -> Self {
        // defaults to not removed
        Self {
            proxy,
            removed: false,
        }
    }
}

impl Seat {
    // spawn new seat. Collection of user input devices: mouse/pointer and keyboard...
    // used to define pointer bindings. Keyboard bindings are handled through XKB
    fn new(proxy: RiverSeatV1) -> Self {
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

    fn create_xkb_binding(
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

    fn create_pointer_binding(
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
    fn do_action(&mut self, windows: &mut VecDeque<Window>, wm_proxy: &RiverWindowManagerV1) {
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
    fn op_end(&mut self) {
        // pull out window proxy from the seats op variable
        if let SeatOp::Resize { window_proxy, .. } = &self.op {
            window_proxy.inform_resize_end(); // inform window it is no longer being resized
        }
        self.proxy.op_end(); // end interactive operation
        self.op = SeatOp::None; // set seat option to none
    }

    fn op_manage(&mut self) {
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

    fn focus_top(&mut self, windows: &VecDeque<Window>) {
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

    fn pointer_move(&mut self, window: &Window) {
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

    fn pointer_resize(&mut self, window: &Window, edges: Edges) {
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


impl Dispatch<wl_registry::WlRegistry, ()> for AppData {
    // event is called on AppData when the compositor sends any of two events from the registry.
    // global: a new cabability is advertised 
    // global_remove: a cabability was destroyed
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(), // the second generic passed to dispatch is the unit type
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        // filter out global_remove events
        // pattern matches against name, interface and version
        // and pulls them out from event.
        // (global_remove doesn't have these fields)
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            // define constants for current protocol versions
            const RIVER_WINDOW_MANAGER_V1_VERSION: u32 = 4;
            const RIVER_XKB_BINDINGS_V1_VERSION: u32 = 1;

            // borrow interface
            match interface.as_str() {
                // if the global event related to the WM
                "river_window_manager_v1" => {
                    // check if server is running an outdated protocol version
                    if version < RIVER_WINDOW_MANAGER_V1_VERSION {
                        eprintln!(
                            "Server river_window_manager_v1 v{version}, but we need at least v{RIVER_WINDOW_MANAGER_V1_VERSION}",
                        );
                        std::process::exit(1);
                    }
                    // object exists on the server is proved by the global event
                    // create a client side proxy proxy that server side resource so we can talk to it
                    // first argument in angle brackets denotes return type of bind
                    // the rest are place holders, denoting the type:
                    // () and type of qh respectively.
                    // this is our window manager object on the server which we will use to
                    // communicate with
                    let wm = registry.bind::<RiverWindowManagerV1, _, _>(
                        name,
                        RIVER_WINDOW_MANAGER_V1_VERSION,
                        qh,
                        (),
                    );
                    // store the river window manager in AppData
                    state.river_wm = Some(wm);
                },
                // if the global event related to the XKB bindings
                "river_xkb_bindings_v1" => {
                    // check if server is running an outdated protocol version
                    if version < RIVER_XKB_BINDINGS_V1_VERSION {
                        eprintln!(
                            "Server supports river_xkb_bindings_v1 v{version}, but we need at least v{RIVER_XKB_BINDINGS_V1_VERSION}",
                        );
                        std::process::exit(1);
                    }
                    let xkb = registry.bind::<RiverXkbBindingsV1, _, _>(
                        name,
                        RIVER_XKB_BINDINGS_V1_VERSION,
                        qh,
                        (),
                    );
                    state.river_xkb = Some(xkb);
                },
                _ => {},
            }
        }
    }
}

impl Dispatch<RiverWindowManagerV1, ()> for AppData {
    fn event(
        state: &mut Self,
        proxy: &RiverWindowManagerV1,
        event: <RiverWindowManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use river::river_window_manager_v1::Event;
        match event {
            Event::Unavailable => {
                eprintln!("Error: Another WM is already running");
                std::process::exit(1);
            },
            Event::Finished => std::process::exit(0),
            Event::ManageStart => {
                let river_xkb = state
                    .river_xkb
                    .as_ref()
                    .expect("river_xkb_bindings_v1 missing");
                state.wm.handle_manage_start(proxy, river_xkb, qh);
            },
            Event::RenderStart => state.wm.handle_render_start(proxy),
            Event::SessionLocked => {},
            Event::SessionUnlocked => {},
            Event::Window { id } => state.wm.windows.push_back(Window::new(id, qh)),
            Event::Output { id } => {
                state.wm.outputs.insert(id.id(), Output::new(id));
            },
            Event::Seat { id } => {
                state.wm.seats.insert(id.id(), Seat::new(id));
            },
        }
    }

    wayland_client::event_created_child!(AppData, RiverWindowManagerV1, [
        river::river_window_manager_v1::EVT_WINDOW_OPCODE => (RiverWindowV1, ()),
        river::river_window_manager_v1::EVT_OUTPUT_OPCODE => (RiverOutputV1, ()),
        river::river_window_manager_v1::EVT_SEAT_OPCODE => (RiverSeatV1, ()),
    ]);
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
        use river::river_window_v1::Event;
        let window = match state.wm.windows.iter_mut().find(|o| &o.proxy == proxy) {
            Some(window) => window,
            None => return,
        };
        match event {
            Event::Closed => window.closed = true,
            Event::DimensionsHint {
                min_width: _,
                min_height: _,
                max_width: _,
                max_height: _,
            } => {}
            Event::Dimensions { width, height } => (window.width, window.height) = (width, height),
            Event::AppId { app_id: _ } => {},
            Event::Title { title: _ } => {},
            Event::Parent { parent: _ } => {},
            Event::DecorationHint { hint: _ } => {},
            Event::PointerMoveRequested { seat } => window.pointer_move_requested = Some(seat),
            Event::PointerResizeRequested { seat, edges } => {
                window.pointer_resize_requested = Some(seat);
                window.pointer_resize_requested_edges =
                    edges.into_result().expect("Invalid edges for resize");
            },
            Event::ShowWindowMenuRequested { x: _, y: _ } => {},
            Event::MaximizeRequested => {},
            Event::UnmaximizeRequested => {},
            Event::FullscreenRequested { output: _ } => {},
            Event::ExitFullscreenRequested => {},
            Event::MinimizeRequested => {},
            Event::UnreliablePid { unreliable_pid: _ } => {},
            Event::PresentationHint { .. } => {},
            Event::Identifier { .. } => {},
        }
    }
}

impl Dispatch<RiverOutputV1, ()> for AppData {
    fn event(
        state: &mut Self,
        proxy: &RiverOutputV1,
        event: <RiverOutputV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use river::river_output_v1::Event;
        let output = state
            .wm
            .outputs
            .get_mut(&proxy.id())
            .expect("Output not found");
        match event {
            Event::Removed => output.removed = true,
            Event::WlOutput { name: _ } => {},
            Event::Position { x: _, y: _ } => {},
            Event::Dimensions { 
                width: _,
                height: _,
            } => {},
        }
    }
}

impl Dispatch<RiverSeatV1, ()> for AppData {
    fn event(
        state: &mut Self,
        proxy: &RiverSeatV1,
        event: <RiverSeatV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use river::river_seat_v1::Event;
        let seat = state.wm.seats.get_mut(&proxy.id()).expect("Seat not found");
        match event {
            Event::Removed => seat.removed = true,
            Event::WlSeat { name: _ } => {},
            Event::PointerEnter { window } => seat.hovered = Some(window),
            Event::PointerLeave => seat.hovered = None,
            Event::WindowInteraction { window } => seat.interacted = Some(window),
            Event::ShellSurfaceInteraction {
                shell_surface: _shell_surface,
            } => {},
            Event::OpDelta { dx, dy } => (seat.op_dx, seat.op_dy) = (dx, dy),
            Event::OpRelease => seat.op_release = true,
            Event::PointerPosition { x: _, y: _ } => {},
        }
    }
}

impl Dispatch<RiverXkbBindingV1, ObjectId> for AppData {
    fn event(
        state: &mut Self,
        proxy: &RiverXkbBindingV1,
        event: <RiverXkbBindingV1 as Proxy>::Event,
        data: &ObjectId,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use river::river_xkb_binding_v1::Event;
        let seat = state.wm.seats.get_mut(data).expect("Seat not found");
        let binding = seat
            .xkb_bindings
            .get(&proxy.id())
            .expect("xkb_binding not found");
        match event {
            Event::Pressed => seat.pending_action = binding.action,
            Event::Released => {}
            Event::StopRepeat => {}
        }
    }
}

impl Dispatch<RiverPointerBindingV1, ObjectId> for AppData {
    fn event(
        state: &mut Self,
        proxy: &RiverPointerBindingV1,
        event: <RiverPointerBindingV1 as Proxy>::Event,
        data: &ObjectId,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use river::river_pointer_binding_v1::Event;
        let seat = state.wm.seats.get_mut(data).expect("Seat not found");
        let binding = seat
            .pointer_bindings
            .get(&proxy.id())
            .expect("xkb_binding not found");
        match event {
            Event::Pressed => seat.pending_action = binding.action,
            Event::Released => {},
        }
    }
}

wayland_client::delegate_noop!(AppData: ignore RiverXkbBindingsV1);
wayland_client::delegate_noop!(AppData: ignore RiverNodeV1);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Queue up a get_registry event.
    let conn: Connection = Connection::connect_to_env()?;
    let display: WlDisplay = conn.display();
    let mut event_queue: wayland_client::EventQueue<_> = conn.new_event_queue();
    let _registry = display.get_registry(&event_queue.handle(), ());

    // Initial State
    let mut app_data: AppData = AppData::default();

    // Roundtrip to process the get_registry event and bind interfaces.
    event_queue.roundtrip(&mut app_data)?;
    if app_data.river_wm.is_none() {
        eprintln!("river_window_manager_v1 global not found! Is river running?");
        std::process::exit(1);
    }
    if app_data.river_xkb.is_none() {
        eprintln!("river_xkb_bindings_v1 global not found! Is river running with xkb support?");
        std::process::exit(1);
    }

    loop {
        event_queue.blocking_dispatch(&mut app_data)?;
    }
}
