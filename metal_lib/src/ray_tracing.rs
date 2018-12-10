
use super::*;

pub enum MPSTriangleAccelerationStructure {}

foreign_obj_type! {
    type CType = MPSTriangleAccelerationStructure;
    pub struct TriangleAccelerationStructure;
    pub struct TriangleAccelerationStructureRef;
}

impl TriangleAccelerationStructure {
    pub fn new(device: &DeviceRef) -> Self {
        unsafe {
            let class = class!(MPSTriangleAccelerationStructure);
            let this: TriangleAccelerationStructure = msg_send![class, alloc];
            let this_alias: *mut Object = msg_send![this.as_ref(), initWithDevice:device];
            if this_alias.is_null() {
                panic!("[MPSTriangleAccelerationStructure init] failed");
            }
            this
        }
    }
}

