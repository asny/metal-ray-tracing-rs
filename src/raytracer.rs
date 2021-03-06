
use metal::*;
use std::mem;
use std::fs::File;
use std::io::prelude::*;
use cgmath::*;
use mersenne_twister::MT19937;
use rand::Rng;

const NOISE_BLOCK_SIZE: usize = 16;
const NOISE_BUFFER_SIZE: usize = NOISE_BLOCK_SIZE * NOISE_BLOCK_SIZE * 3;

const SIZE_OF_RAY: usize = 44;
const SIZE_OF_INTERSECTION: usize = 16;

#[derive(Copy, Clone, Debug)]
struct Triangle
{
    material_index: u32
}

#[derive(Copy, Clone, Debug)]
struct Material
{
    diffuse: [f32; 3]
}

#[derive(Copy, Clone, Debug)]
struct EmitterTriangle
{
    primitive_index: u32,
    emissive: [f32; 3],
    area: f32
}

#[derive(Copy, Clone, Debug)]
struct ApplicationData
{
    ray_number: u32,
    emitter_triangles_count: u32,
    emitter_total_area: f32
}

pub struct RayTracer {
    acceleration_structure: TriangleAccelerationStructure,
    ray_intersector: RayIntersector,

    ray_buffer: Option<Buffer>,
    intersection_buffer: Option<Buffer>,
    triangle_buffer: Buffer,
    material_buffer: Buffer,
    noise_buffer: Buffer,
    app_buffer: Buffer,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    emitter_triangle_buffer: Buffer,

    output_image: Option<Texture>,
    output_image_size: (usize, usize, usize),
    no_emitter_triangles: usize,
    total_light_area: f32,

    test_pipeline_state: ComputePipelineState,
    accumulator_pipeline_state: ComputePipelineState,
    ray_generator_pipeline_state: ComputePipelineState,
    intersection_handler_pipeline_state: ComputePipelineState,
    shadow_handler_pipeline_state: ComputePipelineState,

    rng: MT19937
}

impl RayTracer {

