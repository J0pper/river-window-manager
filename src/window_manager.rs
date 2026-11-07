use std::collections::HashMap;

use wayland_backend::client::ObjectId;
use wayland_client::{Proxy, QueueHandle, Dispatch, Connection};

use crate::{action::Direction, canvas::{Canvas, CanvasId}, canvas_tree::CanvasTree, river::{river_output_v1::RiverOutputV1, river_seat_v1::{Modifiers, RiverSeatV1}, river_window_manager_v1::RiverWindowManagerV1, river_window_v1::{DecorationHint, Edges, RiverWindowV1}, river_xkb_bindings_v1::RiverXkbBindingsV1}, window::{FullscreenState, MaximizeState}};

use crate::action::*;
use crate::app_data::AppData;
use crate::output::Output;
use crate::seat::Seat;
use crate::seat_op::SeatOp;
use crate::window::{Window, Visibility};

#[derive(Debug, Default)]
pub struct WindowManager {
    pub proxy_index: HashMap<RiverWindowV1, CanvasId>, // window -> canvas_id map
    pub canvas_tree: CanvasTree,
    pub outputs: HashMap<ObjectId, Output>,
    pub focused_output: Option<ObjectId>,
    pub seats: HashMap<ObjectId, Seat>,
}

impl WindowManager {
    pub fn handle_manage_start(
        &mut self, // mutable reference to the WindowManager struct. Borrows the object to write to it.
        proxy: &RiverWindowManagerV1, //
        river_xkb: &RiverXkbBindingsV1, //
        qh: &QueueHandle<AppData>, //
    ) {
        self.handle_removed_outputs();
        self.handle_closed_windows();
        self.handle_removed_seats();
        self.init_new_windows();
        self.init_new_seats(river_xkb, qh);
        self.manage_windows();
        self.manage_seats(proxy);
        proxy.manage_finish();
    }

    pub fn handle_render_start(&mut self, proxy: &RiverWindowManagerV1) {
        // for seat in &mut self.seats.values_mut() {
        //     match &seat.op {
        //         SeatOp::None => {}
        //         SeatOp::Move {
        //             window_proxy,
        //             start_x,
        //             start_y,
        //         } => {
        //             'seat_move: {
        //                 let Some(canvas_id) = self.proxy_index.get(window_proxy) else { break 'seat_move };
        //                 let Some(canvas)    = self.canvases.get_mut(canvas_id)   else { break 'seat_move };
        //
        //                 canvas.set_default_position(start_x + seat.op_dx, start_y + seat.op_dy);
        //             }
        //         }
        //         SeatOp::Resize {
        //             window_proxy,
        //             start_x,
        //             start_y,
        //             start_width,
        //             start_height,
        //             edges,
        //         } => {
        //             'resize: {
        //                 let Some(canvas_id) = self.proxy_index.get(window_proxy) else { break 'resize };
        //                 let Some(canvas)    = self.canvases.get_mut(canvas_id)   else { break 'resize };
        //                 let Some(window)    = &mut canvas.window                 else { break 'resize };
        //
        //                 let (mut x, mut y) = (*start_x, *start_y);
        //                 if let (Some(width), Some(height)) = (window.width, window.height) {
        //                     if edges.contains(Edges::Left) {
        //                         x += start_width - width;
        //                     }
        //                     if edges.contains(Edges::Top) {
        //                         y += start_height - height;
        //                     }
        //                 }
        //                 window.set_position(x, y);
        //
        //             }
        //         }
        //     }
        //
        // }

        'pending_render: {
            for canvas in self.canvases.values_mut() {
                let Some(w) = &mut canvas.window else { break 'pending_render };

                if let (Some(x), Some(y)) = (w.x, w.y) {
                    w.node.set_position(x, y);
                    (w.x, w.y) = (None, None);
                }

                if let Some(visibility) = &w.visibility {
                    match visibility {
                        Visibility::Hide => w.hide(),
                        Visibility::Show => w.show(),
                    }
                    w.visibility = None;
                }
            }
        }
        // for window in self.windows.iter() {
        //     if let Some(id) = window.id {
        //         if window.pending_render.hide {
        //             self.hidden_windows.remove(k)
        //             window.proxy.hide();
        //         }
        //         if window.pending_render.show {
        //             window.proxy.show();
        //         }
        //     }
        // }

        proxy.render_finish();
    }

    // function to destroy all unused outputs and remove them
    // from the objects 'outputs' HashMap
    fn handle_removed_outputs(&mut self) {
        // retain only outputs which has not been removed
        // the 'removed' field is presumably set to false
        // elsewhere when an output is no longer in use
        self.outputs.retain(|_, output| {
            if output.removed {
                output.proxy.destroy();
                return false; // don't retain
            }
            true // retain
        });
    }

