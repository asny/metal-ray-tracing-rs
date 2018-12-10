
use objc::{msg_send, sel, sel_impl};
use cocoa::base::id as cocoa_id;
use cocoa::base::YES;
use cocoa::foundation::{NSAutoreleasePool};
use cocoa::appkit::{NSWindow, NSView};

use metal::*;

use winit::os::macos::WindowExt;

use std::fs::File;
use std::io::prelude::*;
use std::mem;

fn prepare_render_pipeline_descriptor(device: &DeviceRef) -> RenderPipelineState
{
    let mut file = File::open("src/shaders.metal").unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    let options = CompileOptions::new();
    let library = device.new_library_with_source(&contents, &options).unwrap();
    let vert = library.get_function("vs", None).unwrap();
    let frag = library.get_function("ps", None).unwrap();

    let pipeline_state_descriptor = RenderPipelineDescriptor::new();
    pipeline_state_descriptor.set_vertex_function(Some(&vert));
    pipeline_state_descriptor.set_fragment_function(Some(&frag));
    pipeline_state_descriptor.color_attachments().object_at(0).unwrap().set_pixel_format(MTLPixelFormat::BGRA8Unorm);

    device.new_render_pipeline_state(&pipeline_state_descriptor).unwrap()
}

fn prepare_render_pass_descriptor<'a>(texture: &TextureRef) -> &'a RenderPassDescriptorRef
{
    let descriptor = RenderPassDescriptor::new();
    let color_attachment = descriptor.color_attachments().object_at(0).unwrap();

    color_attachment.set_texture(Some(texture));
    color_attachment.set_load_action(MTLLoadAction::Clear);
    color_attachment.set_clear_color(MTLClearColor::new(0.5, 0.2, 0.2, 1.0));
    color_attachment.set_store_action(MTLStoreAction::Store);
    descriptor
}

fn main() {
    let mut events_loop = winit::EventsLoop::new();
    let winit_window = winit::WindowBuilder::new()
        .with_dimensions((800, 600).into())
        .with_title("Metal ray tracer".to_string())
        .build(&events_loop).unwrap();

    let window: cocoa_id = unsafe { mem::transmute(winit_window.get_nswindow()) };
    let device = Device::system_default();

    let layer = CoreAnimationLayer::new();
    layer.set_device(&device);
    layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    layer.set_presents_with_transaction(false);

    unsafe {
        let view = window.contentView();
        view.setWantsBestResolutionOpenGLSurface_(YES);
        view.setWantsLayer(YES);
        view.setLayer(mem::transmute(layer.as_ref()));
    }

    let draw_size = winit_window.get_inner_size().unwrap();
    layer.set_drawable_size(draw_size.width as f64, draw_size.height as f64);

    let pipeline_state = prepare_render_pipeline_descriptor(&device);
    let command_queue = device.new_command_queue();

    // Acceleration structure:
    let vertex_data = [
        0.25f32, 0.25, 0.0,
        0.75, 0.25, 0.0,
        0.50, 0.75, 0.0
    ];
    let index_data = [
        0u32, 1, 2
    ];

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

    // Ray intersector:
    let ray_intersector = RayIntersector::new(&device);
    ray_intersector.set_ray_stride(800*600);
    ray_intersector.set_ray_data_type(1); // MPSRayDataTypeOriginMinDistanceDirectionMaxDistance
    ray_intersector.set_intersection_stride(800*600);
    ray_intersector.set_intersection_data_type(4); // MPSIntersectionDataTypeDistancePrimitiveIndexCoordinates

    let mut pool = unsafe { NSAutoreleasePool::new(cocoa::base::nil) };
    let mut running = true;

    while running {
        events_loop.poll_events(|event| {
            match event {
                winit::Event::WindowEvent{ event: winit::WindowEvent::CloseRequested, .. } => running = false,
                _ => ()
            }
        });

        if let Some(drawable) = layer.next_drawable() {
            let render_pass_descriptor = prepare_render_pass_descriptor(drawable.texture());

            let command_buffer = command_queue.new_command_buffer();
            let parallel_encoder = command_buffer.new_parallel_render_command_encoder(&render_pass_descriptor);
            let encoder = parallel_encoder.render_command_encoder();
            encoder.set_render_pipeline_state(&pipeline_state);
            encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
            encoder.end_encoding();
            parallel_encoder.end_encoding();

            render_pass_descriptor.color_attachments().object_at(0).unwrap().set_load_action(MTLLoadAction::DontCare);

            command_buffer.present_drawable(&drawable);
            command_buffer.commit();

            unsafe {
                msg_send![pool, drain];
                pool = NSAutoreleasePool::new(cocoa::base::nil);
            }
        }
    }
}