    pub fn new(device: &DeviceRef, width: usize, height: usize) -> RayTracer
    {
        let (models, materials) = tobj::load_obj(&std::path::PathBuf::from("../../Data/3D models/cornellbox/cornellbox.obj")).unwrap();

        let mut vertex_data = Vec::new();
        let mut index_data = Vec::new();
        let mut triangle_data = Vec::new();
        let mut emitter_triangle_data = Vec::new();
        let mut total_light_area = 0.0f32;
        for model in models {
            println!("{:?}", model);
            let index = (vertex_data.len() / 3) as u32;
            vertex_data.append(&mut model.mesh.positions.clone());
            triangle_data.append(&mut vec![Triangle {material_index: model.mesh.material_id.unwrap() as u32}; model.mesh.indices.len()/3]);

            if let Some(emissive_string) = materials[model.mesh.material_id.unwrap()].unknown_param.get("Ke") {
                for model_primitive_index in 0..model.mesh.indices.len()/3 {
                    let mut i = 3 * model.mesh.indices[model_primitive_index*3] as usize;
                    let p0 = cgmath::Vector3::new(model.mesh.positions[i], model.mesh.positions[i + 1],model.mesh.positions[i + 2]);
                    i = 3 * model.mesh.indices[model_primitive_index*3 + 1] as usize;
                    let p1 = cgmath::Vector3::new(model.mesh.positions[i], model.mesh.positions[i + 1],model.mesh.positions[i + 2]);
                    i = 3 * model.mesh.indices[model_primitive_index*3 + 2] as usize;
                    let p2 = cgmath::Vector3::new(model.mesh.positions[i], model.mesh.positions[i + 1],model.mesh.positions[i + 2]);

                    let area = 0.5 * (p1 - p0).cross(p2 - p0).magnitude();
                    total_light_area += area;
                    let emissive = parse_float3(emissive_string);

                    emitter_triangle_data.push( EmitterTriangle {primitive_index: (index_data.len()/3 + model_primitive_index) as u32, emissive, area} );
                }
            }

            for i in model.mesh.indices.iter() {
                index_data.push(index + i);
            }
        }

        let mut material_data = Vec::new();
        for material in materials {
            println!("{:?}", material);
            material_data.push(Material { diffuse: material.diffuse });
        }

        // Build acceleration structure:
        let vertex_buffer = device.new_buffer_with_data( unsafe { mem::transmute(vertex_data.as_ptr()) },
                                     (vertex_data.len() * mem::size_of::<f32>()) as u64,
                                     MTLResourceOptions::CPUCacheModeDefaultCache);
        let index_buffer = device.new_buffer_with_data( unsafe { mem::transmute(index_data.as_ptr()) },
                                     (index_data.len() * mem::size_of::<u32>()) as u64,
                                     MTLResourceOptions::CPUCacheModeDefaultCache);
        let triangle_buffer = device.new_buffer_with_data( unsafe { mem::transmute(triangle_data.as_ptr()) },
                                     (triangle_data.len() * mem::size_of::<Triangle>()) as u64,
                                     MTLResourceOptions::CPUCacheModeDefaultCache);
        let material_buffer = device.new_buffer_with_data( unsafe { mem::transmute(material_data.as_ptr()) },
                                     (material_data.len() * mem::size_of::<Material>()) as u64,
                                     MTLResourceOptions::CPUCacheModeDefaultCache);
        let emitter_triangle_buffer = device.new_buffer_with_data( unsafe { mem::transmute(emitter_triangle_data.as_ptr()) },
                                     (emitter_triangle_data.len() * mem::size_of::<EmitterTriangle>()) as u64,
                                     MTLResourceOptions::CPUCacheModeDefaultCache);
        let noise_buffer = device.new_buffer((NOISE_BUFFER_SIZE * mem::size_of::<f32>()) as u64,
                                             MTLResourceOptions::CPUCacheModeDefaultCache);
        let app_buffer = device.new_buffer(mem::size_of::<ApplicationData>() as u64, MTLResourceOptions::CPUCacheModeDefaultCache);

        let acceleration_structure = TriangleAccelerationStructure::new(&device);
        acceleration_structure.set_vertex_buffer(Some(&vertex_buffer));
        acceleration_structure.set_vertex_stride((3 * mem::size_of::<f32>()) as i64);
        acceleration_structure.set_index_buffer(Some(&index_buffer));
        acceleration_structure.set_index_type(MPSDataType::uInt32);
        acceleration_structure.set_triangle_count((index_data.len() / 3) as i64);
        acceleration_structure.rebuild();

        // Setup ray intersector:
        let ray_intersector = RayIntersector::new(&device);
        ray_intersector.set_ray_stride(SIZE_OF_RAY as u64);
        ray_intersector.set_ray_data_type(MPSRayDataType::originMinDistanceDirectionMaxDistance);
        ray_intersector.set_intersection_stride(SIZE_OF_INTERSECTION as u64);
        ray_intersector.set_intersection_data_type(MPSIntersectionDataType::distancePrimitiveIndexCoordinates);

        // Pipeline states:
        let test_pipeline_state = Self::create_compute_pipeline_state(device, "src/test.metal", "imageFillTest");
        let ray_generator_pipeline_state = Self::create_compute_pipeline_state(device, "src/tracing.metal", "generateRays");
        let intersection_handler_pipeline_state = Self::create_compute_pipeline_state(device, "src/tracing.metal", "handleIntersections");
        let shadow_handler_pipeline_state = Self::create_compute_pipeline_state(device, "src/tracing.metal", "handleShadows");
        let accumulator_pipeline_state = Self::create_compute_pipeline_state(device, "src/tracing.metal", "accumulateImage");

        let mut val = RayTracer {acceleration_structure, ray_intersector, vertex_buffer, index_buffer, triangle_buffer, emitter_triangle_buffer, material_buffer, noise_buffer, app_buffer, ray_buffer: None, intersection_buffer: None,
            no_emitter_triangles: emitter_triangle_data.len(), total_light_area, output_image: None, output_image_size: (0,0,0), test_pipeline_state, ray_generator_pipeline_state, intersection_handler_pipeline_state, shadow_handler_pipeline_state, accumulator_pipeline_state,
            rng: MT19937::new_unseeded()};
        val.resize(device, width, height);
        val
    }

