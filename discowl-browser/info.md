# Discowl — Servo + Slint Browser Integration

## Architecture

```
┌──────────────────────────────────────────────┐
│                 Slint UI                      │
│  ┌──────────────────────────────────────────┐ │
│  │  Toolbar (Back, Forward, Reload, URL)    │ │
│  └──────────────────────────────────────────┘ │
│  ┌──────────────────────────────────────────┐ │
│  │  WebView Area                            │ │
│  │  ┌────────────────────────────────────┐  │ │
│  │  │  Image (web-content from Servo)    │  │ │
│  │  └────────────────────────────────────┘  │ │
│  └──────────────────────────────────────────┘ │
└──────────────────────────────────────────────┘
         ▲              │
         │  Image       │ callbacks (navigate,
         │  update      │ go-back, go-forward,
         │              │ reload)
    ┌────┴──────────────┴──────────────────────┐
    │           webview module                  │
    │  ┌─────────────────┐  ┌────────────────┐  │
    │  │ SlintServoAdapter│  │ AppDelegate    │  │
    │  │ (state bridge)   │  │ (Servo events) │  │
    │  └────────┬────────┘  └───────┬────────┘  │
    │           │                    │           │
    │  ┌────────▼────────────────────▼────────┐  │
    │  │ Servo WebView + SoftwareRenderingCtx │  │
    │  │ (renders web pages via software GL)  │  │
    │  └─────────────────────────────────────┘  │
    └───────────────────────────────────────────┘
```

## Files

### `src/main.rs`
Entry point. Sets up WGPU renderer for Slint, creates `DiscowlWindow`, installs rendering notifier to capture GPU device/queue, creates Servo WebView, wires toolbar callbacks.

### `src/discowl.slint`
UI layout: toolbar (logo, back, forward, reload, URL bar, go button) + webview rectangle. Exposes `web-content` (Image), `webview-width/height`, navigation callbacks, and global `WebviewEvents` for pointer events.

### `src/webview/mod.rs`
Orchestrator. Creates Servo engine, SoftwareRenderingContext, WebView, delegate, waker, and spawns async event loop via `slint::spawn_local`.

### `src/webview/adapter.rs`
`SlintServoAdapter` — holds Servo, WebView, rendering adapter, WGPU device/queue. Bridges frame updates to Slint via `update_web_content_with_latest_frame()`.

### `src/webview/delegate.rs`
`AppDelegate` implements `WebViewDelegate`. On `notify_new_frame_ready`: paints Servo frame, reads pixels, converts to Slint Image, updates UI. Also tracks URL changes.

### `src/webview/waker.rs`
`EventLoopWaker` impl: sends `()` via `smol::channel` when Servo needs event processing.

### `src/webview/rendering_context/servo_rendering_adapter.rs`
Software rendering path: `ServoRenderingAdapter` trait + `SoftwareAdapter` using `SoftwareRenderingContext::read_to_image()` → `SharedPixelBuffer` → `Image::from_rgba8`.

### `src/webview/events_utils/url_event_util.rs`
URL parsing utility: direct URL → domain fix → search engine fallback.

### `Cargo.toml`
```
slint (WGPU-29 + femtovg-wgpu)  →  UI rendering
servo (no-wgl on Windows)       →  browser engine
surfman (chains + ANGLE)        →  software GL context
url, smol, winit, euclid        →  helpers
```

### `build.rs`
Generates GL bindings via `gl_generator`, compiles `discowl.slint`.

## Rendering Pipeline

1. Servo renders to `SoftwareRenderingContext` (offscreen software GL)
2. `AppDelegate::notify_new_frame_ready` fires
3. `webview.paint()` renders into the context
4. `read_to_image()` reads RGBA pixels from framebuffer
5. Pixels wrapped in `SharedPixelBuffer` → `Image::from_rgba8`
6. `app.set_web_content(image)` updates the Slint UI
7. `app.window().request_redraw()` triggers Slint repaint

## Navigation

| UI Action | Callback | Servo API |
|---|---|---|
| Enter URL / Go | `navigate(url)` | `webview.load(url)` |
| Back button | `go-back()` | `webview.go_back(1)` |
| Forward button | `go-forward()` | `webview.go_forward(1)` |
| Reload button | `reload()` | `webview.reload()` |

## Notes

- Uses **software rendering** (CPU readback via surfman) — no GPU texture sharing, works on all platforms
- Performance is adequate for basic browsing; upgrade to GPU sharing for higher FPS
- Servo's event loop runs asynchronously via `slint::spawn_local`
- Keyboard input forwarding not yet implemented (mouse clicks work)
