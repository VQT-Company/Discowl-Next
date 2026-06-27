mod webview;

use std::cell::Cell;
use std::rc::Rc;

use slint::ComponentHandle;

slint::include_modules!();

fn main() {
    setup_slint_renderer();

    let app = DiscowlWindow::new().unwrap();
    // Hint: request maximized before the window exists.  The call inside the
    // rendering notifier (below) is the reliable one — this is just a backup.
    app.window().set_maximized(true);

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

            // Maximise the window *after* the winit window exists.  Calling
            // set_maximized(true) before run() stores the request but the
            // underlying OS window may not exist yet, so the hint can be lost.
            if let Some(app) = app_weak.upgrade() {
                app.window().set_maximized(true);
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

    // ── Pointer events: forward Slint TouchArea events to Servo ──
    {
        let adapter_weak = Rc::downgrade(&adapter);
        let app_weak = app.as_weak();
        use slint::language::{PointerEventButton, PointerEventKind};
        app.global::<WebviewEvents>().on_webview_pointer_event(
            move |event: slint::language::PointerEvent, x: f32, y: f32| {
                let a = match adapter_weak.upgrade() {
                    Some(a) => a,
                    None => return,
                };
                let adapter = match *a.borrow() {
                    Some(ref a) => a.clone(),
                    None => return,
                };
                let scale = match app_weak.upgrade() {
                    Some(ref app) => app.window().scale_factor() as f32,
                    None => 1.0,
                };
                let point = euclid::Point2D::new(x * scale, y * scale);

                use servo::{InputEvent, MouseButton, MouseButtonAction, MouseButtonEvent, MouseMoveEvent, WebViewPoint};

                let servo_button = match event.button {
                    PointerEventButton::Left => MouseButton::Left,
                    PointerEventButton::Right => MouseButton::Right,
                    PointerEventButton::Middle => MouseButton::Middle,
                    _ => MouseButton::Left,
                };

                match event.kind {
                    PointerEventKind::Down => {
                        adapter.webview().notify_input_event(InputEvent::MouseMove(
                            MouseMoveEvent::new(WebViewPoint::Device(point)),
                        ));
                        adapter.webview().notify_input_event(InputEvent::MouseButton(
                            MouseButtonEvent::new(MouseButtonAction::Down, servo_button, WebViewPoint::Device(point)),
                        ));
                    }
                    PointerEventKind::Up => {
                        adapter.webview().notify_input_event(InputEvent::MouseButton(
                            MouseButtonEvent::new(MouseButtonAction::Up, servo_button, WebViewPoint::Device(point)),
                        ));
                    }
                    PointerEventKind::Move => {
                        adapter.webview().notify_input_event(InputEvent::MouseMove(
                            MouseMoveEvent::new(WebViewPoint::Device(point)),
                        ));
                    }
                    _ => {}
                }
            },
        );
    }

    // ── Keyboard events: forward Slint FocusScope events to Servo ──
    {
        let adapter_weak = Rc::downgrade(&adapter);
        use servo::{InputEvent, KeyboardEvent, Key, KeyState, NamedKey, Code, Location};
        app.global::<WebviewEvents>().on_webview_key_event(
            move |event: slint::language::KeyEvent| {
                let a = match adapter_weak.upgrade() {
                    Some(a) => a,
                    None => return,
                };
                let adapter = match *a.borrow() {
                    Some(ref a) => a.clone(),
                    None => return,
                };
                let text = event.text.to_string();
                let key = if text.len() == 1 {
                    let c = text.chars().next().unwrap();
                    Key::Character(c.to_string())
                } else {
                    match text.as_str() {
                        "enter" | "Return" => Key::Named(NamedKey::Enter),
                        "backspace" | "Backspace" => Key::Named(NamedKey::Backspace),
                        "tab" | "Tab" => Key::Named(NamedKey::Tab),
                        "escape" | "Escape" => Key::Named(NamedKey::Escape),
                        "Delete" => Key::Named(NamedKey::Delete),
                        "Shift" => Key::Named(NamedKey::Shift),
                        "Control" => Key::Named(NamedKey::Control),
                        "Alt" => Key::Named(NamedKey::Alt),
                        _ => Key::Named(NamedKey::Unidentified),
                    }
                };
                let servo_kb = KeyboardEvent::new_without_event(
                    KeyState::Down,
                    key,
                    Code::Unidentified,
                    Location::Standard,
                    convert_modifiers(event.modifiers),
                    false,
                    false,
                );
                adapter.webview().notify_input_event(InputEvent::Keyboard(servo_kb));
            },
        );
    }

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

fn convert_modifiers(m: slint::language::KeyboardModifiers) -> servo::Modifiers {
    use servo::Modifiers;
    let mut result = Modifiers::empty();
    if m.alt {
        result |= Modifiers::ALT;
    }
    if m.control {
        result |= Modifiers::CONTROL;
    }
    if m.shift {
        result |= Modifiers::SHIFT;
    }
    if m.meta {
        result |= Modifiers::META;
    }
    result
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