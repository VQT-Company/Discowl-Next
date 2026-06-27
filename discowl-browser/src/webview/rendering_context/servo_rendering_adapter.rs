use std::cell::{Cell, RefCell};
use std::rc::Rc;

use euclid::default::Size2D;
use gleam::gl;
use slint::{Image, SharedPixelBuffer};
use winit::dpi::PhysicalSize;

use servo::{DeviceIntRect, RenderingContext, SoftwareRenderingContext};
use surfman::{Connection, Surface, SurfaceTexture, SurfaceType};

use grafting::surfman_gl::{SurfmanFrameContext, select_adapter_matching_surfman_luid};
use surfman::chains::{PreserveBuffer, SwapChain};

pub trait ServoRenderingAdapter {
    fn current_framebuffer_as_image(&self) -> Image;
    /// Monotonically increasing version bumped each time a new, unique frame
    /// was actually read back.  Callers can compare successive values to
    /// detect whether the frame content changed since last call.
    fn frame_version(&self) -> u64;
    fn present(&self);
    fn get_rendering_context(&self) -> Rc<dyn RenderingContext>;
}

pub fn create_software_context(
    size: PhysicalSize<u32>,
) -> Box<dyn ServoRenderingAdapter> {
    let rendering_context = Rc::new(
        SoftwareRenderingContext::new(size)
            .expect("Failed to create software rendering context"),
    );
    Box::new(SoftwareAdapter {
        rendering_context,
        frame_version: Cell::new(0),
    })
}

struct SoftwareAdapter {
    rendering_context: Rc<SoftwareRenderingContext>,
    frame_version: Cell<u64>,
}

impl ServoRenderingAdapter for SoftwareAdapter {
    fn current_framebuffer_as_image(&self) -> Image {
        let size = self.rendering_context.size2d().to_i32();
        let viewport_rect = DeviceIntRect::from_origin_and_size(euclid::Point2D::origin(), size);

        let image_buffer = self
            .rendering_context
            .read_to_image(viewport_rect)
            .expect("Failed to get image buffer from frame buffer");

        let (width, height) = image_buffer.dimensions();
        let pixel_slice = image_buffer.into_raw();

        let shared_pixel_buffer =
            SharedPixelBuffer::clone_from_slice(&pixel_slice, width, height);

        self.frame_version.set(self.frame_version.get() + 1);

        Image::from_rgba8(shared_pixel_buffer)
    }

    fn frame_version(&self) -> u64 {
        self.frame_version.get()
    }

    fn present(&self) {}

    fn get_rendering_context(&self) -> Rc<dyn RenderingContext> {
        self.rendering_context.clone()
    }
}

pub fn create_gpu_adapter(
    physical_size: PhysicalSize<u32>,
    device: slint::wgpu_29::wgpu::Device,
    _queue: slint::wgpu_29::wgpu::Queue,
) -> Result<Box<dyn ServoRenderingAdapter>, String> {
    let connection =
        Connection::new().map_err(|e| format!("Connection::new failed: {e:?}"))?;

    let adapter = select_adapter_matching_surfman_luid(&connection, &device)
        .or_else(|_| connection.create_adapter().map_err(|e| format!("create_adapter failed: {e:?}")))
        .map_err(|e| format!("No suitable surfman adapter: {e}"))?;

    let frame_context = Rc::new(
        SurfmanFrameContext::new(&connection, &adapter)
            .map_err(|e| format!("SurfmanFrameContext::new failed: {e:?}"))?,
    );

    let surfman_size = Size2D::new(physical_size.width as i32, physical_size.height as i32);
    let surface = frame_context
        .create_surface(SurfaceType::Generic { size: surfman_size })
        .map_err(|e| format!("create_surface failed: {e:?}"))?;

    frame_context
        .bind_surface(surface)
        .map_err(|e| format!("bind_surface failed: {e:?}"))?;

    frame_context
        .make_current()
        .map_err(|e| format!("make_current failed: {e:?}"))?;

    let swap_chain = frame_context
        .create_attached_swap_chain()
        .map_err(|e| format!("create_attached_swap_chain failed: {e:?}"))?;

    Ok(Box::new(GpuAdapter {
        rendering_context: Rc::new(GpuRenderingContext {
            frame_context,
            swap_chain,
            size: RefCell::new(physical_size),
        }),
        frame_version: Cell::new(0),
    }))
}

struct GpuRenderingContext {
    frame_context: Rc<SurfmanFrameContext>,
    swap_chain: SwapChain<surfman::Device>,
    size: RefCell<PhysicalSize<u32>>,
}

