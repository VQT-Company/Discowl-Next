mod adapter;
mod delegate;
pub mod events_utils;
mod rendering_context;
mod waker;

pub use adapter::SlintServoAdapter;
pub use delegate::AppDelegate;
pub use rendering_context::ServoRenderingAdapter;
pub use waker::Waker;

use servo::Servo;

pub fn create_webview(
    servo: &Servo,
    initial_url: slint::SharedString,
    device: slint::wgpu_29::wgpu::Device,
    queue: slint::wgpu_29::wgpu::Queue,
    set_web_content: Box<dyn Fn(slint::Image)>,
    set_current_url: Box<dyn Fn(slint::SharedString)>,
    get_webview_size: Box<dyn Fn() -> (u32, u32)>,
    scale_factor: f32,
) -> std::rc::Rc<SlintServoAdapter> {
    use std::rc::Rc;

    use euclid::Scale;
    use servo::WebViewBuilder;
    use url::Url;
    use winit::dpi::PhysicalSize;

    let (width, height) = (get_webview_size)();
    let physical_size = PhysicalSize::new(width.max(1), height.max(1));

    let rendering_adapter = rendering_context::create_gpu_adapter(physical_size, device.clone(), queue.clone())
        .unwrap_or_else(|e| {
            eprintln!("[webview] *** GPU adapter failed: {e}");
            eprintln!("[webview] *** Falling back to software rendering (slower, more CPU usage)");
            eprintln!("[webview] *** Build with --release and check the grafting crate for GPU support");
            rendering_context::create_software_context(physical_size)
        });
    let rendering_adapter_rc = Rc::new(rendering_adapter);

    let url = Url::parse(&initial_url).expect("Invalid URL");

    let adapter = Rc::new(SlintServoAdapter::new(
        device.clone(),
        queue.clone(),
        set_web_content,
        set_current_url,
        get_webview_size,
    ));

    let delegate = Rc::new(AppDelegate::new(adapter.clone()));

    let webview = WebViewBuilder::new(servo, rendering_adapter_rc.get_rendering_context())
        .url(url)
        .delegate(delegate)
        .build();

    webview.show();

    let scale = Scale::new(scale_factor);
    webview.set_hidpi_scale_factor(scale);

    adapter.set_inner(webview, rendering_adapter_rc.clone());

    adapter
}
