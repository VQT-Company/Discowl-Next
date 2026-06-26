# Discowl Performance Analysis

## Architecture Overview

```
DiscowlWindow (Slint)
  └─ Toolbar (URL bar, buttons)
  └─ WebView Area
       └─ Image { source: web-content }  ← Slint image property
       └─ TouchArea / FocusScope          ← forwards mouse/keyboard to Servo

Servo Event Loop (async, smol)
  └─ ServoBuilder → WebView
  └─ SoftwareRenderingContext (OSMesa/software GL)
       └─ WebRender compositor (CPU)
       └─ glReadPixels → RGBA image
       └─ RgbaImage → SharedPixelBuffer → slint::Image::from_rgba8
       └─ Slint femtovg-wgpu uploads to GPU texture
```

## The Problem

### The rendering pipeline has four serial stages:

1. **Servo/WebRender composites** using a software OpenGL implementation (OSMesa)
2. **Readback**: `glReadPixels` copies the composited frame from software framebuffer to CPU memory (`RgbaImage`)
3. **Transfer**: `SharedPixelBuffer::clone_from_slice` copies the pixels into Slint's buffer
4. **Upload**: Slint femtovg-wgpu uploads the buffer as a GPU texture

### Why this is slow

- **GPU → CPU → GPU round-trip**: Servo renders in software (CPU), then pixels must be copied to Slint's wgpu/DX12 texture. Every frame traverses: CPU (Servo) → CPU (memcpy) → GPU (texture upload).
- **No direct GPU sharing**: Servo's `SoftwareRenderingContext` does its own CPU-side rendering. Even though the result is eventually uploaded to a wgpu texture, there is no shared GPU memory — it's always a copy.
- **WebRender on software GL**: Without hardware acceleration, Servo's compositor is CPU-bound. This is the main bottleneck.

### Measured performance (estimated)

| Stage | Approximate cost |
|---|---|
| WebRender composite (software) | 10-50+ ms (CPU-bound) |
| glReadPixels (1080p) | ~0.3 ms |
| memcpy to SharedPixelBuffer | ~0.5 ms |
| wgpu texture upload | ~1-2 ms |
| **Total frame time** | **12-55+ ms** (8-80 FPS) |

## Solutions Considered

### 1. Hardware-accelerated Servo rendering via `WindowRenderingContext`

**How**: Servo's `WindowRenderingContext` renders directly to a native OS window via `raw-window-handle` and `surfman` (ANGLE → D3D11 on Windows). No readback, no CPU copy.

**Problem**: This bypasses Slint entirely. The web content renders to its own HWND. Compositing Slint's UI (toolbar, URL bar) on top requires either:
- A child HWND for the webview inside Slint's window
- A transparent overlay window

Both require platform-specific HWND-level coordination.

**Verdict**: The best technical solution for performance, but requires significant platform-specific work and breaks the Slint-everywhere abstraction.

### 2. GPU interop: share wgpu texture between Servo and Slint

**How**: Use `wgpu::Device::create_texture` with `wgpu::TextureUsages::COPY_SRC` and export as a D3D12 shared handle. Servo would render directly to this shared texture.

**Problem**: Servo's `SoftwareRenderingContext` offers no way to provide an external framebuffer. Servo's `WindowRenderingContext` uses `surfman`/ANGLE, not raw wgpu. The private wgpu device inside Servo is not exposed.

**Verdict**: Not feasible without forking Servo or adding extension APIs.

### 3. Replace Servo with wry/WebView2

**How**: Use `wry` which wraps the system WebView2 (Edge Chromium). Native hardware acceleration, no readback, zero copies. Can be embedded as a child window or overlaid.

**Pros**:
- Native browser performance (Edge Chromium)
- JavaScript, CSS, modern web APIs
- `wry::WebView::bounds()` to position inside Slint window
- System WebView2 is preinstalled on Windows 10+

**Cons**:
- Not Servo (the project's stated goal)
- Depends on system Edge runtime
- Still needs HWND-level coordination with Slint

**Verdict**: Pragmatic solution if "browser" is the goal, not "Servo browser".

### 4. Accept current performance, profile real data

**How**: Build with `--release`, run on real pages, measure actual frame rates. The software WebRender may be fast enough for simple pages.

**Verdict**: This is the first step before any optimisation.

### 5. Optimise the existing pipeline

**Possible micro-optimisations**:
- Reuse `SharedPixelBuffer` to avoid allocation per frame
- Use `Image::from_rgba8_premultiplied` if Servo outputs premultiplied alpha
- Skip `glReadPixels` if the frame hasn't changed (frame-dirty tracking)
- Batch texture uploads

**Problem**: These save a fraction of a millisecond. The real cost is WebRender on a software GL backend.

**Verdict**: Marginal gains at best.

## Current Renderer Setup

- **Backend**: `renderer-femtovg-wgpu` + `unstable-wgpu-29`
- **GPU**: DX12 via `Backends::DX12`
- **Context**: `SoftwareRenderingContext` (OSMesa/software GL)
- **Skia**: Not usable — `skia-bindings 0.99.0` fails SSL download on this network

## Key Files

| File | Role |
|---|---|
| `src/discowl.slint` | UI layout — toolbar + FocusScope wrapping Image + TouchArea |
| `src/main.rs` | Entry point, maximized window, rendering notifier, event forwarding |
| `src/webview/adapter.rs` | `SlintServoAdapter` — bridges Servo WebView and Slint UI |
| `src/webview/mod.rs` | `create_webview()` — async Servo event loop with resize polling |
| `src/webview/delegate.rs` | `AppDelegate` — `notify_new_frame_ready` triggers paint + image update |
| `src/webview/rendering_context/servo_rendering_adapter.rs` | `SoftwareAdapter` — `current_framebuffer_as_image()` via readback |
| `Cargo.toml` | Dependencies: slint (femtovg-wgpu), servo 0.3.0, surfman, winit |

## Recommended Next Steps

1. **Build `--release` and benchmark**: Real-world performance may be acceptable for simple pages.
2. **Profile with tracing**: Enable Servo's `profiling` feature to identify the actual bottleneck.
3. **If performance is unacceptable**: Investigate `WindowRenderingContext` with child-HWND compositing, or consider `wry` as a pragmatic alternative.
4. **Hybrid approach**: Use `WindowRenderingContext` for the webview area and overlay Slint's UI via transparent window regions or layered windows (Windows-specific).

## Dependencies

- Servo 0.3.0
- Slint 1.17 (femtovg-wgpu + unstable-wgpu-29)
- wgpu 0.29 (DX12 on Windows)
- surfman 0.12 (chains + sm-angle-default on Windows)
- winit 0.30.12
- image 0.25
- euclid 0.22
- url 2.5
- smol 2.0
- raw-window-handle 0.6
- keyboard-types 0.8.3
