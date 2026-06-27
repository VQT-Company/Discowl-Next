use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::ComponentHandle;

use servo::{WebView, WebViewDelegate};

use super::SlintServoAdapter;

pub struct AppDelegate {
    app: slint::Weak<crate::DiscowlWindow>,
    adapter: Rc<SlintServoAdapter>,
    last_frame: Cell<Instant>,
    frame_interval: Duration,
    has_frame: Cell<bool>,
}

impl AppDelegate {
    pub fn new(
        app: &crate::DiscowlWindow,
        adapter: Rc<SlintServoAdapter>,
    ) -> Self {
        Self {
            app: app.as_weak(),
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
        if let Some(app) = self.app.upgrade() {
            self.adapter.update_web_content_with_latest_frame(&app);
            self.has_frame.set(true);
        }
        self.adapter.present();
    }

    fn notify_url_changed(&self, _webview: WebView, url: url::Url) {
        if let Some(app) = self.app.upgrade() {
            app.set_current_url(url.to_string().into());
        }
    }
}
