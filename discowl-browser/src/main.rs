mod webview;

use std::cell::Cell;
use std::rc::Rc;

use slint::ComponentHandle;

slint::include_modules!();

fn main() {
    setup_slint_renderer();

    let app = DiscowlWindow::new().unwrap();

    let adapter = Rc::new(std::cell::RefCell::new(None::<Rc<webview::SlintServoAdapter>>));
    let adapter_weak = Rc::downgrade(&adapter);

    let initialized = Cell::new(false);
    let app_weak = app.as_weak();

    app.window()
        .set_rendering_notifier(move |state, graphics_api| {
            if initialized.get() {
                return;
            }
            if !matches!(state, slint::RenderingState::RenderingSetup) {
                return;
            }

            let slint::GraphicsAPI::WGPU29 { device, queue, .. } = graphics_api else {
                panic!("Slint did not select a wgpu-29 renderer");
            };

            let app = app_weak.upgrade().unwrap();
            let state = webview::create_webview(
                app,
                "https://google.com".into(),
                device.clone(),
                queue.clone(),
            );

            if let Some(a) = adapter_weak.upgrade() {
                *a.borrow_mut() = Some(state);
            }

            initialized.set(true);
        })
        .unwrap();

    let adapter_weak2 = Rc::downgrade(&adapter);
    app.on_navigate(move |url| {
        println!("Discowl: navigating to {}", url);
        if let Some(a) = adapter_weak2.upgrade() {
            if let Some(ref adapter) = *a.borrow() {
                if let Ok(url) = url::Url::parse(&url) {
                    adapter.webview().load(url);
                } else {
                    let servo_url =
                        webview::events_utils::convert_input_string_to_servo_url(&url);
                    adapter.webview().load(servo_url.into_url());
                }
            }
        }
    });

    let adapter_weak3 = Rc::downgrade(&adapter);
    app.on_go_back(move || {
        println!("Discowl: go back");
        if let Some(a) = adapter_weak3.upgrade() {
            if let Some(ref adapter) = *a.borrow() {
                adapter.webview().go_back(1);
            }
        }
    });

    let adapter_weak4 = Rc::downgrade(&adapter);
    app.on_go_forward(move || {
        println!("Discowl: go forward");
        if let Some(a) = adapter_weak4.upgrade() {
            if let Some(ref adapter) = *a.borrow() {
                adapter.webview().go_forward(1);
            }
        }
    });

    let adapter_weak5 = Rc::downgrade(&adapter);
    app.on_reload(move || {
        println!("Discowl: reload");
        if let Some(a) = adapter_weak5.upgrade() {
            if let Some(ref adapter) = *a.borrow() {
                adapter.webview().reload();
            }
        }
    });

    app.run().unwrap();
}

fn setup_slint_renderer() {
    use slint::wgpu_29::{WGPUConfiguration, WGPUSettings};

    let mut wgpu_settings = WGPUSettings::default();

    #[cfg(target_os = "windows")]
    {
        wgpu_settings.backends = slint::wgpu_29::wgpu::Backends::DX12;
    }

    slint::BackendSelector::new()
        .require_wgpu_29(WGPUConfiguration::Automatic(wgpu_settings))
        .select()
        .unwrap();
}
