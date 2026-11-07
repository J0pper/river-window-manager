use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

use crate::{app_data::AppData, river::river_output_v1::RiverOutputV1};

// A logical output. Represents the displayable area of the screen.
// Perhaps a physical monitor or a virtual display.
#[derive(Debug)]
pub struct Output {
    pub proxy: RiverOutputV1, // proxy to communicate with object. See WindowManager, remove_outputs().
    pub removed: bool,        // bool to decide whether output is active or removed.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Output {
    // create new output. Pass in the proxy
    pub fn new(proxy: RiverOutputV1) -> Self {
        // defaults to not removed
        Self {
            proxy,
            removed: false,
            x: 0,
            y: 0,
            width: 0,
            height: 0
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
        use crate::river::river_output_v1::Event;
        let output = state
            .wm
            .outputs
            .get_mut(&proxy.id())
            .expect("Output not found");
        match event {
            Event::Removed => output.removed = true,
            Event::WlOutput { name: _ } => {},
            Event::Position { x, y } => {
                (output.x, output.y) = (x, y);
                eprintln!("x: {}, y: {}", x, y);
            },
            Event::Dimensions { width, height } => {
                (output.width, output.height) = (width, height);
                eprintln!("width: {}, height: {}", width, height);
            },
            Event::CaptureSessions { count: _ } => {},
        }
    }
}
