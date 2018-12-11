
use metal::*;
use std::mem;

pub struct Intersector {
    acceleration_structure: TriangleAccelerationStructure,
    ray_intersector: RayIntersector,
    ray_buffer: Buffer,
    intersection_buffer: Buffer,
    ray_count: u64
}

impl Intersector {

    pub fn new(device: &DeviceRef) -> Intersector
    {
        let ray_count = 1;
        // Triangle data
        let vertex_data = [
            0.25f32, 0.25, 0.0,
            0.75, 0.25, 0.0,
            0.50, 0.75, 0.0
        ];
        let index_data = [
            0u32, 1, 2
        ];

        // Build acceleration structure:
        let vertex_buffer = device.new_buffer_with_data( unsafe { mem::transmute(vertex_data.as_ptr()) },
                                     (vertex_data.len() * mem::size_of::<f32>()) as u64,
                                     MTLResourceOptions::CPUCacheModeDefaultCache);
        let index_buffer = device.new_buffer_with_data( unsafe { mem::transmute(index_data.as_ptr()) },
                                     (index_data.len() * mem::size_of::<u32>()) as u64,
                                     MTLResourceOptions::CPUCacheModeDefaultCache);

        let acceleration_structure = TriangleAccelerationStructure::new(&device);
        acceleration_structure.set_vertex_buffer(Some(&vertex_buffer));
        acceleration_structure.set_vertex_stride((3 * mem::size_of::<f32>()) as i64);
        acceleration_structure.set_index_buffer(Some(&index_buffer));
        acceleration_structure.set_index_type(32); // MPSDataType: uInt32
        acceleration_structure.set_triangle_count(1);
        acceleration_structure.rebuild();

        // Setup ray intersector:
        let ray_intersector = RayIntersector::new(&device);
        ray_intersector.set_ray_stride(8 * std::mem::size_of::<f32>() as i64);
        ray_intersector.set_ray_data_type(1); // MPSRayDataTypeOriginMinDistanceDirectionMaxDistance
        ray_intersector.set_intersection_stride(8 * std::mem::size_of::<f32>() as i64);
        ray_intersector.set_intersection_data_type(4); // MPSIntersectionDataTypeDistancePrimitiveIndexCoordinates

        // Create buffers:
        let ray_buffer = device.new_buffer(ray_count * 8 * std::mem::size_of::<f32>() as u64, MTLResourceOptions::StorageModePrivate);
        let intersection_buffer = device.new_buffer(ray_count * 8 * std::mem::size_of::<f32>() as u64, MTLResourceOptions::StorageModePrivate);

        Intersector {acceleration_structure, ray_intersector, ray_buffer, intersection_buffer, ray_count}
    }

    pub fn encode_into(&self, command_buffer: &CommandBufferRef)
    {
        self.ray_intersector.encode_intersection_to_command_buffer(command_buffer, 0, //MPSIntersectionTypeNearest
                                                                   &self.ray_buffer, 0,
                                                                   &self.intersection_buffer, 0,
                                                                   self.ray_count, &self.acceleration_structure);
    }

}