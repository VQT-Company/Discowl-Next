mod webview;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use slint::ComponentHandle;

slint::include_modules!();

struct TabEntry {
    url: String,
    title: String,
    tab_id: usize,
    adapter: Rc<webview::SlintServoAdapter>,
}

fn update_tab_titles(app: &DiscowlWindow, tabs: &[TabEntry]) {
    let titles: Vec<slint::SharedString> = tabs
        .iter()
        .map(|t| {
            let label = url::Url::parse(&t.url)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_string()))
                .unwrap_or_else(|| t.title.clone());
            label.into()
        })
        .collect();
    app.set_tab_titles(titles.as_slice().into());
}

fn make_tab_closures(
    app_weak: slint::Weak<DiscowlWindow>,
    tabs: &Rc<RefCell<Vec<TabEntry>>>,
    active_tab_id: &Rc<Cell<usize>>,
    tab_id: usize,
) -> (
    Box<dyn Fn(slint::Image)>,
    Box<dyn Fn(slint::SharedString)>,
    Box<dyn Fn() -> (u32, u32)>,
) {
    let tabs_weak = Rc::downgrade(tabs);
    let active = active_tab_id.clone();

    let set_web_content = {
        let aw = app_weak.clone();
        let act = active.clone();
        move |img: slint::Image| {
            if act.get() == tab_id {
                if let Some(app) = aw.upgrade() {
                    app.set_web_content(img);
                    app.window().request_redraw();
                }
            }
        }
    };

    let set_current_url = {
        let aw = app_weak.clone();
        let tw = tabs_weak.clone();
        move |url: slint::SharedString| {
            let url_str = url.to_string();
            if let Some(tabs) = tw.upgrade() {
                let mut tabs = tabs.borrow_mut();
                if let Some(tab) = tabs.iter_mut().find(|t| t.tab_id == tab_id) {
                    tab.url = url_str;
                }
            }
            if let (Some(tabs), Some(app)) = (tw.upgrade(), aw.upgrade()) {
                update_tab_titles(&app, &tabs.borrow());
            }
        }
    };

    let get_webview_size = {
        let aw = app_weak.clone();
        move || -> (u32, u32) {
            if let Some(app) = aw.upgrade() {
                let scale = app.window().scale_factor();
                let w = (app.get_webview_width() as f32 * scale) as u32;
                let h = (app.get_webview_height() as f32 * scale) as u32;
                (w.max(1), h.max(1))
            } else {
                (1, 1)
            }
        }
    };

    (
        Box::new(set_web_content),
        Box::new(set_current_url),
        Box::new(get_webview_size),
    )
}

