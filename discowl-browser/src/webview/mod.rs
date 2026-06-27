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
    use url::Url;
    use euclid::Scale;
    use winit::dpi::PhysicalSize;
    use servo::{ServoBuilder, WebViewBuilder};

    let adapter = Rc::new(SlintServoAdapter::new(device.clone(), queue.clone()));

    let scale_factor = app.window().scale_factor();
    let width = (app.get_webview_width() as f32 * scale_factor) as u32;
    let height = (app.get_webview_height() as f32 * scale_factor) as u32;
    let physical_size = PhysicalSize::new(width.max(1), height.max(1));

    let rendering_adapter = rendering_context::create_gpu_adapter(physical_size, device, queue)
        .unwrap_or_else(|e| {
            eprintln!("[webview] *** GPU adapter failed: {e}");
            eprintln!("[webview] *** Falling back to software rendering (slower, more CPU usage)");
            eprintln!("[webview] *** Build with --release and check the grafting crate for GPU support");
            rendering_context::create_software_context(physical_size)
        });
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
    let app_weak = slint::ComponentHandle::as_weak(&app);
    slint::spawn_local(async move {
        use std::time::{Duration, Instant};

        // Throttle event-loop spinning to avoid wasted CPU.
        // The AppDelegate caps frame display at 200 ms (5 FPS), so there is
        // no benefit in spinning Servo's event loop faster than ~6 Hz.
        // Spinning slower also keeps input latency acceptable (~150 ms).
        let spin_interval = Duration::from_millis(150);
        let mut last_spin = Instant::now();

        loop {
            let state = match state_weak.upgrade() {
                Some(s) => s,
                None => break,
            };

            // Block until the first waker signal arrives.
            let _ = state.waker_receiver().recv().await;

            // Drain any additional pending wakeups that arrived during the
            // previous spin — batch them into a single event-loop tick.
            while state.waker_receiver().try_recv().is_ok() {}

            // Only spin the Servo event loop once per interval.
            if last_spin.elapsed() >= spin_interval {
                last_spin = Instant::now();
                state.servo().spin_event_loop();
                if let Some(app) = app_weak.upgrade() {
                    state.resize_webview_if_needed(&app);
                }
            }
        }
    })
    .expect("Failed to spawn servo event loop");

    adapter
}