    fn handle_closed_windows(&mut self) {
        let mut to_remove: Vec<_> = Vec::new();
        for (canvas_id, canvas) in &self.canvases {
            let Some(window) = &canvas.window else { continue };

            if window.closed {
                to_remove.push(canvas_id.clone());
            }
        }

        to_remove
            .iter()
            .for_each(|id| {
                for seat in self.seats.values_mut() {
                    // if let acts as a 'match'. matching the current
                    // seat operation with SeatOp move or resize
                    // matches if seat.op = (move | resize)
                    if let SeatOp::Move { window_proxy, .. }
                    | SeatOp::Resize { window_proxy, .. } = &seat.op
                    {
                        if let Some(canvas) = self.canvases.get(&id) {
                            // SeatOp window the same as iterated windows?
                            let Some(window) = &canvas.window else { continue };

                            if window_proxy == &window.proxy {
                                seat.op_end(); // cancel current drag/resize of seat
                            };
                        }
                    }
                }

                self.canvases.remove(&id);
            });
    }

    // remove all unused seats
    fn handle_removed_seats(&mut self) {
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
        for (_canvas_id, canvas) in self.canvases.iter_mut() {
            let Some(window) = &mut canvas.window else { continue };

            if !window.new { continue };

            if let (Some(x), Some(y)) = (window.x, window.y) {
                window.set_position(x, y); // set the position of the windows node
            }

            if let (Some(width), Some(height)) = (window.width, window.height) {
                window.set_dimensions(width, height); // send the first proposed
            }

            for seat in self.seats.values_mut() {
                seat.proxy.focus_window(&window.proxy);
            }

            window.new = false; // window has been initialized and is no longer new
        }
    }

    // initialize all new seats
    fn init_new_seats(&mut self, river_xkb: &RiverXkbBindingsV1, qh: &QueueHandle<AppData>) {
        // See xkbcommon/xkbcommon-keysyms.h
        // predefine keycodes for all buttons used by the WM
        // my guess is that River handles key input for the windows
        // all of these are for WM related things (such as bindings)
        const _NO_SYM: u32 = 0x000000;
        const SPACE: u32 = 0x20;
        const ENTER: u32 = 0xff8d;
        const C: u32 = 0x63;
        const _K: u32 = 0x6b;
        const M: u32 = 0x6d;
        const N: u32 = 0x6e;
        const O: u32 = 0x6f;
        const Q: u32 = 0x71;
        const T: u32 = 0x74;
        const ESC: u32 = 0xff1b;
        const KEY_LEFT: u32 = 0xff51;
        const KEY_RIGHT: u32 = 0xff53;
        const _SUPER_L: u32 = 0xffeb;
        const BTN_LEFT: u32 = 0x110;
        const BTN_RIGHT: u32 = 0x111;
        const _TAB: u32 = 0xff09;
        const MODS: Modifiers = Modifiers::Mod4; // define the modifier key (super)


        for seat in self.seats.values_mut() {
            if !seat.new { continue }
            
            seat.create_xkb_binding(river_xkb, qh, MODS, SPACE,     Action::SpawnFoot); // come back
            seat.create_xkb_binding(river_xkb, qh, MODS, Q,         Action::Window(WindowAction::CloseFocused)); // come back
            seat.create_xkb_binding(river_xkb, qh, MODS, N,         Action::Canvas(CanvasAction::Zoom { scale: -0.05 }));
            seat.create_xkb_binding(river_xkb, qh, MODS, M,         Action::Canvas(CanvasAction::Zoom { scale:  0.05 }));
            seat.create_xkb_binding(river_xkb, qh, MODS, KEY_LEFT,  Action::Canvas(CanvasAction::Drill { step: Depth::In, id: None }));
            seat.create_xkb_binding(river_xkb, qh, MODS, KEY_RIGHT, Action::Canvas(CanvasAction::Drill { step: Depth::Out, id: None }));

            seat.create_xkb_binding(river_xkb, qh, MODS, ESC, Action::Exit);

            seat.new = false; // seat has been initialized and is no longer new
        }
    }

