mod adapter;
mod delegate;
pub mod events_utils;
mod rendering_context;
mod waker;

pub use adapter::SlintServoAdapter;
pub use delegate::AppDelegate;
pub use rendering_context::ServoRenderingAdapter;
pub use waker::Waker;

use slint::ComponentHandle;

pub fn create_webview(
    app: crate::DiscowlWindow,
    initial_url: slint::SharedString,
    device: slint::wgpu_29::wgpu::Device,
    queue: slint::wgpu_29::wgpu::Queue,
) -> std::rc::Rc<SlintServoAdapter> {
    use std::rc::Rc;
    use smol::channel;
    use url::Url;
    use euclid::Scale;
    use winit::dpi::PhysicalSize;
    use servo::{ServoBuilder, WebViewBuilder};

    let (sender, receiver) = channel::unbounded::<()>();
    let adapter = Rc::new(SlintServoAdapter::new(sender, receiver, device, queue));

    let scale_factor = app.window().scale_factor();
    let width = (app.get_webview_width() as f32 * scale_factor) as u32;
    let height = (app.get_webview_height() as f32 * scale_factor) as u32;
    let physical_size = PhysicalSize::new(width.max(1), height.max(1));

    let rendering_adapter =
        rendering_context::create_software_context(physical_size);
    let rendering_adapter_rc = Rc::new(rendering_adapter);

    let waker = Waker::new(adapter.waker_sender());
    let servo = ServoBuilder::default()
        .event_loop_waker(Box::new(waker))
        .build();

    let url = Url::parse(&initial_url).expect("Invalid URL");
    let delegate = Rc::new(AppDelegate::new(&app, adapter.clone()));

    let webview = WebViewBuilder::new(&servo, rendering_adapter_rc.get_rendering_context())
        .url(url)
        .delegate(delegate)
        .build();

    webview.show();

    let scale = Scale::new(scale_factor as f32);
    webview.set_hidpi_scale_factor(scale);

    adapter.set_inner(servo, webview, rendering_adapter_rc.clone());

    let state_weak = Rc::downgrade(&adapter);
    slint::spawn_local(async move {
        loop {
            let state = match state_weak.upgrade() {
                Some(s) => s,
                None => break,
            };
            let _ = state.waker_receiver().recv().await;
            state.servo().spin_event_loop();
        }
    })
    .expect("Failed to spawn servo event loop");

    adapter
}
