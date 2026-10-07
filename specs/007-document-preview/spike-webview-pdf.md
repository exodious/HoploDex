# Spike: the web view's own PDF viewer (2026-10-03)

**Question**: can HoploDex preview a PDF with the PDF viewer the web view
already has, instead of bundling or linking a PDF engine (research.md §3 and
§6 chose PDFium from pdfium-binaries; the owner doesn't want a binary someone
else built)? The answer must keep issue #13's findings closed and write
nothing to disk unless the user asks for it.

**Answer on Linux: yes**, with four measures (below). **On macOS: yes**,
with the same measures and two more: WebKit's PDF viewer there offers "Open
with Preview" (in a hover toolbar and in its context menu), which writes the
decrypted document to a temporary file with one click, so both must be
turned off. **On Windows: yes**, with the same measures done WebView2's
way, and two differences that shape a design: the content filter there
(`WebResourceRequested`) sees only the top frame, not Edge's viewer, so the
proxy is the layer that covers the viewer and must take loopback too; and
the proxy and WebRTC switches are browser arguments, which every web view
sharing a user data folder must agree on, so the preview needs a user data
folder of its own. Edge's viewer has nothing like Open in Preview: its Save
goes through the OS's Save As dialog, then `on_download`.

**Re-run (2026-10-06, at 2bd34b59)** after the Windows work changed the
shared probes and exit timing: on Linux (bare, proxy only, filter only, and
everything with Save allowed) and on macOS 26.6.2 (bare, HUD and menu off,
proxy only, filter only, WebRTC off, and everything), every result matched
the tables below. The macOS run added the two details in its Open in
Preview row (a copy per click, and Recent Documents entries). The app
manifest check wasn't repeated; nothing it depends on changed.

## What each web view has

| OS | Web view | Built-in PDF viewer | Tested |
|---|---|---|---|
| Linux | WebKitGTK | PDF.js, bundled in WebKitGTK since 2.40 (`webkit-pdfjs-viewer://pdfjs/web/viewer.html`). In Debian's 2.54 (which the AppImage bundles) and Arch's 2.52 | Yes, Debian 2.54 in the dev container |
| macOS | WKWebView | WebKit's PDF plugin ("Unified PDF Viewer", on the OS's PDFKit), in the main frame | Yes, macOS 26.6 in a tart VM (2026-10-06) |
| Windows | WebView2 | Edge's viewer (PDFium in Chromium's sandbox), updated with the WebView2 runtime | Yes, runtime 154.0.4258.62 on Windows Server 2025 (2026-10-06) |

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

On macOS, `src-tauri/examples/pdf_spike_mac.sh` runs it in a macOS VM (tart,
macOS 26.6.2) over SSH: it starts the spike through the VM's Terminal, since
an app started over SSH gets no window, and posts real mouse input and takes
screenshots with `examples/pdf_spike_input.swift` (CGEvent). It hovers for
WebKit's PDF toolbar and clicks its buttons, right-clicks for the context
menu and clicks Open with Preview, clicks the link, and reads the window's
accessibility tree. The disk check covers the user's temp and cache folders
(`/var/folders/…`), `/tmp`, `/private/var/tmp` and, in the VM, all of `HOME`.

On Windows, `src-tauri/examples/pdf_spike_win.ps1` runs it in the signed-in
desktop session of a test machine, with a fresh WebView2 user data folder
(`WEBVIEW2_USER_DATA_FOLDER`) in the run's temp folder. It posts real mouse
and keyboard input and takes screenshots with `examples/pdf_spike_input.ps1`
(`SendInput`, `CopyFromScreen`): it clicks the toolbar's settings, Save
(typing a path into the Save As dialog that opens) and Print, presses
Ctrl+S, right-clicks for the context menu, clicks the link, and reads the
window's UI Automation tree. Edge's viewer runs in frames no page script
reaches, so the spike attaches to them through the DevTools protocol
(`CallDevToolsProtocolMethodForSession`) and runs the same reach probe there,
as code a PDF that exploited the viewer could run. It also logs every
request WebView2's `WebResourceRequested` sees, script dialogs, context
menus (with their items), permission requests, external URI schemes, Save
As and process failures. The disk check covers `%LOCALAPPDATA%` (the temp
folder and the WebView2 folder are in it) and `%APPDATA%`, for the marker as
ASCII and as UTF-16. `examples/pdf_spike.rs` needed `build.rs` to link
tauri-build's Windows resource into examples too: without its Common
Controls manifest, Windows refuses to start them
(`STATUS_ENTRYPOINT_NOT_FOUND`).

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

