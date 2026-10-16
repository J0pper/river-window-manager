use wayland_backend::{ client::ObjectId };
use wayland_client::{ Connection, protocol::{wl_display::WlDisplay, wl_registry}, Dispatch, Proxy, QueueHandle };


use crate::river::{
    river_node_v1::RiverNodeV1, river_output_v1::RiverOutputV1, river_pointer_binding_v1::RiverPointerBindingV1, river_seat_v1::{RiverSeatV1}, river_window_manager_v1::RiverWindowManagerV1, river_window_v1::{DecorationHint, RiverWindowV1}, river_xkb_binding_v1::RiverXkbBindingV1, river_xkb_bindings_v1::RiverXkbBindingsV1,
};


mod window;
mod window_manager;
mod output;
mod seat;
mod seat_op;
mod action;
mod xkb_binding;
mod pointer_binding;
mod app_data;

use output::Output;
use app_data::AppData;
use window::Window;
use seat::Seat;

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
            Event::Finished => {

                std::process::exit(0);
            },
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
            Event::Window { id } => state.wm.windows.push_back(Window::new(&state.wm, id, qh)),
            Event::Output { id } => {
                state.wm.focused_output = Some(id.id());
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
            Event::DecorationHint { hint } => {
                window.pending_manage.decoration_hint = match hint.into_result() {
                    Ok(val) => val,
                    Err(_) => DecorationHint::NoPreference
                }
            },
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
            Event::CaptureSessions { count: _ } => {},
            Event::TouchMoveRequested { seat: _, touch_point: _ } => {},
            Event::TouchResizeRequested { seat: _, touch_point: _, edges: _ } => {},
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
            Event::Position {
                x,
                y,
            } => {
                (output.x, output.y) = (x, y);
                eprintln!("x: {}, y: {}", x, y);
            },
            Event::Dimensions { 
                width,
                height,
            } => {
                (output.width, output.height) = (width, height);
                eprintln!("width: {}, height: {}", width, height);
            },
            Event::CaptureSessions { count: _ } => {},
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
            Event::OpDeltaTouch { touch_point: _, dx: _, dy: _ } => {},
            Event::OpCancelTouch { touch_point: _ } => {},
            Event::OpReleaseTouch { touch_point: _ } => {},
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
