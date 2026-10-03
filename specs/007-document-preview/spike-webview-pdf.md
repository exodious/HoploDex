# Spike: the web view's own PDF viewer (2026-10-03)

**Question**: can HoploDex preview a PDF with the PDF viewer the web view
already has, instead of bundling or linking a PDF engine (research.md §3 and
§6 chose PDFium from pdfium-binaries; the owner doesn't want a binary someone
else built)? The answer must keep issue #13's findings closed and write
nothing to disk unless the user asks for it.

**Answer on Linux: yes**, with four measures (below). macOS and Windows are
still to be tested, each in its own session.

## What each web view has

| OS | Web view | Built-in PDF viewer | Tested |
|---|---|---|---|
| Linux | WebKitGTK | PDF.js, bundled in WebKitGTK since 2.40 (`webkit-pdfjs-viewer://pdfjs/web/viewer.html`). In Debian's 2.54 (which the AppImage bundles) and Arch's 2.52 | Yes, Debian 2.54 in the dev container |
| macOS | WKWebView | PDFKit, the OS's own | No |
| Windows | WebView2 | Edge's viewer (PDFium in Chromium's sandbox), updated with Edge | No |

So nothing new is bundled: on Linux PDF.js already ships inside the bundled
WebKitGTK, and on macOS and Windows the viewer is the OS's. research.md §3's
"Linux's WebKitGTK has none" was wrong.

## The spike

`src-tauri/examples/pdf_spike.rs`, run by `src-tauri/examples/pdf_spike.sh`
in the dev container under Xvfb with throwaway `HOME` and `XDG_*` dirs. It
opens one window, `preview`, that no capability names, on a PDF that a
custom protocol (`hdpreview`) serves from memory with `Cache-Control:
no-store`. The PDF (3.6 MB) holds a marker string, a link to example.com and
a JavaScript open action. Probes in the top frame and in the viewer's frame
report what each can reach; real X input (XTest) clicks the link and the
viewer's Save button. Afterwards every file written during the run is listed
and searched for the marker.

## Results (Linux, WebKitGTK 2.54)

