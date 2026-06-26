use std::rc::Rc;

use euclid::Point2D;
use slint::{Image, SharedPixelBuffer};
use winit::dpi::PhysicalSize;

use servo::{DeviceIntRect, RenderingContext, SoftwareRenderingContext};

pub trait ServoRenderingAdapter {
    fn current_framebuffer_as_image(&self) -> Image;
    fn get_rendering_context(&self) -> Rc<dyn RenderingContext>;
}

pub fn create_software_context(
    size: PhysicalSize<u32>,
) -> Box<dyn ServoRenderingAdapter> {
    let rendering_context = Rc::new(
        SoftwareRenderingContext::new(size)
            .expect("Failed to create software rendering context"),
    );
    Box::new(SoftwareAdapter { rendering_context })
}

struct SoftwareAdapter {
    rendering_context: Rc<SoftwareRenderingContext>,
}

impl ServoRenderingAdapter for SoftwareAdapter {
    fn current_framebuffer_as_image(&self) -> Image {
        let size = self.rendering_context.size2d().to_i32();
        let viewport_rect = DeviceIntRect::from_origin_and_size(Point2D::origin(), size);

        let image_buffer = self
            .rendering_context
            .read_to_image(viewport_rect)
            .expect("Failed to get image buffer from frame buffer");

        let (width, height) = image_buffer.dimensions();
        let pixel_slice = image_buffer.into_raw();

        let shared_pixel_buffer =
            SharedPixelBuffer::clone_from_slice(&pixel_slice, width, height);

        Image::from_rgba8(shared_pixel_buffer)
    }

    fn get_rendering_context(&self) -> Rc<dyn RenderingContext> {
        self.rendering_context.clone()
    }
}