    fn update_noise_buffer(&mut self)
    {
        let mut data = [0.0f32; NOISE_BUFFER_SIZE];
        for i in 0..NOISE_BUFFER_SIZE {
            data[i] = self.rng.next_f32();
        }

        unsafe {
            let ptr = self.noise_buffer.contents() as *mut [f32; NOISE_BUFFER_SIZE];
            *ptr = mem::transmute(data);
        }
    }

    fn create_compute_pipeline_state(device: &DeviceRef, file_path: &str, function_name: &str) -> ComputePipelineState
    {
        let mut file = File::open(file_path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        let options = CompileOptions::new();
        let library = device.new_library_with_source(&contents, &options).unwrap();
        let compute_function = library.get_function(function_name, None).unwrap();

        let pipeline_state_descriptor = ComputePipelineDescriptor::new();
        pipeline_state_descriptor.set_compute_function(Some(&compute_function));

        device.new_compute_pipeline_state_with_function(&pipeline_state_descriptor.compute_function().unwrap()).unwrap()

    }

    pub fn resize(&mut self, device: &DeviceRef, width: usize, height: usize)
    {
        self.output_image_size = (width, height, 1);
        let ray_count = width * height;

        let texture_descriptor = TextureDescriptor::new();
        texture_descriptor.set_pixel_format(MTLPixelFormat::RGBA32Float);
        texture_descriptor.set_width(width as u64);
        texture_descriptor.set_height(height as u64);
        texture_descriptor.set_storage_mode(MTLStorageMode::Private);
        texture_descriptor.set_usage(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite);
        self.output_image = Some(device.new_texture(&texture_descriptor));

        self.ray_buffer = Some(device.new_buffer((ray_count * SIZE_OF_RAY) as u64, MTLResourceOptions::StorageModePrivate));
        self.intersection_buffer = Some(device.new_buffer((ray_count * SIZE_OF_INTERSECTION) as u64, MTLResourceOptions::StorageModePrivate));

    }

    pub fn encode_into(&mut self, ray_number: usize, command_buffer: &CommandBufferRef)
    {
        self.update_noise_buffer();

        self.encode_ray_generator(command_buffer, ray_number);

        self.ray_intersector.encode_intersection_to_command_buffer(command_buffer,
                                                                   MPSIntersectionType::nearest,
                                                                   self.ray_buffer.as_ref().unwrap(), 0,
                                                                   self.intersection_buffer.as_ref().unwrap(), 0,
                                                                   (self.output_image_size.0 * self.output_image_size.1) as u64,
                                                                   &self.acceleration_structure);

        self.encode_intersection_handler(command_buffer, ray_number);

        self.ray_intersector.encode_intersection_to_command_buffer(command_buffer,
                                                                   MPSIntersectionType::any,
                                                                   self.ray_buffer.as_ref().unwrap(), 0,
                                                                   self.intersection_buffer.as_ref().unwrap(), 0,
                                                                   (self.output_image_size.0 * self.output_image_size.1) as u64,
                                                                   &self.acceleration_structure);

        self.encode_shadow_handler(command_buffer);

        self.encode_accumulator(command_buffer, ray_number);
    }

    fn encode_ray_generator(&self, command_buffer: &CommandBufferRef, ray_number: usize)
    {
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_buffer(0, Some(self.ray_buffer.as_ref().unwrap()), 0);
        encoder.set_buffer(1, Some(&self.noise_buffer), 0);
        encoder.set_compute_pipeline_state(&self.ray_generator_pipeline_state);
        self.dispatch_thread_groups(&encoder);

        encoder.end_encoding();
    }

    fn encode_intersection_handler(&self, command_buffer: &CommandBufferRef, ray_number: usize)
    {
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_buffer(0, Some(self.intersection_buffer.as_ref().unwrap()), 0);
        encoder.set_buffer(1, Some(&self.material_buffer), 0);
        encoder.set_buffer(2, Some(&self.triangle_buffer), 0);
        encoder.set_buffer(3, Some(self.ray_buffer.as_ref().unwrap()), 0);
        encoder.set_buffer(4, Some(&self.vertex_buffer), 0);
        encoder.set_buffer(5, Some(&self.index_buffer), 0);
        encoder.set_buffer(6, Some(&self.emitter_triangle_buffer), 0);
        encoder.set_buffer(7, Some(&self.app_buffer), 0);
        encoder.set_buffer(8, Some(&self.noise_buffer), 0);
        encoder.set_compute_pipeline_state(&self.intersection_handler_pipeline_state);
        self.dispatch_thread_groups(&encoder);

        encoder.end_encoding();
    }

    fn encode_shadow_handler(&self, command_buffer: &CommandBufferRef)
    {
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_buffer(0, Some(self.ray_buffer.as_ref().unwrap()), 0);
        encoder.set_buffer(1, Some(self.intersection_buffer.as_ref().unwrap()), 0);
        encoder.set_compute_pipeline_state(&self.shadow_handler_pipeline_state);
        self.dispatch_thread_groups(&encoder);

        encoder.end_encoding();
    }

    fn encode_accumulator(&self, command_buffer: &CommandBufferRef, ray_number: usize)
    {
        unsafe {
            let ptr = self.app_buffer.contents() as *mut ApplicationData;
            *ptr = ApplicationData {ray_number: ray_number as u32, emitter_triangles_count: self.no_emitter_triangles as u32, emitter_total_area: self.total_light_area};
        }

        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_texture(0, Some(self.output_image.as_ref().unwrap()));
        encoder.set_buffer(0, Some(self.ray_buffer.as_ref().unwrap()), 0);
        encoder.set_buffer(1, Some(&self.app_buffer), 0);
        encoder.set_compute_pipeline_state(&self.accumulator_pipeline_state);
        self.dispatch_thread_groups(&encoder);

        encoder.end_encoding();
    }

    pub fn encode_into_test(&self, command_buffer: &CommandBufferRef)
    {
        let encoder = command_buffer.new_compute_command_encoder();

        encoder.set_texture(0, Some(self.output_image.as_ref().unwrap()));
        encoder.set_compute_pipeline_state(&self.test_pipeline_state);
        self.dispatch_thread_groups(&encoder);

        encoder.end_encoding();
    }

    fn dispatch_thread_groups(&self, encoder: &ComputeCommandEncoderRef)
    {
        let threads_per_thread_group = MTLSize {width: 8, height: 8, depth: 1};
        let thread_groups_count = MTLSize {width: self.output_image_size.0 as u64 / threads_per_thread_group.width,
            height: self.output_image_size.1 as u64 / threads_per_thread_group.height,
            depth: self.output_image_size.2 as u64 / threads_per_thread_group.depth};
        encoder.dispatch_thread_groups(thread_groups_count, threads_per_thread_group);
    }

    pub fn output_texture(&self) -> &TextureRef
    {
        self.output_image.as_ref().unwrap()
    }

}

fn parse_float3(val_str: &str) -> [f32; 3] {
    let mut words = val_str[..].split_whitespace();
    let mut vals = [0.0f32; 3];
    for (i, p) in words.enumerate() {
        vals[i] = std::str::FromStr::from_str(p).unwrap();
    }
    vals
}