## Results (macOS 26.6, WKWebView)

| Check | Result |
|---|---|
| Renders | Yes: both pages, scrolling, zoom. **No PDF.js and no child frame**: the top frame itself is a plugin document (`document.contentType` `application/pdf`, an `<embed>` in the body) at `hdpreview://localhost/doc.pdf` |
| Accessibility | The page text and the link are in the window's accessibility tree (`AXStaticText "HDSPIKEMARKER page one"`, `AXLink`), so VoiceOver can read it |
| Where the viewer runs | WebKit's native PDF code, in the same web process as the top frame, which has Tauri's IPC (`__TAURI_INTERNALS__`, `webkit.messageHandlers.ipc`). On Linux PDF.js runs in a cross-origin frame without it. A PDF that exploited the viewer here would run where the IPC is, so the app ACL manifest (measure 1) is what stops it |
| Top frame → app commands | **Allowed without an app manifest** (`spike_secret` returned), plugin commands refused: as on Linux |
| Top frame → raw `ipc://` fetch with a guessed key | No answer (timed out) without the filter; refused by the filter |
| Toolbar on hover (WebKit's "PDF HUD") | Zoom out, zoom in, **Open in Preview**, Download |
| HUD's Download | Nothing: no `on_download`, no file. WebKit hands it to a private UI-delegate method wry doesn't implement |
| **HUD's Open in Preview, and the context menu's Open with Preview** | **Writes the decrypted PDF** to `$TMPDIR/WebKitPDFs-XXXXXX/` (mode 0400) and opens it in Preview: the HUD's as `doc.pdf`, the menu's as a second, random-prefixed copy (`47e6Y8-doc.pdf`), so each click leaves a copy of its own. No Tauri handler sees it (no navigation, no download event). **The copy stays** after HoploDex quits, normally or killed; macOS removes it only with the temp folder. **Preview also adds both copies to Recent Documents** (`com.apple.LSSharedFileList.RecentDocuments.sfl4` and Preview's own `com.apple.preview.sfl4` under `~/Library/Application Support/com.apple.sharedfilelist/`), and those entries, which would carry the document's file name, stay in File > Open Recent, the Dock and Spotlight after the copy is gone |
| Showing the PDF in an `<iframe>` or `<embed>` instead | Same toolbar and menu. And the iframe is same-origin with the page, so its probe reached the parent's `__TAURI_INTERNALS__`. No help |
| `PDFPluginHUDEnabled` off | No toolbar. Set through WebKit SPI: `+[WKPreferences _features]`, then `-[WKPreferences _setEnabled:forFeature:]`. The context menu still has Open with Preview |
| `contextmenu` cancelled (a frame script's `preventDefault`) | No context menu. The PDF can't run page script to undo it |
| Both | Hovering and right-clicking show nothing; clicking where Open in Preview was writes nothing |
| Clicking the PDF's link | A top-level navigation to `http://example.com/link-target`, refused by `on_navigation` |
| The PDF's JavaScript | No alert in any run, no `window.alert` call. WebKit's plugin has no switch for it, so nothing to turn off. Not proof it can't run |
| Decrypted content on disk | **None**, with the HUD off and the menu cancelled. Files written: the compiled content filter (rules only) and the GPU process's Metal shader lists |
| Print | Not tried |

## Results (Windows Server 2025, WebView2 154)

| Check | Result |
|---|---|
| Renders | Yes: both pages, page count, zoom, fit, rotate, two-page view, find, Print, Save, full screen, a settings menu (Pin toolbar, View document properties). The address is `http://hdpreview.localhost/doc.pdf`, served by the custom protocol; no extra navigation needed |
| Accessibility | The page text (`Text "HDSPIKEMARKER page one"`, `"Second page HDSPIKEMARKER"`) and a link are in the window's UI Automation tree, so Narrator can read it |
| Where the viewer runs | Three renderer processes. The top frame (ours, with Tauri's IPC) holds only an `<embed>`. Edge's viewer is `chrome-extension://mhjfbmdgcfjbbpaeojofohoefgiehjai/edge_pdf/index.html`, a DevTools target of type `webview` in a process of its own. Inside it, a frame at `http://hdpreview.localhost/doc.pdf` (cross-origin to the extension, its own process again) is where the PDF renders. The frame script reached neither: it ran only in the top frame and an empty `about:blank` frame of the `<embed>` |
| Viewer frames → Tauri IPC | **Blocked.** Neither has `__TAURI_INTERNALS__` or `chrome.webview`, and both are cross-origin to their parent (`SecurityError`). A `postMessage` with no key does nothing. Their requests to `http://ipc.localhost/` never reach Tauri: `WebResourceRequested` doesn't see them, so they go to the network (to the proxy, with loopback in it) |
| What Edge's viewer frame has | Private extension APIs: `chrome.edgePdfPrivate`, `pdfViewerPrivate`, `mimeHandlerPrivate`, `fileSystem`, `tabs`, `windows`, `management`, `runtime` and more. It's Edge's own code, not the PDF's; the PDF's frame has only `loadTimes`, `csi`, `app` and `metricsPrivate` |
| Top frame → app commands | **Allowed without an app manifest** (`spike_secret` returned), plugin commands refused: as on Linux and macOS |
| Toolbar Save, and the context menu's Save | **The OS's Save As dialog**, through WebView2's `SaveAsUIShowing` (`application/pdf`), which HoploDex can cancel. After the user picks a place, `on_download` fires with that place as the destination, and can change or refuse it; the document is fetched again from the custom protocol. Nothing is written before the user chooses |
| Ctrl+S | Nothing, with Save shown or hidden (`SendInput`, after clicking the page) |
| Print | Edge's print preview, inside the window; this machine's default printer was Microsoft Print to PDF, which asks where to save. Not printed |
| Context menu | Save, Print, Rotate clockwise and counterclockwise, Inspect (DevTools, on in a debug build); Copy too when text is selected. No "open with", web search, read aloud or Copilot. `ContextMenuRequested` fires for it, with the viewer's frame as the target, so HoploDex can drop items or the whole menu; `AreDefaultContextMenusEnabled(false)` removes it |
| Anything like Open in Preview | **No.** No toolbar button, menu item or settings entry hands the document to another program or a temporary file |
| `HiddenPdfToolbarItems` | Hides the named buttons (tried Save, Save As, Print); the toolbar closes up, so the others move |
| Clicking the PDF's link | A top-level navigation to `http://example.com/link-target`, refused by `on_navigation`. **But Edge's automatic HTTPS still sent `https://example.com/link-target`** (a document request from the top frame, seen by `WebResourceRequested`; with the proxy it went to the tripwire as `CONNECT example.com:443`), then showed "This site doesn't support a secure connection". Its Continue to site reloaded the PDF. The same with a persistent profile. The content filter refuses that request |
| The PDF's JavaScript | No script dialog in any run (`ScriptDialogOpening`, with WebView2's own dialogs off). WebView2 has no switch for it. Not proof it can't run |
| WebView2's own requests | `CONNECT config.edge.skype.com:443` and `edge.microsoft.com:443` from the browser process, not the page; the proxy sends them to the tripwire like anything else |
| Decrypted content on disk | **None**, InPrivate (`incognito`) or with a persistent profile, with `Cache-Control: no-store`. Files written: all inside the WebView2 user data folder (profile files, GPU shader caches, Crashpad, variations seed, component caches); InPrivate still writes them |
| Main window and preview in one user data folder | **The preview never loads.** With different browser arguments (the preview's proxy and WebRTC switch), its `build()` returns Ok but its web view shows nothing and navigates nowhere. With a folder each, both work |

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

### Network on macOS

The same attempts, from the top frame (where the PDF is), with Tauri's
`macos-proxy` feature for `proxy_url`:

| Variant | What got out |
|---|---|
| No proxy, no filter | The local service got the beacon, the WebSocket, both fetches and the image; example.com was reached; **WebRTC is present** and sent STUN packets to the local UDP port |
| Proxy to the tripwire | **The local service still got everything: `127.0.0.1` and `localhost` bypass the proxy.** Only the outside went to the tripwire, as `CONNECT example.com:80`, `CONNECT dns-leak-fetch.invalid:80` and so on (host and port only). WebRTC still reached the UDP port |
| Content filter only (a `WKContentRuleList`, the same rules) | Nothing over HTTP or WebSocket, `sendBeacon` refused; **WebRTC still reached the UDP port** (the filter doesn't cover it) |
| WebRTC off (`-[WKPreferences _setPeerConnectionEnabled:]`, SPI) | `RTCPeerConnection` absent; everything else as with no filter |
| Filter, proxy and WebRTC off | Nothing got out, and the tripwire saw nothing |

Other findings:
- **The proxy needs macOS 14**, and wry links its Network.framework calls
  strongly, so turning on `macos-proxy` makes the whole app need macOS 14.
  That's no obstacle now that the minimum is macOS 26 (plan.md, raised
  2026-10-06). But it doesn't cover the computer's own services, so on
  macOS the filter is the layer that matters; the tripwire adds only a log
  of outside requests that get past it.
- **Taking over the tripwire's port**: while the spike held `127.0.0.1`,
  binding `127.0.0.1` again failed, plain or with `SO_REUSEADDR` +
  `SO_REUSEPORT`, but **binding `0.0.0.0` on the same port succeeded**, plain
  too (Linux refused it). The proxied connections still all went to the
  tripwire, the more specific address; the other socket got none.
- DNS lookups weren't logged (no `LD_PRELOAD` equivalent reaches WebKit's
  network process). With the proxy, names went to it in `CONNECT` lines
  rather than being looked up; with the filter, the loads stopped first.
- The compiled filter is written to the store directory given to
  `WKContentRuleListStore` (rules only).

### Network on Windows

The same attempts, from the top frame and, through the DevTools protocol,
from Edge's viewer frame and the PDF's frame. The proxy is wry's
`--proxy-server` browser argument; the spike writes the arguments itself
(wry drops its own, and the proxy, once an app gives any).

| Variant | What got out |
|---|---|
| No filter, proxy to the tripwire | **The local service got everything from all three frames**: fetches, the image, the beacon, the WebSocket, and WebRTC's STUN packets: Chromium sends loopback around the proxy. The outside went to the tripwire (`GET http://example.com/…`, `CONNECT dns-leak-ws.invalid:80`) |
| Proxy with `--proxy-bypass-list=<-loopback>` | Every TCP attempt from every frame went to the tripwire, `127.0.0.1` and `localhost` included (`GET http://127.0.0.1:…/fetch-loop`, `CONNECT 127.0.0.1:…` for the WebSocket, the viewer frames' `POST http://ipc.localhost/spike_secret`). Only WebRTC's UDP still reached the local service |
| Content filter only (`WebResourceRequested` on `*`, refusing all but the preview's own URLs) | The top frame's fetches, image and beacon refused, and the link's HTTPS request. **Not its WebSocket, not WebRTC, and nothing from the two viewer frames**, which `WebResourceRequested` never sees: the local service got all of theirs |
| `--webrtc-ip-handling-policy=disable_non_proxied_udp` | No STUN packets from any frame; the top frame and the PDF's frame gather no candidates. Edge's viewer frame still lists the computer's LAN address as a candidate, but sent nothing. `--force-webrtc-ip-handling-policy` and `--enforce-webrtc-ip-permission-check` changed nothing |
| Filter, proxy with loopback, WebRTC switch | Nothing reached the local service. The tripwire saw what the filter can't see: the viewer frames' attempts and the top frame's WebSocket |

Other findings:
- **Taking over the tripwire's port**: while the spike held `127.0.0.1`,
  binding `127.0.0.1` again failed, plain (10048) or with `SO_REUSEADDR`
  (10013), whether or not the tripwire set `SO_EXCLUSIVEADDRUSE`. **Binding
  `0.0.0.0` on the same port succeeded**, plain or with `SO_REUSEADDR`,
  with `SO_EXCLUSIVEADDRUSE` too. As on macOS, the proxied connections all
  went to the tripwire, the more specific address; the other socket got
  none.
- `WebResourceRequested` sees the top frame's fetches (context 7), images
  (3), beacons (14), documents (1) and Tauri's own IPC requests, so a
  filter that refuses everything else also refuses the IPC, which a preview
  doesn't need.
- DNS lookups weren't logged (no `LD_PRELOAD` equivalent). With the proxy,
  names went to it in requests and `CONNECT` lines rather than being looked
  up.

**So the network block is three layers, each tested on its own**: the
content filter (no port involved), the proxy to a port HoploDex holds for as
long as a preview exists (a tripwire that logs anything getting past the
filter), and the navigation handler for top-level navigations. On macOS the
filter is a `WKContentRuleList`, WebRTC must be turned off as well, and the
proxy is only the tripwire (above). On Windows it's the other way round: the
filter (`WebResourceRequested`) covers only the top frame, so the proxy,
with `<-loopback>`, is the layer that covers Edge's viewer, and WebRTC needs
`--webrtc-ip-handling-policy=disable_non_proxied_udp`. `SO_EXCLUSIVEADDRUSE`
adds nothing for a `127.0.0.1` port there.

A test that keeps it blocked: open a PDF in the preview, have a frame script
make every attempt above against a local test service, and fail if the
service or the tripwire sees a connection or the DNS log shows a lookup.

## WebKit's sandbox (Linux)

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

## If Open in Preview gets through (macOS, 2026-10-06)

The HUD and the context menu are turned off through SPI and a frame script.
An OS update could leave the switch in place but stop it working, which a
startup check wouldn't notice. These runs (`pdf_spike_mac.sh` with
`SPIKE_MAC_FLOW=names|close|watch-hud|watch-menu`, commit 76372ece) left the
HUD and menu on, so the copy was written, and tested what HoploDex could do
about it afterwards.

| Check | Result |
|---|---|
| The copy's name | **The last segment of the URL path, nothing else.** Served at `hdpreview://localhost/<token>/document.pdf`, the copies were `document.pdf` (HUD) and `QJ12Py-document.pdf` (menu), and Preview's window title and Open Recent showed those. The token appeared nowhere. A `Content-Disposition` filename and the PDF's `/Title` were both ignored (not in the names, the titles or either sfl4 list) |
| Deleting the copies when the preview window closes | The delete works (dir 0700, files 0400, owned by the user; `Destroyed` fires 13 ms after `close()`). **But Preview keeps the document**: its windows keep showing both pages, with no error, because it holds the files memory-mapped. The unlinked data stays on disk, readable through Preview, until Preview lets go. Deleting only unlinks |
| Recent Documents after the delete | Open Recent stops listing them, but **both sfl4 files keep the entries** (the `WebKitPDFs-*` path and file name), through a quit and a relaunch |
| Other records of the path | **The copy's full path is also kept in `~/Library/DuetExpertCenter/_ATXDataStore.db-wal`** (Siri suggestions) **and `~/Library/Biome/streams/restricted/App.DocumentInteraction/`** (with Preview as the app), for every copy in every run, earlier sessions' included. Name only, no content, and nothing HoploDex can reasonably clear |
| Content after the delete | No file written during the run held the marker (HOME, the temp folders, Preview's container). Preview's container changed only its view-state plist (volume and inode, no name). The QuickLook thumbnail cache didn't change. Spotlight is off on the VM's volume, so it wasn't tested; a Mac with indexing on still needs to be checked |
| Watching `$TMPDIR` for a new `WebKitPDFs-*` and deleting it at once (1 ms poll) | Seen 4 ms after the HUD click (373 ms after the menu click, mostly the menu closing). At that moment the folder held WebKit's atomic-write temp file (`document.pdf.sb-…`, full size, **mode 0644** until the rename; the 0700 folder still keeps other users out). **The delete beat Preview every time**: Preview never launched or launched to nothing, no error was shown, and nothing was added to Recents (so nothing went to Biome or Duet either). WebKit only logged "Cannot create PDF file in the temporary directory" |
| The same, deleting 50 ms late | The rename had happened and Preview launched, but the delete still won: no window, nothing in Recents |
| The same, 500 ms late | Preview had already opened it and Recents had the entry, as with deleting at close |

So if the SPI stops working: **a generic last path segment** keeps the
document's name out of every record; **a watch that deletes within tens of
milliseconds** stops Preview getting the file at all, and so stops the
Recents, Biome and Duet records; and **a sweep at close** removes the file,
but not Preview's mapped copy while Preview stays open, and not the records.
The watch must react well inside 50–500 ms; FSEvents' latency hasn't been
measured against that.

## What a design on this needs

1. **An app ACL manifest** (`tauri_build::AppManifest` in `build.rs`) so
   every HoploDex command needs a permission, granted only to the main
   window. Without it the preview window's top frame can call any command.
   This is hardening for the whole app, not just the preview.
2. **The preview web view**: `incognito`, the content filter, `proxy_url`
   to a tripwire port HoploDex holds (on macOS through `macos-proxy`),
   WebRTC off (on macOS through SPI),
   `on_navigation` allowing only the preview's own URL and the viewer's,
   `on_new_window` denying, and `on_download` asking with the OS's save
   dialog (so Save writes only where the user picks) or denying. On Windows
   the filter is a `WebResourceRequested` handler, and the proxy, the
   loopback rule (`--proxy-bypass-list=<-loopback>`) and the WebRTC switch
   (`--webrtc-ip-handling-policy=disable_non_proxied_udp`) are browser
   arguments, written in full with wry's defaults
   (`additional_browser_args`), since wry adds its own only when an app
   gives none.
3. **A frame script** (`initialization_script_for_all_frames`) that turns off
   PDF.js scripting and XFA at `webviewerloaded` (Linux), and cancels
   `contextmenu` (macOS, where the menu has Open with Preview). On Windows
   it reaches neither viewer frame; the context menu is WebView2's
   (`ContextMenuRequested` or `AreDefaultContextMenusEnabled`).
4. **Serve from memory** through a custom protocol with
   `Cache-Control: no-store`; a one-time path per preview so nothing else can
   fetch it.

5. **WebKit's sandbox, where it can run** (Linux; see above), with a check
   that bwrap works before turning it on.
6. **On macOS, WebKit's PDF toolbar off** (`PDFPluginHUDEnabled`, through
   SPI), or its Open in Preview writes the decrypted document to `$TMPDIR`.
   The HUD and WebRTC switches are WebKit SPI, which an OS update can rename
   or remove: HoploDex must check at startup that both exist and are off,
   and show no in-app preview if not, and a macOS test must hover,
   right-click and click Open in Preview and fail if any `WebKitPDFs-*`
   copy appears. As backstops (see "If Open in Preview gets through"):
   serve the preview at a fixed generic name (`<token>/document.pdf`, since
   the copy is named after the last path segment); while a preview is
   open, watch `$TMPDIR` for a new `WebKitPDFs-*` folder, delete it within
   tens of milliseconds, close the preview and turn in-app preview off on
   that computer; and sweep folders that appeared during a preview when it
   closes, at startup and on signals, deleting only those.
7. **On Windows, a user data folder of the preview's own.** Browser
   arguments belong to the browser process, which every web view sharing a
   user data folder shares, so a preview with the proxy and the WebRTC
   switch next to a main window without them never loads. A folder of its
   own (`data_directory`) gives it a browser process of its own; it must be
   one HoploDex controls and clears like its other caches, and E2E's
   `WEBVIEW2_USER_DATA_FOLDER` overrides it, so an E2E run of the preview
   needs a way to set it too. The proxy is the layer that keeps Edge's
   viewer off the network and the computer's own services, so a Windows
   test must run the reach probe in the viewer's frames (through the
   DevTools protocol, as the spike does) and fail if the local service sees
   anything. Edge's automatic HTTPS means a clicked link sends a request
   even though `on_navigation` refuses the navigation; the filter refuses
   it.

TIFF (which no web view but WebKit on macOS shows) and text keep the plan's
own decoders.