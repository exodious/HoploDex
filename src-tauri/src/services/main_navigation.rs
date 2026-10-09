//! Where the main window's web view may navigate (#73).
//!
//! The main web view shows the app's own page and nothing else. Its content
//! security policy stops the page's scripts reaching the network, but a
//! top-level navigation (a link, a script setting `location`, a redirect, a
//! dropped file) is not governed by it, and could carry data out in the URL
//! or replace the app with another page. So the window gets a native
//! navigation allowlist: the origin the app is served from, and in a
//! development run the dev server's. Everything else is refused, and so is a
//! new window (`on_new_window` denies it, main.rs).
//!
//! The PDF surface's child web view has its own, narrower allowlist
//! (`services::preview::surface`, research.md §6); this is the main window's
//! (research.md §6, amended for #73).

use tauri::Url;

/// The origin Tauri serves the packaged frontend from: `tauri://localhost`
/// on Linux and macOS, `http://tauri.localhost` on Windows (WebView2 has no
/// custom schemes, so wry maps `tauri:` to that host). HoploDex does not set
/// `useHttpsScheme`, so the `https` form is not an app origin.
pub fn packaged_origin(windows: bool) -> Url {
    let origin = if windows { "http://tauri.localhost" } else { "tauri://localhost" };
    Url::parse(origin).expect("a constant URL")
}

/// The origins the main web view may be on: the packaged one for this OS
/// and, only when a development run supplies it, the dev server's.
pub fn app_origins(windows: bool, dev_url: Option<&Url>) -> Vec<Url> {
    let mut origins = vec![packaged_origin(windows)];
    origins.extend(dev_url.cloned());
    origins
}

/// Whether the main web view may navigate to `target`: only a page of one of
/// the app's `origins`, compared by scheme, host and port. The comparison is
/// on the parsed URL's parts, never on text, so a look-alike host
/// (`tauri.localhost.evil.com`) or userinfo (`tauri://localhost@evil.com`,
/// whose host is `evil.com`) doesn't pass; a URL that carries userinfo at
/// all is refused. `file:`, `data:`, `blob:`, `javascript:` and `about:`
/// have no matching origin.
pub fn navigation_allowed(origins: &[Url], target: &Url) -> bool {
    if !target.username().is_empty() || target.password().is_some() {
        return false;
    }
    origins.iter().any(|origin| {
        target.scheme() == origin.scheme()
            && target.host_str().is_some()
            && target.host_str() == origin.host_str()
            && target.port_or_known_default() == origin.port_or_known_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    fn packaged(windows: bool) -> Vec<Url> {
        app_origins(windows, None)
    }

    #[test]
    fn the_packaged_page_is_allowed_in_the_form_each_os_serves_it() {
        // Linux and macOS.
        assert!(navigation_allowed(&packaged(false), &url("tauri://localhost")));
        assert!(navigation_allowed(&packaged(false), &url("tauri://localhost/")));
        assert!(navigation_allowed(&packaged(false), &url("tauri://localhost/index.html?x=1#y")));
        // Windows.
        assert!(navigation_allowed(&packaged(true), &url("http://tauri.localhost")));
        assert!(navigation_allowed(&packaged(true), &url("http://tauri.localhost/")));
        assert!(navigation_allowed(&packaged(true), &url("http://tauri.localhost/assets/a.js")));
    }

    #[test]
    fn one_oss_form_is_not_anothers() {
        assert!(!navigation_allowed(&packaged(false), &url("http://tauri.localhost/")));
        assert!(!navigation_allowed(&packaged(false), &url("https://tauri.localhost/")));
        assert!(!navigation_allowed(&packaged(true), &url("tauri://localhost/")));
        // HoploDex does not use the https form on Windows.
        assert!(!navigation_allowed(&packaged(true), &url("https://tauri.localhost/")));
    }

    #[test]
    fn a_foreign_page_is_refused() {
        for windows in [false, true] {
            let origins = packaged(windows);
            for target in [
                "http://example.com/",
                "https://example.com/collect?data=secret",
                "http://localhost/",
                "http://127.0.0.1:8080/",
                "http://[::1]/",
                "https://localhost/",
                "http://tauri.localhost:8080/",
                "tauri://localhost:1234/",
                "tauri://example.com/",
                "ws://localhost/",
                "ftp://tauri.localhost/",
            ] {
                assert!(
                    !navigation_allowed(&origins, &url(target)),
                    "{target} (windows: {windows})"
                );
            }
        }
    }

    #[test]
    fn other_schemes_are_refused() {
        for windows in [false, true] {
            let origins = packaged(windows);
            for target in [
                "file:///etc/passwd",
                "file://localhost/etc/passwd",
                "data:text/html,<script>alert(1)</script>",
                "javascript:alert(1)",
                "blob:tauri://localhost/00000000-0000-0000-0000-000000000000",
                "blob:http://tauri.localhost/00000000-0000-0000-0000-000000000000",
                "about:blank",
                "hdpreview://localhost/abc/document.pdf",
                "asset://localhost/etc/passwd",
                "ipc://localhost/get_database_status",
            ] {
                assert!(
                    !navigation_allowed(&origins, &url(target)),
                    "{target} (windows: {windows})"
                );
            }
        }
    }

    #[test]
    fn look_alike_hosts_and_userinfo_are_refused() {
        let linux = packaged(false);
        for target in [
            "tauri://localhost.evil.com/",
            "tauri://evil.com/localhost",
            "tauri://localhost@evil.com/",
            "tauri://localhost:pw@evil.com/",
            "tauri://user@localhost/",
            "tauri://user:pw@localhost/",
            "tauri://xlocalhost/",
            "tauri://localhostx/",
            "tauri://evil.localhost/",
        ] {
            assert!(!navigation_allowed(&linux, &url(target)), "{target}");
        }
        let windows = packaged(true);
        for target in [
            "http://tauri.localhost.evil.com/",
            "http://tauri.localhost@evil.com/",
            "http://tauri.localhost:80@evil.com/",
            "http://tauri.localhost%2f@evil.com/",
            "http://user@tauri.localhost/",
            "http://evil.com/?http://tauri.localhost/",
            "http://evil.com#@tauri.localhost/",
            "http://tauri.localhost.@evil.com/",
            "http://evil.tauri.localhost/",
            "http://xtauri.localhost/",
        ] {
            assert!(!navigation_allowed(&windows, &url(target)), "{target}");
        }
    }

    #[test]
    fn the_dev_server_is_allowed_only_when_one_is_given() {
        let dev = url("http://localhost:1420");
        let with_dev = app_origins(false, Some(&dev));
        assert!(navigation_allowed(&with_dev, &url("http://localhost:1420/")));
        assert!(navigation_allowed(&with_dev, &url("http://localhost:1420/src/main.tsx")));
        assert!(navigation_allowed(&with_dev, &url("tauri://localhost/")));
        // Not another port, host or scheme.
        assert!(!navigation_allowed(&with_dev, &url("http://localhost:1421/")));
        assert!(!navigation_allowed(&with_dev, &url("http://localhost/")));
        assert!(!navigation_allowed(&with_dev, &url("http://127.0.0.1:1420/")));
        assert!(!navigation_allowed(&with_dev, &url("https://localhost:1420/")));
        // Without it (a release or E2E build), the dev server is just a
        // foreign page.
        assert!(!navigation_allowed(&packaged(false), &url("http://localhost:1420/")));
    }
}