impl Drop for GpuRenderingContext {
    fn drop(&mut self) {
        let device = &mut self.frame_context.device.borrow_mut();
        let context = &mut self.frame_context.context.borrow_mut();
        let _ = self.swap_chain.destroy(device, context);
    }
}

impl RenderingContext for GpuRenderingContext {
    fn prepare_for_rendering(&self) {
        self.frame_context.prepare_for_rendering();
    }

    fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<image::RgbaImage> {
        self.frame_context.read_to_image_region(
            source_rectangle.min.x,
            source_rectangle.min.y,
            source_rectangle.width(),
            source_rectangle.height(),
        )
    }

    fn size(&self) -> PhysicalSize<u32> {
        *self.size.borrow()
    }

    fn resize(&self, size: PhysicalSize<u32>) {
        if *self.size.borrow() == size {
            return;
        }
        *self.size.borrow_mut() = size;
        let mut device = self.frame_context.device.borrow_mut();
        let mut context = self.frame_context.context.borrow_mut();
        let surfman_size = Size2D::new(size.width as i32, size.height as i32);
        let _ = self.swap_chain.resize(&mut *device, &mut *context, surfman_size);
    }

    fn present(&self) {
        let mut device = self.frame_context.device.borrow_mut();
        let mut context = self.frame_context.context.borrow_mut();
        let _ = self.swap_chain.swap_buffers(
            &mut *device,
            &mut *context,
            PreserveBuffer::No,
        );
    }

    fn make_current(&self) -> Result<(), surfman::Error> {
        self.frame_context.make_current()
    }

    fn gleam_gl_api(&self) -> Rc<dyn gleam::gl::Gl> {
        self.frame_context.gleam_gl.clone()
    }

    fn glow_gl_api(&self) -> std::sync::Arc<glow::Context> {
        self.frame_context.glow_gl.clone()
    }

    fn create_texture(&self, surface: Surface) -> Option<(SurfaceTexture, u32, Size2D<i32>)> {
        self.frame_context.create_texture(surface)
    }

    fn destroy_texture(&self, surface_texture: SurfaceTexture) -> Option<Surface> {
        self.frame_context.destroy_texture(surface_texture)
    }

    fn connection(&self) -> Option<Connection> {
        self.frame_context.connection()
    }
}

struct GpuAdapter {
    rendering_context: Rc<GpuRenderingContext>,
    frame_version: Cell<u64>,
}

impl GpuRenderingContext {
    fn read_framebuffer_direct(&self) -> Option<image::RgbaImage> {
        let size = self.size();
        let w = size.width as i32;
        let h = size.height as i32;
        if w <= 0 || h <= 0 {
            return None;
        }
        let gleam = &self.frame_context.gleam_gl;
        let mut pixels = gleam.read_pixels(0, 0, w, h, gl::RGBA, gl::UNSIGNED_BYTE);
        if gleam.get_error() != gl::NO_ERROR {
            return None;
        }
        // Y-flip in-place (swap rows, avoid clone)
        let stride = (w as usize) * 4;
        let hu = h as usize;
        let half = hu / 2;
        for row in 0..half {
            let a = row * stride;
            let b = (hu - 1 - row) * stride;
            // split_at_mut(b) gives [0..b) and [b..)
            let (prefix, suffix) = pixels.split_at_mut(b);
            let top = &mut prefix[a..a + stride];
            let bot = &mut suffix[..stride];
            for i in 0..stride {
                std::mem::swap(&mut top[i], &mut bot[i]);
            }
        }
        image::RgbaImage::from_raw(w as u32, h as u32, pixels)
    }
}

impl ServoRenderingAdapter for GpuAdapter {
    fn current_framebuffer_as_image(&self) -> Image {
        let _ = self.rendering_context.make_current();

        match self.rendering_context.read_framebuffer_direct() {
            Some(image_buffer) => {
                let (width, height) = image_buffer.dimensions();
                let pixel_slice = image_buffer.into_raw();
                let shared_pixel_buffer =
                    SharedPixelBuffer::clone_from_slice(&pixel_slice, width, height);
                self.frame_version.set(self.frame_version.get() + 1);
                Image::from_rgba8(shared_pixel_buffer)
            }
            None => {
                eprintln!("[gpu] direct readback failed");
                Image::default()
            }
        }
    }

    fn frame_version(&self) -> u64 {
        self.frame_version.get()
    }

    fn present(&self) {
        self.rendering_context.present();
    }

    fn get_rendering_context(&self) -> Rc<dyn RenderingContext> {
        self.rendering_context.clone()
    }
}