| Check | Result |
|---|---|
| Renders | Yes: both pages, page count, zoom, and toolbar buttons for find, print and Save. Text layer present, so a screen reader can read it |
| Decrypted content on disk | **None.** No file written during the run holds the marker. Files written: WebKit salts, `hsts-storage.sqlite`, the Mesa shader cache, the fontconfig cache, the dconf user db |
| Where the viewer runs | A child frame with origin `webkit-pdfjs-viewer://pdfjs`. The top frame is our `hdpreview://localhost/doc.pdf` |
| Viewer frame → Tauri IPC | **Blocked.** No `__TAURI_INTERNALS__` (Tauri injects its scripts and the invoke key into the main frame only, `tauri/src/manager/webview.rs:162`). `parent` is cross-origin (`SecurityError`). A raw `ipc://` fetch fails. A `postMessage` to the IPC handler without the key does nothing (the key is 128 random bits, `tauri/src/lib.rs:1250`) |
| Top frame → app commands | **Allowed without an app manifest.** Tauri counts any registered custom protocol as local (`webview/mod.rs:1698`), and with no app ACL manifest a local page may call every app command (`webview/mod.rs:1823`). Plugin commands were refused |
| Top frame → app commands, with an app manifest | **Refused** (`spike_secret not allowed`) |
| Network from either frame | **Reachable** by default. Blocked by a proxy to a port HoploDex holds, or by a content filter; see "Network" below |
| Clicking the PDF's link | A top-level navigation to `http://example.com/link-target`, refused by `on_navigation` |
| The PDF's JavaScript | PDF.js has `enableScripting` on and ships `pdf.sandbox.mjs`. The test PDF's `app.alert` did not reach `window.alert` in any run, but that isn't proof it can't run. Setting `enableScripting` and `enableXfa` to false from a frame script at `webviewerloaded` (PDF.js's hook for embedders) works |
| Save | A navigation to a `blob:webkit-pdfjs-viewer://pdfjs/…` URL. If allowed, Tauri's `on_download` fires; its default destination was the working directory (`/workspace/src-tauri/document.pdf`), so it must be handled, not left to the default. With a destination set, the file was written there |
| Print | Not tried (the GTK print dialog; a user's own choice) |

## Network

The first runs pointed the proxy at 127.0.0.1:9, assuming nothing listens
there. Nothing guarantees that: Windows has no privileged ports, macOS has
let any program bind below 1024 since Mojave, and Linux does too where
`ip_unprivileged_port_start` is lowered. A program listening there would get
the preview's requests. So the second runs tested a port HoploDex holds
itself, and a second, port-free layer.

The viewer frame tried: `fetch` to example.com, to an unresolvable name, to
a fake local service on `127.0.0.1` and on `localhost`; `sendBeacon` and a
WebSocket to that service and to an unresolvable name; images from both;
WebRTC with a STUN server on the service's UDP port and on an unresolvable
name; a `dns-prefetch` link and an anchor to unresolvable names. An
`LD_PRELOAD` shim (`examples/pdf_spike_dns.c`) logged every hostname any
process looked up.

| Variant | What got out |
|---|---|
| No proxy, no filter | **The local service got the beacon, the WebSocket and both fetches** (so a preview could reach any service on the computer), and example.com was reached. Three hostnames were looked up |
| Proxy to 127.0.0.1:9 | Nothing reached the service or the outside; no lookups. Safe only while nothing listens on port 9 |
| Proxy to a port the spike holds (the tripwire) | Nothing reached the service or the outside; no lookups. The tripwire logged all 8 attempts, each as the full URL (`GET http://127.0.0.1:…/fetch-loop`, `GET ws://…`, `POST …/beacon`), including the top frame's |
| Content filter only (WebKit content rules: block everything but the preview's own schemes) | Nothing reached the service or the outside; no lookups; `sendBeacon` refused outright |
| Both, plus WebRTC off | Nothing got out, and the tripwire saw nothing: the filter stopped every request before the proxy |

Other findings:
- **Taking over the tripwire's port fails.** While the spike held it,
  binding `127.0.0.1` or `0.0.0.0` on that port, plain or with
  `SO_REUSEADDR` + `SO_REUSEPORT`, gave "Address already in use".
- **`localhost` and `127.0.0.1` go through the proxy.** wry passes an empty
  bypass list (`wry/src/webkitgtk/mod.rs:275`).
- **The proxy is per preview.** wry sets it on the web view's data manager,
  and `incognito` gives the preview a fresh one, so the main window is
  untouched.
- **WebRTC is off and absent**: `enable-webrtc` defaults to false and
  `RTCPeerConnection` doesn't exist. Turning it off explicitly costs nothing.
  `enable-dns-prefetching` is deprecated and ignored; with the proxy or the
  filter, no lookups happened anyway.
- **WebKit's own web-process sandbox is off** by default; see "WebKit's
  sandbox" below.
- The compiled content filter is written to a store directory (rules only,
  no document content).

**So the network block is three layers, each tested on its own**: the
content filter (no port involved), the proxy to a port HoploDex holds for as
long as a preview exists (a tripwire that logs anything getting past the
filter), and the navigation handler for top-level navigations. On Windows
the port must be bound with `SO_EXCLUSIVEADDRUSE`; the filter there would be
WebView2's `WebResourceRequested`, and on macOS a `WKContentRuleList`.

A test that keeps it blocked: open a PDF in the preview, have a frame script
make every attempt above against a local test service, and fail if the
service or the tripwire sees a connection or the DNS log shows a lookup.

## WebKit's sandbox

WebKitGTK can run each web process (the one that parses the PDF and runs
PDF.js) under bubblewrap. The 4.1 API leaves it off unless the app turns it
on, and `webkit_web_context_set_sandbox_enabled` must be called before the
context has any web process: later, it ends the program (`g_error`). wry
creates the preview's context and loads the first URL inside `build()`
(`wry/src/webkitgtk/mod.rs:372`), so a Tauri-made web view can't turn it on
for itself. What can: `WEBKIT_FORCE_SANDBOX=1` in the environment before the
first context exists, which turns it on for every web view, the main
window's included.

| Where | Result |
|---|---|
| Dev container | Not testable. WebKit skips bwrap where `/run/.containerenv` exists (`get_sandbox_enabled` = 1, but the web process ran unconfined). With that file hidden, bwrap couldn't mount `/proc` (podman forbids it), and **the whole app crashed** (`Trace/breakpoint trap`): WebKit doesn't fall back to running unsandboxed |
| Host (Arch, WebKitGTK 2.52), sandbox off | Web process shares every namespace with the app, no seccomp, and sees `HOME` (the canary file), `/home`, `/run/user` and all of `/tmp` |
| Host, `WEBKIT_FORCE_SANDBOX=1` | Web process runs under bwrap: **its own user, pid, network and mount namespaces**, `NoNewPrivs`, a seccomp filter. **`HOME` (the canary), `/home` and `/run/user` are gone**, and `/tmp` holds only what WebKit binds in. The viewer works as before (both pages, text layer), and every network and IPC check gives the same result as without it |

The network process stays outside the sandbox (WebKit's design): it is the
one that makes requests, so it is the one the proxy and filter govern. With
the web process in its own network namespace, a compromised viewer can't
open a socket of its own; it can only ask the network process, which goes
through the filter and the tripwire.

**What this means for a design**:
- **Turning it on is all-or-nothing through Tauri**: `WEBKIT_FORCE_SANDBOX`
  in `main()` before Tauri starts sandboxes the main window too. That needs
  a full E2E run on a host (the container can't run it). The alternative is
  a preview web view HoploDex creates itself, outside Tauri, on its own
  context with the sandbox on (and with no Tauri IPC in it at all). Not
  tried.
- **It must never be forced where bwrap can't run**, or HoploDex crashes at
  its first web view. That rules out forcing it blindly: HoploDex would have
  to check first that bwrap works (it can't in containers, inside Flatpak,
  which has its own sandbox, or, possibly, on Ubuntu 24.04 and later, whose
  AppArmor restricts unprivileged user namespaces; not verified). And it
  needs a decision for when it can't: preview with the other layers only,
  or no in-app preview on that computer.

## What a design on this needs

1. **An app ACL manifest** (`tauri_build::AppManifest` in `build.rs`) so
   every HoploDex command needs a permission, granted only to the main
   window. Without it the preview window's top frame can call any command.
   This is hardening for the whole app, not just the preview.
2. **The preview web view**: `incognito`, the content filter, `proxy_url`
   to a tripwire port HoploDex holds, WebRTC off,
   `on_navigation` allowing only the preview's own URL and the viewer's,
   `on_new_window` denying, and `on_download` asking with the OS's save
   dialog (so Save writes only where the user picks) or denying.
3. **A frame script** (`initialization_script_for_all_frames`) that turns off
   PDF.js scripting and XFA at `webviewerloaded`.
4. **Serve from memory** through a custom protocol with
   `Cache-Control: no-store`; a one-time path per preview so nothing else can
   fetch it.

5. **WebKit's sandbox, where it can run** (Linux; see above), with a check
   that bwrap works before turning it on.

TIFF (which no web view but WebKit on macOS shows) and text keep the plan's
own decoders.

## What the macOS and Windows sessions need to check

Run `cargo run --example pdf_spike` (no Xvfb; click the link and Save by
hand) with `SPIKE_PROXY=own SPIKE_ALLOW_BLOB=1`, and for each:
- Does the viewer show a PDF from a custom protocol? (Windows serves it as
  `http://hdpreview.localhost/`.) What URL and origin does the viewer frame
  report, and are any extra navigations needed (allow and log them)?
- Can the viewer's frame reach IPC (`internals`, `postMessage`, `rawIpc`)?
  On Windows, check whether WebView2's PDF viewer is a script context the
  frame script reaches at all.
- Does `proxy_url` send everything to the tripwire, `localhost` included?
  (macOS needs Tauri's `macos-proxy` feature and macOS 14; WebView2 takes it
  as a browser argument.) The content filter (`SPIKE_BLOCKER`) is
  Linux-only in the spike; the macOS and Windows equivalents need adding.
- Does anything land on disk? Look for the marker under the app's data and
  cache dirs (WebView2's user data folder especially) and the temp dir.
- Does the PDF's JavaScript run (an alert reading "HD SPIKE PDF JAVASCRIPT
  RAN"), and if so, can it be turned off?
- Where does Save go, and does `on_download` fire?
- The app manifest result is the same on every OS (Tauri's own code), so it
  needs no retest.
