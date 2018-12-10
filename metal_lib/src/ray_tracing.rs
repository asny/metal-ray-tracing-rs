
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

impl TriangleAccelerationStructureRef {

    pub fn set_vertex_buffer(&self, buffer: Option<&BufferRef>) {
        unsafe {
            msg_send![self, setVertexBuffer: buffer];
        }
    }

    pub fn set_index_buffer(&self, buffer: Option<&BufferRef>) {
        unsafe {
            msg_send![self, setIndexBuffer: buffer];
        }
    }

    pub fn set_index_type(&self, index_type: u32) {
        unsafe {
            msg_send![self, setIndexType: index_type];
        }
    }

    pub fn set_vertex_stride(&self, stride: i64) {
        unsafe {
            msg_send![self, setVertexStride: stride];
        }
    }

    pub fn set_triangle_count(&self, count: i64) {
        unsafe {
            msg_send![self, setTriangleCount: count];
        }
    }

    pub fn rebuild(&self) {
        unsafe {
            msg_send![self, rebuild];
        }
    }
}
