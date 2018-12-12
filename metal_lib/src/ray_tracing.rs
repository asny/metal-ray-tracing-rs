
use super::*;
use cocoa::foundation::{NSUInteger};

#[repr(u64)]
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum MPSIntersectionDataType {
    distance = 0,
    distancePrimitiveIndex = 1,
    distancePrimitiveIndexCoordinates = 2,
    distancePrimitiveIndexInstanceIndex = 3,
    distancePrimitiveIndexInstanceIndexCoordinates = 4
}

pub enum MPSRayIntersector {}

foreign_obj_type! {
    type CType = MPSRayIntersector;
    pub struct RayIntersector;
    pub struct RayIntersectorRef;
}

impl RayIntersector {
    pub fn new(device: &DeviceRef) -> Self {
        unsafe {
            let class = class!(MPSRayIntersector);
            let this: RayIntersector = msg_send![class, alloc];
            let this_alias: *mut Object = msg_send![this.as_ref(), initWithDevice:device];
            if this_alias.is_null() {
                panic!("[MPSRayIntersector init] failed");
            }
            this
        }
    }
}

impl RayIntersectorRef {
    pub fn set_ray_stride(&self, stride: i64) {
        unsafe {
            msg_send![self, setRayStride: stride];
        }
    }

    pub fn set_ray_data_type(&self, data_type: u64) {
        unsafe {
            msg_send![self, setRayDataType: data_type];
        }
    }

    pub fn set_intersection_stride(&self, stride: i64) {
        unsafe {
            msg_send![self, setIntersectionStride: stride];
        }
    }

    pub fn set_intersection_data_type(&self, data_type: MPSIntersectionDataType) {
        unsafe {
            msg_send![self, setIntersectionDataType: data_type];
        }
    }

    pub fn encode_intersection_to_command_buffer(&self, command_buffer: &CommandBufferRef, intersection_type: u64,
                                                 ray_buffer: &BufferRef, ray_buffer_offset: NSUInteger,
                                                 intersection_buffer: &BufferRef, intersection_buffer_offset: NSUInteger,
                                                 ray_count: NSUInteger, acceleration_structure: &TriangleAccelerationStructureRef)
    {
        unsafe {
            msg_send![self, encodeIntersectionToCommandBuffer: command_buffer
                                                                intersectionType:intersection_type
                                                                rayBuffer:ray_buffer
                                                                rayBufferOffset:ray_buffer_offset
                                                                intersectionBuffer:intersection_buffer
                                                                intersectionBufferOffset:intersection_buffer_offset
                                                                rayCount:ray_count
                                                                accelerationStructure:acceleration_structure];
        }
    }
}

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
