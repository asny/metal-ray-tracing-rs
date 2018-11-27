
extern crate metal;
extern crate cocoa;

#[macro_use] extern crate objc;

extern crate winit;

use cocoa::base::id as cocoa_id;
use cocoa::base::YES;
use cocoa::foundation::{NSAutoreleasePool};
use cocoa::appkit::{NSWindow, NSView};

use metal::*;

use winit::os::macos::WindowExt;

use std::mem;

fn prepare_pipeline_state<'a>(device: &DeviceRef, library: &LibraryRef) -> RenderPipelineState
{
    let vert = library.get_function("triangle_vertex", None).unwrap();
    let frag = library.get_function("triangle_fragment", None).unwrap();

    let pipeline_state_descriptor = RenderPipelineDescriptor::new();
    pipeline_state_descriptor.set_vertex_function(Some(&vert));
    pipeline_state_descriptor.set_fragment_function(Some(&frag));
    pipeline_state_descriptor.color_attachments().object_at(0).unwrap().set_pixel_format(MTLPixelFormat::BGRA8Unorm);

    device.new_render_pipeline_state(&pipeline_state_descriptor).unwrap()
}

fn prepare_render_pass_descriptor(descriptor: &RenderPassDescriptorRef, texture: &TextureRef)
{
    let color_attachment = descriptor.color_attachments().object_at(0).unwrap();

    color_attachment.set_texture(Some(texture));
    color_attachment.set_load_action(MTLLoadAction::Clear);
    color_attachment.set_clear_color(MTLClearColor::new(0.5, 0.2, 0.2, 1.0));
    color_attachment.set_store_action(MTLStoreAction::Store);
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

    let library = device.new_library_with_file("src/default.metallib").unwrap();
    let pipeline_state = prepare_pipeline_state(&device, &library);
    let command_queue = device.new_command_queue();

    let vbuf = {
        let vertex_data = [
              0.0f32,  0.5, 1.0, 0.0, 0.0,
             -0.5, -0.5, 0.0, 1.0, 0.0,
              0.5,  0.5, 0.0, 0.0, 1.0,
        ];

        device.new_buffer_with_data(
            unsafe { mem::transmute(vertex_data.as_ptr()) },
            (vertex_data.len() * mem::size_of::<f32>()) as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache)
    };

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
            let render_pass_descriptor = RenderPassDescriptor::new();
            prepare_render_pass_descriptor(&render_pass_descriptor, drawable.texture());

            let command_buffer = command_queue.new_command_buffer();
            let parallel_encoder = command_buffer.new_parallel_render_command_encoder(&render_pass_descriptor);
            let encoder = parallel_encoder.render_command_encoder();
            encoder.set_render_pipeline_state(&pipeline_state);
            encoder.set_vertex_buffer(0, Some(&vbuf), 0);
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