use std::cell::{Cell, RefCell};
use std::rc::Rc;

use servo::WebView;

use super::rendering_context::ServoRenderingAdapter;

pub struct SlintServoAdapter {
    inner: RefCell<SlintServoAdapterInner>,
    active: Cell<bool>,
    set_web_content: Box<dyn Fn(slint::Image)>,
    set_current_url: Box<dyn Fn(slint::SharedString)>,
    get_webview_size: Box<dyn Fn() -> (u32, u32)>,
}

struct SlintServoAdapterInner {
    webview: Option<WebView>,
    rendering_adapter: Option<Rc<Box<dyn ServoRenderingAdapter>>>,
    #[allow(dead_code)]
    device: slint::wgpu_29::wgpu::Device,
    #[allow(dead_code)]
    queue: slint::wgpu_29::wgpu::Queue,
}

impl SlintServoAdapter {
    pub fn new(
        device: slint::wgpu_29::wgpu::Device,
        queue: slint::wgpu_29::wgpu::Queue,
        set_web_content: Box<dyn Fn(slint::Image)>,
        set_current_url: Box<dyn Fn(slint::SharedString)>,
        get_webview_size: Box<dyn Fn() -> (u32, u32)>,
    ) -> Self {
        Self {
            inner: RefCell::new(SlintServoAdapterInner {
                webview: None,
                rendering_adapter: None,
                device,
                queue,
            }),
            active: Cell::new(true),
            set_web_content,
            set_current_url,
            get_webview_size,
        }
    }

    pub fn webview(&self) -> WebView {
        self.inner
            .borrow()
            .webview
            .as_ref()
            .expect("Webview not initialized yet")
            .clone()
    }

    pub fn set_inner(
        &self,
        webview: WebView,
        rendering_adapter: Rc<Box<dyn ServoRenderingAdapter>>,
    ) {
        let mut inner = self.inner.borrow_mut();
        inner.webview = Some(webview);
        inner.rendering_adapter = Some(rendering_adapter);
    }

    pub fn set_active(&self, active: bool) {
        self.active.set(active);
    }

    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    pub fn present(&self) {
        let inner = self.inner.borrow();
        if let Some(rendering_adapter) = &inner.rendering_adapter {
            rendering_adapter.present();
        }
    }

    pub fn update_web_content_with_latest_frame(&self) {
        let inner = self.inner.borrow();
        let rendering_adapter = inner
            .rendering_adapter
            .as_ref()
            .expect("Rendering adapter not initialized");

        let slint_image = rendering_adapter.current_framebuffer_as_image();
        (self.set_web_content)(slint_image);
    }

    pub fn set_current_url(&self, url: slint::SharedString) {
        (self.set_current_url)(url);
    }

    pub fn resize_webview_if_needed(&self) {
        let (width, height) = (self.get_webview_size)();

        let inner = self.inner.borrow();
        if let Some(ref webview) = inner.webview {
            use winit::dpi::PhysicalSize;
            let current = webview.size();
            if (current.width - width as f32).abs() > 0.5
                || (current.height - height as f32).abs() > 0.5
            {
                webview.resize(PhysicalSize::new(width.max(1), height.max(1)));
            }
        }
    }
}
