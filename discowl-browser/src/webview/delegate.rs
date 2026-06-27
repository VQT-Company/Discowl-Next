use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use servo::{WebView, WebViewDelegate};

use super::SlintServoAdapter;

pub struct AppDelegate {
    adapter: Rc<SlintServoAdapter>,
    last_frame: Cell<Instant>,
    frame_interval: Duration,
    has_frame: Cell<bool>,
}

impl AppDelegate {
    pub fn new(adapter: Rc<SlintServoAdapter>) -> Self {
        Self {
            adapter,
            last_frame: Cell::new(Instant::now()),
            frame_interval: Duration::from_millis(200),
            has_frame: Cell::new(false),
        }
    }
}

impl WebViewDelegate for AppDelegate {
    fn notify_new_frame_ready(&self, webview: WebView) {
        let now = Instant::now();
        if now - self.last_frame.get() < self.frame_interval {
            return;
        }
        self.last_frame.set(now);

        webview.paint();
        if self.adapter.is_active() {
            self.adapter.update_web_content_with_latest_frame();
            self.has_frame.set(true);
        }
        self.adapter.present();
    }

    fn notify_url_changed(&self, _webview: WebView, url: url::Url) {
        self.adapter.set_current_url(url.to_string().into());
    }
}