fn main() {
    setup_slint_renderer();

    let app = DiscowlWindow::new().unwrap();
    app.window().set_maximized(true);

    let all_adapters: Rc<RefCell<Vec<Rc<webview::SlintServoAdapter>>>> =
        Rc::new(RefCell::new(Vec::new()));
    let tabs: Rc<RefCell<Vec<TabEntry>>> = Rc::new(RefCell::new(Vec::new()));
    let active_tab_id: Rc<Cell<usize>> = Rc::new(Cell::new(0));
    let next_tab_id: Rc<Cell<usize>> = Rc::new(Cell::new(0));

    let device: Rc<RefCell<Option<slint::wgpu_29::wgpu::Device>>> =
        Rc::new(RefCell::new(None));
    let queue: Rc<RefCell<Option<slint::wgpu_29::wgpu::Queue>>> =
        Rc::new(RefCell::new(None));
    let shared_servo: Rc<RefCell<Option<Rc<servo::Servo>>>> =
        Rc::new(RefCell::new(None));

    let for_notifier = all_adapters.clone();
    let for_callbacks = all_adapters.clone();

    let app_w = app.as_weak();
    let tabs_w = Rc::downgrade(&tabs);
    let active_w = active_tab_id.clone();
    let next_w = next_tab_id.clone();
    let dev_w = device.clone();
    let que_w = queue.clone();
    let serv_w = shared_servo.clone();

    let initialized = Cell::new(false);

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
            let app = app_w.upgrade().unwrap();
            let device = device.clone();
            let queue = queue.clone();
            let scale = app.window().scale_factor() as f32;

            let (waker_sender, waker_receiver) = smol::channel::unbounded();
            let waker = webview::Waker::new(waker_sender);
            let serv = Rc::new(
                servo::ServoBuilder::default()
                    .event_loop_waker(Box::new(waker))
                    .build(),
            );
            *serv_w.borrow_mut() = Some(serv.clone());
            *dev_w.borrow_mut() = Some(device.clone());
            *que_w.borrow_mut() = Some(queue.clone());

            next_w.set(1);

            let tabs_main = match tabs_w.upgrade() {
                Some(t) => t,
                None => return,
            };

            let (set_web, set_url, get_size) =
                make_tab_closures(app_w.clone(), &tabs_main, &active_w, 0);

            let main_adapter = webview::create_webview(
                &*serv,
                "https://google.com".into(),
                device,
                queue,
                set_web,
                set_url,
                get_size,
                scale,
            );
            main_adapter.set_active(true);

            if let Some(t) = tabs_w.upgrade() {
                t.borrow_mut().push(TabEntry {
                    url: "https://google.com".into(),
                    title: "Google".into(),
                    tab_id: 0,
                    adapter: main_adapter.clone(),
                });
            }
            for_notifier.borrow_mut().push(main_adapter);
            active_w.set(0);

            app.set_current_tab_index(0);
            if let Some(t) = tabs_w.upgrade() {
                update_tab_titles(&app, &t.borrow());
            }

            let weak_aa = Rc::downgrade(&for_notifier);
            slint::spawn_local(async move {
                use std::time::{Duration, Instant};

                let spin_interval = Duration::from_millis(150);
                let mut last_spin = Instant::now();

                loop {
                    let _ = waker_receiver.recv().await;
                    while waker_receiver.try_recv().is_ok() {}

                    if last_spin.elapsed() >= spin_interval {
                        last_spin = Instant::now();
                        serv.spin_event_loop();

                        if let Some(aa) = weak_aa.upgrade() {
                            for a in aa.borrow().iter() {
                                a.resize_webview_if_needed();
                            }
                        } else {
                            break;
                        }
                    }
                }
            })
            .expect("Failed to spawn servo event loop");

            initialized.set(true);
        })
        .unwrap();

    // ── Tab: new-tab ───────────────────────────────────────────────
    {
        let aw = app.as_weak();
        let tabs = tabs.clone();
        let active = active_tab_id.clone();
        let next = next_tab_id.clone();
        let dev = device.clone();
        let que = queue.clone();
        let serv = shared_servo.clone();
        let all = for_callbacks.clone();

        app.on_new_tab(move || {
            let device = match dev.borrow().as_ref() {
                Some(d) => d.clone(),
                None => return,
            };
            let queue = match que.borrow().as_ref() {
                Some(q) => q.clone(),
                None => return,
            };
            let servo = match serv.borrow().as_ref() {
                Some(s) => s.clone(),
                None => return,
            };
            let app = match aw.upgrade() {
                Some(a) => a,
                None => return,
            };

            let tab_id = next.get();
            next.set(tab_id + 1);

            let (set_web, set_url, get_size) =
                make_tab_closures(aw.clone(), &tabs, &active, tab_id);

            let adapter = webview::create_webview(
                &*servo,
                "about:blank".into(),
                device,
                queue,
                set_web,
                set_url,
                get_size,
                app.window().scale_factor() as f32,
            );
            adapter.set_active(false);

            let mut tabs_mut = tabs.borrow_mut();
            let old = active.get();
            if old < tabs_mut.len() {
                tabs_mut[old].adapter.set_active(false);
            }
            tabs_mut.push(TabEntry {
                url: "about:blank".into(),
                title: "New Tab".into(),
                tab_id,
                adapter: adapter.clone(),
            });
            let idx = tabs_mut.len() - 1;
            active.set(idx);
            tabs_mut[idx].adapter.set_active(true);
            drop(tabs_mut);

            all.borrow_mut().push(adapter.clone());

            app.set_current_tab_index(idx as i32);
            update_tab_titles(&app, &tabs.borrow());

            let tabs_ref = tabs.borrow();
            tabs_ref[idx].adapter.webview().paint();
            tabs_ref[idx].adapter.update_web_content_with_latest_frame();
            tabs_ref[idx].adapter.present();
        });
    }

    // ── Tab: close-tab ─────────────────────────────────────────────
    {
        let aw = app.as_weak();
        let tabs = tabs.clone();
        let active = active_tab_id.clone();

        app.on_close_tab(move |idx: i32| {
            let idx = idx as usize;
            let mut tabs_mut = tabs.borrow_mut();
            if tabs_mut.len() <= 1 {
                return;
            }

            let was_active = idx == active.get();
            tabs_mut.remove(idx);

            if was_active {
                let new_idx = idx.min(tabs_mut.len().saturating_sub(1));
                active.set(new_idx);
                tabs_mut[new_idx].adapter.set_active(true);
                let adapter = tabs_mut[new_idx].adapter.clone();
                drop(tabs_mut);

                if let Some(app) = aw.upgrade() {
                    app.set_current_tab_index(new_idx as i32);
                    adapter.webview().paint();
                    adapter.update_web_content_with_latest_frame();
                    adapter.present();
                    update_tab_titles(&app, &tabs.borrow());
                }
            } else {
                if active.get() > idx {
                    active.set(active.get() - 1);
                }
                let new_active = active.get();
                drop(tabs_mut);

                if let Some(app) = aw.upgrade() {
                    app.set_current_tab_index(new_active as i32);
                    update_tab_titles(&app, &tabs.borrow());
                }
            }
        });
    }

    // ── Tab: switch-tab ────────────────────────────────────────────
    {
        let aw = app.as_weak();
        let tabs = tabs.clone();
        let active = active_tab_id.clone();

        app.on_switch_tab(move |idx: i32| {
            let idx = idx as usize;
            let tabs = tabs.borrow();
            if idx >= tabs.len() {
                return;
            }

            let old = active.get();
            if old == idx {
                return;
            }

            tabs[old].adapter.set_active(false);
            tabs[idx].adapter.set_active(true);
            active.set(idx);

            if let Some(app) = aw.upgrade() {
                app.set_current_tab_index(idx as i32);
                tabs[idx].adapter.webview().paint();
                tabs[idx].adapter.update_web_content_with_latest_frame();
                tabs[idx].adapter.present();
            }
        });
    }

    // ── Navigation (active tab) ────────────────────────────────────
    {
        let tabs = tabs.clone();
        let active = active_tab_id.clone();

        app.on_navigate(move |url: slint::SharedString| {
            let tabs = tabs.borrow();
            let idx = active.get();
            let adapter = match tabs.get(idx) {
                Some(t) => t.adapter.clone(),
                None => return,
            };
            drop(tabs);
            if let Ok(url) = url::Url::parse(&url) {
                adapter.webview().load(url);
            } else {
                let servo_url =
                    webview::events_utils::convert_input_string_to_servo_url(&url);
                adapter.webview().load(servo_url.into_url());
            }
        });
    }

    {
        let tabs = tabs.clone();
        let active = active_tab_id.clone();

        app.on_go_back(move || {
            let tabs = tabs.borrow();
            let idx = active.get();
            if let Some(t) = tabs.get(idx) {
                t.adapter.webview().go_back(1);
            }
        });
    }

    {
        let tabs = tabs.clone();
        let active = active_tab_id.clone();

        app.on_go_forward(move || {
            let tabs = tabs.borrow();
            let idx = active.get();
            if let Some(t) = tabs.get(idx) {
                t.adapter.webview().go_forward(1);
            }
        });
    }

    {
        let tabs = tabs.clone();
        let active = active_tab_id.clone();

        app.on_reload(move || {
            let tabs = tabs.borrow();
            let idx = active.get();
            if let Some(t) = tabs.get(idx) {
                t.adapter.webview().reload();
            }
        });
    }

    // ── Pointer events ─────────────────────────────────────────────
    {
        let tabs = tabs.clone();
        let active = active_tab_id.clone();
        let aw = app.as_weak();

        use slint::language::{PointerEventButton, PointerEventKind};

        app.global::<WebviewEvents>().on_webview_pointer_event(
            move |event: slint::language::PointerEvent, x: f32, y: f32| {
                let tabs = tabs.borrow();
                let active_idx = active.get();
                let adapter = match tabs.get(active_idx) {
                    Some(t) => t.adapter.clone(),
                    None => return,
                };
                drop(tabs);

                let scale = match aw.upgrade() {
                    Some(ref app) => app.window().scale_factor() as f32,
                    None => 1.0,
                };
                let point = euclid::Point2D::new(x * scale, y * scale);

                use servo::{
                    InputEvent, MouseButton, MouseButtonAction, MouseButtonEvent,
                    MouseMoveEvent, WebViewPoint,
                };

                let servo_button = match event.button {
                    PointerEventButton::Left => MouseButton::Left,
                    PointerEventButton::Right => MouseButton::Right,
                    PointerEventButton::Middle => MouseButton::Middle,
                    _ => MouseButton::Left,
                };

                match event.kind {
                    PointerEventKind::Down => {
                        adapter
                            .webview()
                            .notify_input_event(InputEvent::MouseMove(
                                MouseMoveEvent::new(WebViewPoint::Device(point)),
                            ));
                        adapter
                            .webview()
                            .notify_input_event(InputEvent::MouseButton(
                                MouseButtonEvent::new(
                                    MouseButtonAction::Down,
                                    servo_button,
                                    WebViewPoint::Device(point),
                                ),
                            ));
                    }
                    PointerEventKind::Up => {
                        adapter
                            .webview()
                            .notify_input_event(InputEvent::MouseButton(
                                MouseButtonEvent::new(
                                    MouseButtonAction::Up,
                                    servo_button,
                                    WebViewPoint::Device(point),
                                ),
                            ));
                    }
                    PointerEventKind::Move => {
                        adapter
                            .webview()
                            .notify_input_event(InputEvent::MouseMove(
                                MouseMoveEvent::new(WebViewPoint::Device(point)),
                            ));
                    }
                    _ => {}
                }
            },
        );
    }

    // ── Keyboard events ────────────────────────────────────────────
    {
        let tabs = tabs.clone();
        let active = active_tab_id.clone();

        use servo::{
            Code, InputEvent, Key, KeyState, KeyboardEvent, Location, NamedKey,
        };

        app.global::<WebviewEvents>().on_webview_key_event(
            move |event: slint::language::KeyEvent| {
                let tabs = tabs.borrow();
                let active_idx = active.get();
                let adapter = match tabs.get(active_idx) {
                    Some(t) => t.adapter.clone(),
                    None => return,
                };
                drop(tabs);

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
                adapter
                    .webview()
                    .notify_input_event(InputEvent::Keyboard(servo_kb));
            },
        );
    }

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
