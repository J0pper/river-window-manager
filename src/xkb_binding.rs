use crate::river::river_xkb_binding_v1::RiverXkbBindingV1;

use crate::action::Action;

#[derive(Debug)]
pub struct XkbBinding {
    pub proxy: RiverXkbBindingV1,
    pub action: Action,
}
