use crate::river::river_pointer_binding_v1::RiverPointerBindingV1;

use crate::action::Action;

#[derive(Debug)]
pub struct PointerBinding {
    pub proxy: RiverPointerBindingV1,
    pub action: Action,
}
