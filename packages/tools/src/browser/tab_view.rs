//! How a tab's WebView is built: the probe, the handlers that tell the host of its loads, titles
//! and new windows, and on macOS the configuration all tabs share.

use std::sync::Arc;

use tao::event_loop::EventLoopProxy;
use wry::{NewWindowResponse, PageLoadEvent, WebView, WebViewBuilder};

use super::guard::allowed;
use super::host::{Cmd, PROBE, Shared, Window};
use super::platform::user_agent;
use super::tabs::BLANK;

pub(super) fn builder<'a>(
    id: u32,
    url: &'a str,
    window: &Window,
    shared: &Arc<Shared>,
    proxy: EventLoopProxy<Cmd>,
    with_ipc: bool,
) -> WebViewBuilder<'a> {
    let ipc = Arc::clone(shared);
    let loads = Arc::clone(shared);
    let requests = Arc::clone(shared);
    let titles = Arc::clone(shared);
    // A title or a load changes what the strip names the tab.
    let (titled, loaded) = (proxy.clone(), proxy.clone());
    let mut builder = WebViewBuilder::new()
        .with_url(url)
        .with_incognito(true)
        .with_initialization_script_for_main_only(
            PROBE.replace("__TOKEN__", &shared.token).replace(
                "__HEADLESS__",
                if window.headless { "true" } else { "false" },
            ),
            true,
        )
        .with_on_page_load_handler(move |event, url| {
            super::debug(|| {
                let kind = if matches!(event, PageLoadEvent::Started) {
                    "started"
                } else {
                    "finished"
                };
                format!("tab {id} load {kind}: {url}")
            });
            loads.page_load(id, event, &url);
            let _ = loaded.send_event(Cmd::Strip);
        })
        .with_document_title_changed_handler(move |title| {
            titles.titled(id, title);
            let _ = titled.send_event(Cmd::Strip);
        })
        .with_navigation_handler(move |url| {
            let allow = allowed(&url) || url == BLANK;
            super::debug(|| format!("tab {id} navigation requested: {url} (allowed: {allow})"));
            if allow {
                requests.navigation_requested(id);
            }
            allow
        })
        .with_download_started_handler(|_url, _path| false)
        // A link with target=_blank or `window.open` opens a tab, which is shown: what the page
        // opened is what it is about now.
        .with_new_window_req_handler(move |url, _features| {
            if allowed(&url) {
                let _ = proxy.send_event(Cmd::NewTab { url, reply: None });
            }
            NewWindowResponse::Deny
        })
        .with_background_throttling(wry::BackgroundThrottlingPolicy::Disabled);
    if with_ipc {
        builder = builder.with_ipc_handler(move |request| ipc.ipc(request.body()));
    }
    if let Some(agent) = user_agent() {
        builder = builder.with_user_agent(agent);
    }
    builder
}

/// Whether the tabs share one WebKit configuration (see `share`).
pub(super) const SHARED: bool = cfg!(target_os = "macos");

/// On macOS a new tab shares the first one's configuration: one cookie store for the window, as
/// in a browser (a page opened in a new tab stays signed in), and one IPC channel.
#[cfg(target_os = "macos")]
pub(super) fn share<'a>(
    builder: WebViewBuilder<'a>,
    first: Option<&WebView>,
) -> WebViewBuilder<'a> {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2_web_kit::WKWebViewConfiguration;
    use wry::{WebViewBuilderExtMacos, WebViewExtMacOS};
    let Some(first) = first else {
        return builder;
    };
    let view = first.webview();
    // SAFETY: the first tab's WKWebView is alive (borrowed); `configuration` answers a copy of
    // its WKWebViewConfiguration, retained.
    let configuration: Retained<WKWebViewConfiguration> =
        unsafe { msg_send![&*view, configuration] };
    builder.with_webview_configuration(configuration)
}

#[cfg(not(target_os = "macos"))]
pub(super) fn share<'a>(
    builder: WebViewBuilder<'a>,
    _first: Option<&WebView>,
) -> WebViewBuilder<'a> {
    builder
}

#[cfg(not(target_os = "linux"))]
pub(super) fn build(builder: WebViewBuilder, window: &tao::window::Window) -> wry::Result<WebView> {
    builder.build_as_child(window)
}

#[cfg(target_os = "linux")]
pub(super) fn build(builder: WebViewBuilder, window: &tao::window::Window) -> wry::Result<WebView> {
    use tao::platform::unix::WindowExtUnix;
    use wry::WebViewBuilderExtUnix;
    builder.build_gtk(window.default_vbox().expect("tao window has a vbox"))
}
