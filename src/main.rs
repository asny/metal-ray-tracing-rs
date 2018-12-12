
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

mod raytracer;

fn create_blit_pipeline_state(device: &DeviceRef) -> RenderPipelineState
{
    let mut file = File::open("src/blit.metal").unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    let options = CompileOptions::new();
    let library = device.new_library_with_source(&contents, &options).unwrap();
    let vert = library.get_function("blitVertex", None).unwrap();
    let frag = library.get_function("blitFragment", None).unwrap();

    let pipeline_state_descriptor = RenderPipelineDescriptor::new();
    pipeline_state_descriptor.set_vertex_function(Some(&vert));
    pipeline_state_descriptor.set_fragment_function(Some(&frag));
    pipeline_state_descriptor.color_attachments().object_at(0).unwrap().set_pixel_format(MTLPixelFormat::BGRA8Unorm);

    device.new_render_pipeline_state(&pipeline_state_descriptor).unwrap()
}

fn encode_blit_into(command_buffer: &CommandBufferRef, blit_pipeline_state: &RenderPipelineStateRef, input_texture: &TextureRef, output_texture: &TextureRef)
{
    let descriptor = RenderPassDescriptor::new();
    let color_attachment = descriptor.color_attachments().object_at(0).unwrap();

    color_attachment.set_texture(Some(output_texture));
    color_attachment.set_load_action(MTLLoadAction::Clear);
    color_attachment.set_clear_color(MTLClearColor::new(0.5, 0.2, 0.2, 1.0));
    color_attachment.set_store_action(MTLStoreAction::Store);

    let encoder = command_buffer.new_render_command_encoder(&descriptor);
    encoder.set_render_pipeline_state(blit_pipeline_state);
    encoder.set_fragment_texture(0, Some(input_texture));
    encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
    encoder.end_encoding();
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

    let blit_pipeline_state = create_blit_pipeline_state(&device);
    let command_queue = device.new_command_queue();

    let raytracer = raytracer::RayTracer::new(&device, draw_size.width as usize, draw_size.height as usize);

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

            let command_buffer = command_queue.new_command_buffer();
            raytracer.encode_into(command_buffer);
            encode_blit_into(&command_buffer, &blit_pipeline_state, raytracer.output_texture(), &drawable.texture());

            command_buffer.present_drawable(&drawable);
            command_buffer.commit();

            unsafe {
                msg_send![pool, drain];
                pool = NSAutoreleasePool::new(cocoa::base::nil);
            }
        }
    }
}
