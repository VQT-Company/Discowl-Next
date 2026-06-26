use std::rc::Rc;

use slint::ComponentHandle;

use servo::{WebView, WebViewDelegate};

use super::SlintServoAdapter;

pub struct AppDelegate {
    app: slint::Weak<crate::DiscowlWindow>,
    adapter: Rc<SlintServoAdapter>,
}

impl AppDelegate {
    pub fn new(
        app: &crate::DiscowlWindow,
        adapter: Rc<SlintServoAdapter>,
    ) -> Self {
        Self {
            app: app.as_weak(),
            adapter,
        }
    }
}

impl WebViewDelegate for AppDelegate {
    fn notify_new_frame_ready(&self, webview: WebView) {
        webview.paint();
        if let Some(app) = self.app.upgrade() {
            self.adapter.update_web_content_with_latest_frame(&app);
        }
    }

    fn notify_url_changed(&self, _webview: WebView, url: url::Url) {
        if let Some(app) = self.app.upgrade() {
            app.set_current_url(url.to_string().into());
        }
    }
}
