use std::collections::{HashMap, VecDeque};

use wayland_backend::client::ObjectId;
use wayland_client::{Proxy, QueueHandle};

use crate::river::{
    river_seat_v1::Modifiers,
    river_window_manager_v1::RiverWindowManagerV1,
    river_window_v1::{DecorationHint, Edges},
    river_xkb_bindings_v1::RiverXkbBindingsV1,
};

use crate::action::Action;
use crate::app_data::AppData;
use crate::output::Output;
use crate::seat::Seat;
use crate::seat_op::SeatOp;
use crate::window::Window;

#[derive(Debug, Default)]
pub struct WindowManager {
    pub windows: VecDeque<Window>,
    pub outputs: HashMap<ObjectId, Output>,
    pub focused_output: Option<ObjectId>,
    pub seats: HashMap<ObjectId, Seat>,
}

impl WindowManager {
    //
    pub fn handle_manage_start(
        &mut self, // mutable reference to the WindowManager struct. Borrows the object to write to it.
        proxy: &RiverWindowManagerV1, //
        river_xkb: &RiverXkbBindingsV1, //
        qh: &QueueHandle<AppData>, //
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

    pub fn handle_render_start(&mut self, proxy: &RiverWindowManagerV1) {
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
            true // retain
        });
    }

    fn remove_windows(&mut self) {
        // takes the window list. self.windows is reset to T::default()
        let old_windows = std::mem::take(&mut self.windows);
        // sets self.windows to
        self.windows = old_windows
            .into_iter() // turns old_windows into an iterable
            .filter(|window| {
                // filter windows
                if window.closed {
                    for seat in self.seats.values_mut() {
                        // if let acts as a 'match'. matching the current
                        // seat operation with SeatOp move or resize
                        // matches if seat.op = (move | resize)
                        if let SeatOp::Move { window_proxy, .. }
                        | SeatOp::Resize { window_proxy, .. } = &seat.op
                        {
                            if window_proxy == &window.proxy {
                                // SeatOp window the same as iterated windows?
                                seat.op_end(); // cancel current drag/resize of seat
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
        const SPACE: u32 = 0x20;
        const N: u32 = 0x6e;
        const Q: u32 = 0x71;
        const ESC: u32 = 0xff1b;
        const BTN_LEFT: u32 = 0x110;
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

            let pending = window.pending_manage;

            if let (Some(width), Some(height)) = (pending.width, pending.height) {
                window.set_dimensions(width, height);
            }

            match pending.decoration_hint {
                DecorationHint::OnlySupportsCsd | DecorationHint::PrefersCsd => {
                    window.proxy.use_csd();
                }
                DecorationHint::PrefersSsd | DecorationHint::NoPreference => {
                    window.proxy.use_ssd();
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
