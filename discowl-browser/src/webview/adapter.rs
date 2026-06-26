use std::cell::{Ref, RefCell};
use std::rc::Rc;

use slint::ComponentHandle;
use smol::channel::{Receiver, Sender};

use servo::{Servo, WebView};

use super::rendering_context::ServoRenderingAdapter;

pub struct SlintServoAdapter {
    waker_sender: Sender<()>,
    waker_receiver: Receiver<()>,
    inner: RefCell<SlintServoAdapterInner>,
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
        waker_sender: Sender<()>,
        waker_receiver: Receiver<()>,
        device: slint::wgpu_29::wgpu::Device,
        queue: slint::wgpu_29::wgpu::Queue,
    ) -> Self {
        Self {
            waker_sender,
            waker_receiver,
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

    pub fn update_web_content_with_latest_frame(&self, app: &crate::DiscowlWindow) {
        let inner = self.inner.borrow();
        let rendering_adapter = inner
            .rendering_adapter
            .as_ref()
            .expect("Rendering adapter not initialized");

        let slint_image = rendering_adapter.current_framebuffer_as_image();
        app.set_web_content(slint_image);
        app.window().request_redraw();
    }
}
