use wayland_backend::{ client::ObjectId };
use wayland_client::{ Connection, protocol::{wl_display::WlDisplay, wl_registry}, Dispatch, Proxy, QueueHandle };


use crate::river::{
    river_node_v1::RiverNodeV1, river_output_v1::RiverOutputV1, river_pointer_binding_v1::RiverPointerBindingV1, river_seat_v1::{RiverSeatV1}, river_window_manager_v1::RiverWindowManagerV1, river_xkb_binding_v1::RiverXkbBindingV1, river_xkb_bindings_v1::RiverXkbBindingsV1,
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
mod canvas;
mod canvas_tree;

use app_data::AppData;

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
            Event::Pressed => {
                eprintln!("ACTION {:?}", binding.action);
                seat.pending_action = binding.action;
            },

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