    fn manage_windows(&mut self) {
        // iterate over windows mutably
        for canvas in &mut self.canvases.values_mut() {
            let Some(w) = &mut canvas.window else { return };

            // get the seat move proxy
            if let Some(seat_proxy) = w.pointer_move_requested.take() {
                // find the seat corresponding to its ID held by the proxy
                let seat = self
                    .seats
                    .get_mut(&seat_proxy.id())
                    .expect("Seat not found");
                seat.pointer_move(w); // call pointer_move function on seat come back
            }

            // get the seat resize proxy
            if let Some(seat_proxy) = w.pointer_resize_requested.take() {
                // find the seat corresponding to its ID held by the proxy
                let seat = self
                    .seats
                    .get_mut(&seat_proxy.id())
                    .expect("Seat not found");
                seat.pointer_resize(w, w.pointer_resize_requested_edges); // call pointer_move function on seat come back
            }



            if let (Some(width), Some(height)) = (w.width, w.height) {
                w.proxy.propose_dimensions(width, height);
                (w.width, w.height) = (None, None);
            }

            'block: {
                let Some(output_id) = &self.focused_output        else { break 'block };
                let Some(output)    = self.outputs.get(output_id) else { break 'block };
                let Some(f_state)   = &w.fullscreen_state         else { break 'block };

                match f_state {
                    FullscreenState::Fullscreen     => w.proxy.fullscreen(&output.proxy),
                    FullscreenState::ExitFullscreen => w.proxy.exit_fullscreen(),
                }
            }

            if let Some(maximize_state) = &w.maximize_state {
                match maximize_state {
                    MaximizeState::Maximize   => w.proxy.inform_maximized(),
                    MaximizeState::Unmaximize => w.proxy.inform_unmaximized(),
                }
            }

            if let Some(decor_hint) = w.decoration_hint {
                match decor_hint {
                    DecorationHint::OnlySupportsCsd | DecorationHint::PrefersCsd => {
                        w.proxy.use_csd();
                    }
                    DecorationHint::PrefersSsd | DecorationHint::NoPreference => {
                        w.proxy.use_ssd();
                    }
                }
            }
        }
    }

    // manage seats
    fn manage_seats(&mut self, wm_proxy: &RiverWindowManagerV1) {
        // iterate over all seats mutably
        for seat in self.seats.values_mut() {
            // new variable window_proxy if the pattern matches
            if let Some(window_proxy) = seat.interacted.take() {
                seat.proxy.focus_window(&window_proxy);
            }

            seat.do_action(wm_proxy, self); // perform action on seats window. come back
            // if seat is under release operation
            if seat.op_release {
                seat.op_end(); // come back
                seat.op_release = false; // been released
            } else {
                seat.op_manage(); // come back
            }
        }
    }

    pub fn find_and_focus_canvas(&mut self, direction: Direction) {
        let target_canvas = match direction {
            Direction::Up => 
            Direction::Down =>
            Direction::Left =>
            Direction::Right =>
        }


    }

    pub fn focus_canvas(&mut self, canvas: CanvasId) {


    }

    fn find_canvas_above(&self) {
        let Some(curr_canvas_id) = self.canvas_tree.current_canvas else { return };
        let Some(curr_canvas) = self.canvas_tree.canvases.get(&curr_canvas_id) else { return };
        let canvases: &HashMap<CanvasId, Canvas> = &self.canvas_tree.canvases;

        for (canvas_id, canvas) in canvases {


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
        use crate::river::river_window_manager_v1::Event;
        match event {
            Event::Unavailable => {
                eprintln!("Error: Another WM is already running");
                std::process::exit(1);
            },
            Event::Finished => {
                // destroy all relevant objects
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
            Event::Window { id } => {
                'block: {
                    // Rertrieve all needed variables.
                    let Some(curr_canvas_id) = state.wm.canvas_tree.current_canvas        else { break 'block };
                    // let Some(curr_canvas)    = state.wm.canvases.get_mut(&curr_canvas_id) else { break 'block };
                    let Some(output_id)      = &state.wm.focused_output                   else { break 'block };
                    let Some(output)         = state.wm.outputs.get(&output_id)           else { break 'block };

                    // Create Window & Canvas objects.
                    let     new_window = Window::create(id, qh);
                    let mut new_canvas = Canvas::create(Some(curr_canvas_id), Some(new_window.clone()));

                    // Calculate position & dimensions of new Window/Canvas.
                    let (new_x, new_y)          = state.wm.canvas_tree.calc_next_canvas_pos(curr_canvas_id);
                    let (new_width, new_height) = (output.width, output.height);

                    println!("x: {}, y: {}", new_x, new_y);
                    println!("width: {}, height: {}", new_width, new_height);

                    // Set position & dimensions of new Window/Canvas.
                    new_canvas.set_default_position(new_x, new_y);
                    new_canvas.set_default_dimensions(new_width, new_height);

                    for seat in state.wm.seats.values_mut() {
                        seat.proxy.focus_window(&new_window.proxy);
                    }

                    state.wm.proxy_index.insert(new_window.proxy, new_canvas.id.clone());
                    state.wm.canvas_tree.canvases.insert(new_canvas.id.clone(), new_canvas.clone());

                    if let Some(curr_canvas) = state.wm.canvas_tree.canvases.get_mut(&curr_canvas_id) {
                        curr_canvas.focus_window(new_canvas.id);
                        curr_canvas.sub_windows.push(new_canvas.id);
                    }

                }
            },
            Event::Output { id } => {
                state.wm.focused_output = Some(id.id());
                state.wm.outputs.insert(id.id(), Output::new(id));

                let canvas = Canvas::create(None, None);
                state.wm.current_canvas = Some(canvas.id.clone());
                state.wm.canvases.insert(canvas.id, canvas);

            },
            Event::Seat { id } => {
                state.wm.seats.insert(id.id(), Seat::new(id));
            },
        }
    }

    wayland_client::event_created_child!(AppData, RiverWindowManagerV1, [
        crate::river::river_window_manager_v1::EVT_WINDOW_OPCODE => (RiverWindowV1, ()),
        crate::river::river_window_manager_v1::EVT_OUTPUT_OPCODE => (RiverOutputV1, ()),
        crate::river::river_window_manager_v1::EVT_SEAT_OPCODE => (RiverSeatV1, ()),
    ]);
}
