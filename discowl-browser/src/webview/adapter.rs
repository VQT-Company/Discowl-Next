use std::cell::{Cell, Ref, RefCell};
use std::rc::Rc;

use slint::ComponentHandle;
use smol::channel::{Receiver, Sender};

use servo::{Servo, WebView};

use super::rendering_context::ServoRenderingAdapter;

pub struct SlintServoAdapter {
    waker_sender: Sender<()>,
    waker_receiver: Receiver<()>,
    inner: RefCell<SlintServoAdapterInner>,
    last_frame_version: Cell<u64>,
}

struct SlintServoAdapterInner {
    servo: Option<Servo>,
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
    ) -> Self {
        let (sender, receiver) = smol::channel::unbounded();
        Self {
            waker_sender: sender,
            waker_receiver: receiver,
            last_frame_version: Cell::new(0),
            inner: RefCell::new(SlintServoAdapterInner {
                servo: None,
                webview: None,
                rendering_adapter: None,
                device,
                queue,
            }),
        }
    }

    pub fn waker_sender(&self) -> Sender<()> {
        self.waker_sender.clone()
    }

    pub fn waker_receiver(&self) -> Receiver<()> {
        self.waker_receiver.clone()
    }

    pub fn servo(&self) -> Ref<'_, Servo> {
        Ref::map(self.inner.borrow(), |inner| {
            inner.servo.as_ref().expect("Servo not initialized yet")
        })
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
        servo: Servo,
        webview: WebView,
        rendering_adapter: Rc<Box<dyn ServoRenderingAdapter>>,
    ) {
        let mut inner = self.inner.borrow_mut();
        inner.servo = Some(servo);
        inner.webview = Some(webview);
        inner.rendering_adapter = Some(rendering_adapter);
    }

    pub fn present(&self) {
        let inner = self.inner.borrow();
        if let Some(rendering_adapter) = &inner.rendering_adapter {
            rendering_adapter.present();
        }
    }

    pub fn update_web_content_with_latest_frame(&self, app: &crate::DiscowlWindow) {
        let inner = self.inner.borrow();
        let rendering_adapter = inner
            .rendering_adapter
            .as_ref()
            .expect("Rendering adapter not initialized");

        let version = rendering_adapter.frame_version();
        let last = self.last_frame_version.get();
        if version == last {
            return;
        }
        self.last_frame_version.set(version);

        let slint_image = rendering_adapter.current_framebuffer_as_image();
        app.set_web_content(slint_image);
        app.window().request_redraw();
    }

    pub fn resize_webview_if_needed(&self, app: &crate::DiscowlWindow) {
        let scale = app.window().scale_factor();
        let width = (app.get_webview_width() as f32 * scale) as u32;
        let height = (app.get_webview_height() as f32 * scale) as u32;

        let inner = self.inner.borrow();
        if let Some(ref webview) = inner.webview {
            use winit::dpi::PhysicalSize;
            let current = webview.size();
            if (current.width - width as f32).abs() > 0.5 || (current.height - height as f32).abs() > 0.5 {
                webview.resize(PhysicalSize::new(width.max(1), height.max(1)));
            }
        }
    }
}
