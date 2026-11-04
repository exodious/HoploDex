//! The passphrase dialog on Windows: a window of Win32 controls, owned by the
//! main window, which is disabled while it is up, as a modal dialog's owner
//! is. Each field is an edit control with `ES_PASSWORD`. The dialog runs on a
//! thread of its own with its own message loop, so it never waits on, or
//! re-enters, the app's event loop; `IsDialogMessageW` gives it a dialog's
//! keys (Tab, Enter on the default button, Escape for Cancel).
//!
//! Each field's text is read into a buffer that is wiped straight after;
//! what the edit control keeps of it is the OS's own input path.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, Once};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, CreateFontIndirectW, DT_CALCRECT, DT_WORDBREAK, DeleteObject, DrawTextW, GetDC,
    GetSysColorBrush, HDC, HFONT, InvalidateRect, ReleaseDC, SelectObject, SetBkMode, SetTextColor,
    TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{
    BST_CHECKED, EM_SETPASSWORDCHAR, ICC_PROGRESS_CLASS, INITCOMMONCONTROLSEX,
    InitCommonControlsEx, PBM_SETPOS, PBM_SETRANGE32,
};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, BM_GETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_MULTILINE,
    BN_CLICKED, BN_SETFOCUS, BS_NOTIFY, BS_PUSHBUTTON, CREATESTRUCTW, EN_SETFOCUS, RemovePropW,
    SetPropW, WA_INACTIVE, WM_ACTIVATE, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW,
    DC_HASDEFID, DM_GETDEFID, DefWindowProcW, DestroyWindow, DispatchMessageW, EN_CHANGE,
    ES_AUTOHSCROLL, ES_PASSWORD, GWLP_USERDATA, GetMessageW, GetSystemMetrics, GetWindowLongPtrW,
    GetWindowRect, GetWindowTextLengthW, GetWindowTextW, HMENU, IDC_ARROW, IDCANCEL, IDOK,
    IsDialogMessageW, KillTimer, LoadCursorW, MSG, NONCLIENTMETRICSW, PostMessageW,
    PostQuitMessage, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SPI_GETNONCLIENTMETRICS, SW_HIDE,
    SW_SHOW, SWP_NOZORDER, SendMessageW, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    SetWindowPos, SetWindowTextW, ShowWindow, SystemParametersInfoW, TranslateMessage, WM_APP,
    WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORSTATIC, WM_DESTROY, WM_SETFONT, WM_TIMER,
    WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_POPUP,
    WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use zeroize::Zeroizing;

use super::{Controller, View};
use crate::services::passphrase::Passphrase;
use crate::services::passphrase_dialog::checks::{PAUSE, Texts};
use crate::services::passphrase_dialog::strength::ADVICE;
use crate::services::passphrase_dialog::{
    ACKNOWLEDGEMENT, Answer, DialogWindow, Event, Field, Request, Retry,
};

const CLASS: &str = "HoploDexPassphraseDialog";
/// `retry` and `close`, posted from the caller's thread.
const WM_RETRY: u32 = WM_APP + 1;
const WM_END: u32 = WM_APP + 2;
const PAUSE_TIMER: usize = 1;
/// The window property that names the focused control (`State::focused`).
static FOCUS_PROP: std::sync::LazyLock<Vec<u16>> = std::sync::LazyLock::new(|| wide("HoploDexFocus"));
const ID_SHOW: i32 = 100;
const ID_ACKNOWLEDGEMENT: i32 = 101;
const ID_FIRST_FIELD: i32 = 200;
/// How the edit controls hide what is typed: a black circle.
const HIDDEN_CHAR: usize = 0x25CF;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(super) fn open<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
    window: tauri::Window<R>,
    request: Request,
    events: Sender<Event>,
) -> Result<Box<dyn DialogWindow>, String> {
    let owner = window.hwnd().map_err(|err| format!("no HWND: {err}"))?.0 as isize;
    let retry = Arc::new(Mutex::new(None));
    let (made, result) = mpsc::channel();
    let slot = Arc::clone(&retry);
    std::thread::Builder::new()
        .name("passphrase-dialog".to_owned())
        .spawn(move || run(owner, request, events, slot, made))
        .map_err(|err| format!("no thread for the dialog: {err}"))?;
    let hwnd = result.recv().map_err(|_| "the dialog's thread ended".to_owned())??;
    Ok(Box::new(Win32Window { hwnd, retry }))
}

/// The caller's handle: posts to the dialog's thread.
struct Win32Window {
    hwnd: isize,
    retry: Arc<Mutex<Option<Retry>>>,
}

impl DialogWindow for Win32Window {
    fn retry(&self, retry: &Retry) {
        *self.retry.lock().expect("retry mutex poisoned") = Some(retry.clone());
        // SAFETY: posting to a window handle is safe whatever its state.
        unsafe { PostMessageW(self.hwnd as HWND, WM_RETRY, 0, 0) };
    }

    fn close(&self) {
        // SAFETY: as above.
        unsafe { PostMessageW(self.hwnd as HWND, WM_END, 0, 0) };
    }
}

/// One field: its edit control and the line under it.
struct Row {
    field: Field,
    edit: HWND,
    line: HWND,
}

/// One dialog's controls and state, owned by its thread.
struct State {
    hwnd: Cell<HWND>,
    owner: HWND,
    request: Request,
    rows: RefCell<Vec<Row>>,
    meter: Cell<HWND>,
    meter_label: Cell<HWND>,
    suggestion: Cell<HWND>,
    acknowledgement: Cell<HWND>,
    show: Cell<HWND>,
    busy_label: Cell<HWND>,
    primary: Cell<HWND>,
    cancel: Cell<HWND>,
    font: Cell<HFONT>,
    /// The control that last had the focus, put back when the window is
    /// active again.
    last_focus: Cell<HWND>,
    events: Sender<Event>,
    retry: Arc<Mutex<Option<Retry>>>,
    controller: RefCell<Controller>,
    busy: Cell<bool>,
    ended: Cell<bool>,
}

fn register_class() {
    static REGISTERED: Once = Once::new();
    REGISTERED.call_once(|| {
        let name = wide(CLASS);
        // SAFETY: a class registered once for this process, with a static
        // name buffer (leaked) and this module's window procedure.
        unsafe {
            let class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: GetModuleHandleW(std::ptr::null()),
                hIcon: std::ptr::null_mut(),
                hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
                lpszMenuName: std::ptr::null(),
                lpszClassName: name.leak().as_ptr(),
            };
            RegisterClassW(&class);
            let common = INITCOMMONCONTROLSEX {
                dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_PROGRESS_CLASS,
            };
            InitCommonControlsEx(&common);
        }
    });
}

fn run(
    owner: isize,
    request: Request,
    events: Sender<Event>,
    retry: Arc<Mutex<Option<Retry>>>,
    made: Sender<Result<isize, String>>,
) {
    register_class();
    let shape = request.shape;
    let state = Box::new(State {
        hwnd: Cell::new(std::ptr::null_mut()),
        owner: owner as HWND,
        request,
        rows: RefCell::new(Vec::new()),
        meter: Cell::new(std::ptr::null_mut()),
        meter_label: Cell::new(std::ptr::null_mut()),
        suggestion: Cell::new(std::ptr::null_mut()),
        acknowledgement: Cell::new(std::ptr::null_mut()),
        show: Cell::new(std::ptr::null_mut()),
        busy_label: Cell::new(std::ptr::null_mut()),
        primary: Cell::new(std::ptr::null_mut()),
        cancel: Cell::new(std::ptr::null_mut()),
        font: Cell::new(std::ptr::null_mut()),
        last_focus: Cell::new(std::ptr::null_mut()),
        events,
        retry,
        controller: RefCell::new(Controller::new(shape)),
        busy: Cell::new(false),
        ended: Cell::new(false),
    });
    let title = wide(&state.request.title);
    let class = wide(CLASS);
    let state = Box::into_raw(state);
    // SAFETY: the window gets `state` through CREATESTRUCTW and keeps it in
    // GWLP_USERDATA until WM_DESTROY, after which it is freed below.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            400,
            300,
            owner as HWND,
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            state.cast::<c_void>(),
        )
    };
    if hwnd.is_null() {
        // SAFETY: never handed to a window.
        drop(unsafe { Box::from_raw(state) });
        let _ = made.send(Err("could not create the dialog".to_owned()));
        return;
    }
    // SAFETY: the state lives until this function frees it, after the loop.
    let st = unsafe { &*state };
    st.build();
    // SAFETY: plain Win32 calls on windows this thread made, and on the
    // owner, which a modal dialog disables while it is up.
    unsafe {
        EnableWindow(st.owner, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        if let Some(first) = st.rows.borrow().first() {
            SetFocus(first.edit);
        }
    }
    let _ = made.send(Ok(hwnd as isize));

    let mut msg: MSG = unsafe { std::mem::zeroed() };
    // SAFETY: this thread's own message loop.
    unsafe {
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        DeleteObject(st.font.get());
        drop(Box::from_raw(state));
    }
}

fn state_of(hwnd: HWND) -> Option<&'static State> {
    // SAFETY: GWLP_USERDATA holds the `State` WM_CREATE stored, or 0.
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const State;
    // SAFETY: the state outlives every message to the window (it is freed
    // only after the message loop ends, past WM_DESTROY).
    unsafe { ptr.as_ref() }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CREATE {
        // SAFETY: CreateWindowExW passed the state as lpCreateParams.
        let create = unsafe { &*(lparam as *const CREATESTRUCTW) };
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize) };
        if let Some(state) = state_of(hwnd) {
            state.hwnd.set(hwnd);
        }
        return 0;
    }
    let Some(state) = state_of(hwnd) else {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    };
    match msg {
        // The default button, for Enter and its bold border.
        DM_GETDEFID => ((DC_HASDEFID as isize) << 16) | IDOK as isize,
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as i32;
            let code = ((wparam >> 16) & 0xFFFF) as u32;
            if code == EN_SETFOCUS || code == BN_SETFOCUS {
                state.focused(lparam as HWND);
                return 0;
            }
            match (id, code) {
                (IDOK, BN_CLICKED) => state.submit(),
                (IDCANCEL, BN_CLICKED) => state.cancel(),
                (ID_SHOW, BN_CLICKED) => state.toggle_show(),
                (ID_ACKNOWLEDGEMENT, BN_CLICKED) => state.changed(None),
                (_, EN_CHANGE) => {
                    let field = state
                        .rows
                        .borrow()
                        .iter()
                        .find(|r| r.edit == lparam as HWND)
                        .map(|r| r.field);
                    state.changed(field);
                }
                _ => {}
            }
            0
        }
        WM_TIMER if wparam == PAUSE_TIMER => {
            unsafe { KillTimer(hwnd, PAUSE_TIMER) };
            let view = state.with_texts(|texts| state.controller.borrow_mut().paused(texts));
            state.apply(&view);
            0
        }
        WM_CTLCOLORSTATIC => {
            let control = lparam as HWND;
            let hdc = wparam as HDC;
            let is_line = state.rows.borrow().iter().any(|r| r.line == control);
            unsafe {
                SetBkMode(hdc, TRANSPARENT as i32);
                if is_line {
                    // The error red of Windows' own dialogs.
                    SetTextColor(hdc, 0x0000_00C4);
                }
                GetSysColorBrush(COLOR_BTNFACE) as LRESULT
            }
        }
        // As a dialog does: back to the field that had the focus, not the
        // frame, when the window is active again.
        WM_ACTIVATE => {
            if (wparam & 0xFFFF) as u32 != WA_INACTIVE {
                let last = state.last_focus.get();
                let first = state.rows.borrow().first().map(|row| row.edit);
                if let Some(control) = (!last.is_null()).then_some(last).or(first) {
                    unsafe { SetFocus(control) };
                }
            }
            0
        }
        WM_CLOSE => {
            state.cancel();
            0
        }
        WM_RETRY => {
            if let Some(retry) = state.retry.lock().expect("retry mutex poisoned").take() {
                state.retry(&retry);
            }
            0
        }
        WM_END => {
            state.end();
            0
        }
        WM_DESTROY => {
            unsafe { RemovePropW(hwnd, FOCUS_PROP.as_ptr()) };
            state.send_end(Event::Cancelled);
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// What `edit` holds, copied once into a wiped buffer.
fn text_of(edit: HWND) -> Zeroizing<String> {
    // SAFETY: reads at most `units.len()` UTF-16 units into the buffer.
    let units = unsafe {
        let len = GetWindowTextLengthW(edit).max(0) as usize;
        let mut units = Zeroizing::new(vec![0u16; len + 1]);
        let read = GetWindowTextW(edit, units.as_mut_ptr(), units.len() as i32).max(0) as usize;
        units.truncate(read);
        units
    };
    let mut out = Zeroizing::new(String::with_capacity(units.len() * 3));
    out.extend(char::decode_utf16(units.iter().copied()).map(|c| c.unwrap_or('\u{FFFD}')));
    out
}

impl State {
    /// Makes the controls and lays them out, top to bottom, at the owner's
    /// DPI, in the system's message font.
    fn build(&self) {
        let hwnd = self.hwnd.get();
        let shape = self.request.shape;
        // SAFETY: plain Win32 calls on this thread's own windows.
        unsafe {
            let dpi = GetDpiForWindow(self.owner).max(96);
            let px = |points: i32| points * dpi as i32 / 96;
            let mut metrics: NONCLIENTMETRICSW = std::mem::zeroed();
            metrics.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
            SystemParametersInfoW(
                SPI_GETNONCLIENTMETRICS,
                metrics.cbSize,
                (&mut metrics as *mut NONCLIENTMETRICSW).cast(),
                0,
            );
            metrics.lfMessageFont.lfHeight = metrics.lfMessageFont.lfHeight * dpi as i32 / 96;
            self.font.set(CreateFontIndirectW(&metrics.lfMessageFont));

            let margin = px(14);
            let width = px(340);
            let mut y = margin;
            let make = |class: &str,
                        text: &str,
                        style: u32,
                        ex: u32,
                        id: i32,
                        height: i32,
                        y: i32,
                        x: i32,
                        w: i32| {
                let control = CreateWindowExW(
                    ex,
                    wide(class).as_ptr(),
                    wide(text).as_ptr(),
                    WS_CHILD | WS_VISIBLE | style,
                    x,
                    y,
                    w,
                    height,
                    hwnd,
                    id as isize as HMENU,
                    GetModuleHandleW(std::ptr::null()),
                    std::ptr::null(),
                );
                SendMessageW(control, WM_SETFONT, self.font.get() as WPARAM, 1);
                control
            };
            let text_height = |text: &str, w: i32| -> i32 {
                let dc = GetDC(hwnd);
                let old = SelectObject(dc, self.font.get());
                let mut rect = RECT { left: 0, top: 0, right: w, bottom: 0 };
                let mut units = wide(text);
                DrawTextW(dc, units.as_mut_ptr(), -1, &mut rect, DT_CALCRECT | DT_WORDBREAK);
                SelectObject(dc, old);
                ReleaseDC(hwnd, dc);
                rect.bottom.max(px(16))
            };
            let line_height = px(18);
            let field_height = px(24);

            let message = &self.request.message;
            let h = text_height(message, width);
            make("STATIC", message, 0, 0, 0, h, y, margin, width);
            y += h + px(12);

            let mut rows = Vec::new();
            for (index, &field) in shape.fields().iter().enumerate() {
                make("STATIC", shape.label(field), 0, 0, 0, line_height, y, margin, width);
                y += line_height + px(2);
                let edit = make(
                    "EDIT",
                    "",
                    WS_TABSTOP | WS_BORDER | ES_PASSWORD as u32 | ES_AUTOHSCROLL as u32,
                    WS_EX_CLIENTEDGE,
                    ID_FIRST_FIELD + index as i32,
                    field_height,
                    y,
                    margin,
                    width,
                );
                SendMessageW(edit, EM_SETPASSWORDCHAR, HIDDEN_CHAR, 0);
                y += field_height + px(2);
                let line = make("STATIC", "", 0, 0, 0, line_height * 2, y, margin, width);
                ShowWindow(line, SW_HIDE);
                y += line_height * 2;
                if field == Field::Passphrase && shape.has_strength() {
                    let meter =
                        make("msctls_progress32", "", 0, 0, 0, px(10), y + px(4), margin, px(160));
                    SendMessageW(meter, PBM_SETRANGE32, 0, 5);
                    let label = make(
                        "STATIC",
                        "",
                        0,
                        0,
                        0,
                        line_height,
                        y,
                        margin + px(172),
                        width - px(172),
                    );
                    ShowWindow(meter, SW_HIDE);
                    self.meter.set(meter);
                    self.meter_label.set(label);
                    y += line_height + px(2);
                    let suggestion = make("STATIC", "", 0, 0, 0, line_height * 2, y, margin, width);
                    self.suggestion.set(suggestion);
                    y += line_height * 2;
                    let h = text_height(ADVICE, width);
                    make("STATIC", ADVICE, 0, 0, 0, h, y, margin, width);
                    y += h + px(8);
                }
                rows.push(Row { field, edit, line });
            }
            *self.rows.borrow_mut() = rows;

            let show = make(
                "BUTTON",
                shape.show_label(),
                WS_TABSTOP | BS_AUTOCHECKBOX as u32 | BS_NOTIFY as u32,
                0,
                ID_SHOW,
                line_height + px(4),
                y,
                margin,
                width,
            );
            self.show.set(show);
            y += line_height + px(8);
            if shape.has_acknowledgement() {
                let h = text_height(ACKNOWLEDGEMENT, width - px(20)) + px(4);
                let check = make(
                    "BUTTON",
                    ACKNOWLEDGEMENT,
                    WS_TABSTOP | BS_AUTOCHECKBOX as u32 | BS_MULTILINE as u32 | BS_NOTIFY as u32,
                    0,
                    ID_ACKNOWLEDGEMENT,
                    h,
                    y,
                    margin,
                    width,
                );
                self.acknowledgement.set(check);
                y += h + px(8);
            }
            let busy = make("STATIC", &self.request.busy, 0, 0, 0, line_height, y, margin, width);
            ShowWindow(busy, SW_HIDE);
            self.busy_label.set(busy);
            y += line_height + px(8);

            let button_w = px(110);
            let button_h = px(28);
            let primary = make(
                "BUTTON",
                &self.request.primary,
                WS_TABSTOP | BS_DEFPUSHBUTTON as u32 | BS_NOTIFY as u32,
                0,
                IDOK,
                button_h,
                y,
                margin + width - 2 * button_w - px(8),
                button_w,
            );
            let cancel = make(
                "BUTTON",
                "Cancel",
                WS_TABSTOP | BS_PUSHBUTTON as u32 | BS_NOTIFY as u32,
                0,
                IDCANCEL,
                button_h,
                y,
                margin + width - button_w,
                button_w,
            );
            self.primary.set(primary);
            self.cancel.set(cancel);
            y += button_h + margin;

            // The window around that client area, centred on the owner.
            let mut frame = RECT { left: 0, top: 0, right: width + 2 * margin, bottom: y };
            AdjustWindowRectEx(
                &mut frame,
                WS_POPUP | WS_CAPTION | WS_SYSMENU,
                0,
                WS_EX_DLGMODALFRAME,
            );
            let (w, h) = (frame.right - frame.left, frame.bottom - frame.top);
            let mut owner_rect: RECT = std::mem::zeroed();
            let (x, top) = if GetWindowRect(self.owner, &mut owner_rect) != 0 {
                (
                    owner_rect.left + (owner_rect.right - owner_rect.left - w) / 2,
                    owner_rect.top + (owner_rect.bottom - owner_rect.top - h) / 3,
                )
            } else {
                ((GetSystemMetrics(SM_CXSCREEN) - w) / 2, (GetSystemMetrics(SM_CYSCREEN) - h) / 3)
            };
            SetWindowPos(hwnd, std::ptr::null_mut(), x, top, w, h, SWP_NOZORDER);
        }
        self.changed(None);
    }

    /// `control` has the focus: remembered, and named in the window's
    /// `HoploDexFocus` property, which the E2E suite's key helper posts to
    /// while another app is active (`e2e/scripts/dialog-input.ps1`).
    fn focused(&self, control: HWND) {
        self.last_focus.set(control);
        // SAFETY: a property of this dialog, removed at WM_DESTROY.
        unsafe { SetPropW(self.hwnd.get(), FOCUS_PROP.as_ptr(), control) };
    }

    fn row_edit(&self, field: Field) -> Option<HWND> {
        self.rows.borrow().iter().find(|r| r.field == field).map(|r| r.edit)
    }

    fn checked(control: HWND) -> bool {
        // SAFETY: a checkbox of this dialog.
        !control.is_null()
            && unsafe { SendMessageW(control, BM_GETCHECK, 0, 0) } == BST_CHECKED as isize
    }

    /// Runs `with` on the fields' text, copied into wiped buffers.
    fn with_texts<T>(&self, with: impl FnOnce(Texts<'_>) -> T) -> T {
        let text = |field| self.row_edit(field).map(text_of);
        let (current, passphrase, confirmation) =
            (text(Field::Current), text(Field::Passphrase), text(Field::Confirmation));
        let texts = Texts {
            current: current.as_deref().map(String::as_str),
            passphrase: passphrase.as_deref().map_or("", String::as_str),
            confirmation: confirmation.as_deref().map_or("", String::as_str),
            acknowledged: Self::checked(self.acknowledgement.get()),
        };
        with(texts)
    }

    fn changed(&self, field: Option<Field>) {
        let view = self.with_texts(|texts| self.controller.borrow_mut().typed(field, texts));
        self.apply(&view);
        // SAFETY: a timer on this dialog, restarted on each change.
        unsafe { SetTimer(self.hwnd.get(), PAUSE_TIMER, PAUSE.as_millis() as u32, None) };
    }

    fn apply(&self, view: &View) {
        // SAFETY: plain Win32 calls on this dialog's controls.
        unsafe {
            for row in self.rows.borrow().iter() {
                match view.line(row.field) {
                    Some(line) => {
                        SetWindowTextW(row.line, wide(line).as_ptr());
                        ShowWindow(row.line, SW_SHOW);
                    }
                    None => {
                        ShowWindow(row.line, SW_HIDE);
                    }
                }
            }
            if !self.meter.get().is_null() {
                match &view.estimate {
                    Some(estimate) => {
                        SendMessageW(
                            self.meter.get(),
                            PBM_SETPOS,
                            usize::from(estimate.score) + 1,
                            0,
                        );
                        ShowWindow(self.meter.get(), SW_SHOW);
                        SetWindowTextW(self.meter_label.get(), wide(estimate.label()).as_ptr());
                        SetWindowTextW(self.suggestion.get(), wide(&estimate.suggestion).as_ptr());
                    }
                    None => {
                        ShowWindow(self.meter.get(), SW_HIDE);
                        SetWindowTextW(self.meter_label.get(), wide("").as_ptr());
                        SetWindowTextW(self.suggestion.get(), wide("").as_ptr());
                    }
                }
            }
            EnableWindow(self.primary.get(), i32::from(view.ready && !self.busy.get()));
        }
    }

    /// The default button, or Enter: what was typed goes to the caller, once,
    /// and the dialog waits, busy. Nothing while the checks don't pass (the
    /// One Enter rule).
    fn submit(&self) {
        if self.busy.get() {
            return;
        }
        let answer = self.with_texts(|texts| {
            self.controller.borrow().ready(texts).then(|| Answer {
                passphrase: Passphrase::from_typed(texts.passphrase),
                current: texts.current.map(Passphrase::from_typed),
            })
        });
        let Some(answer) = answer else { return };
        self.set_busy(true);
        let _ = self.events.send(Event::Submitted(answer));
    }

    fn cancel(&self) {
        if !self.busy.get() {
            self.send_end(Event::Cancelled);
        }
    }

    fn set_busy(&self, busy: bool) {
        self.busy.set(busy);
        let on = i32::from(!busy);
        // SAFETY: plain Win32 calls on this dialog's controls.
        unsafe {
            for row in self.rows.borrow().iter() {
                EnableWindow(row.edit, on);
            }
            EnableWindow(self.show.get(), on);
            if !self.acknowledgement.get().is_null() {
                EnableWindow(self.acknowledgement.get(), on);
            }
            EnableWindow(self.cancel.get(), on);
            if busy {
                EnableWindow(self.primary.get(), 0);
            }
            ShowWindow(self.busy_label.get(), if busy { SW_SHOW } else { SW_HIDE });
        }
    }

    fn toggle_show(&self) {
        let char = if Self::checked(self.show.get()) { 0 } else { HIDDEN_CHAR };
        // SAFETY: plain Win32 calls on this dialog's edit controls.
        unsafe {
            for row in self.rows.borrow().iter() {
                SendMessageW(row.edit, EM_SETPASSWORDCHAR, char, 0);
                InvalidateRect(row.edit, std::ptr::null(), 1);
            }
        }
    }

    fn retry(&self, retry: &Retry) {
        self.set_busy(false);
        if let Some(edit) = self.row_edit(retry.field) {
            // SAFETY: an edit control of this dialog.
            unsafe {
                SetWindowTextW(edit, wide("").as_ptr());
                SetFocus(edit);
            }
        }
        let view = self.with_texts(|texts| self.controller.borrow_mut().retried(retry, texts));
        self.apply(&view);
    }

    fn send_end(&self, event: Event) {
        if !self.ended.replace(true) {
            let _ = self.events.send(event);
        }
    }

    /// Empties the fields, gives the owner back its input and takes the
    /// dialog down.
    fn end(&self) {
        self.ended.set(true);
        // SAFETY: plain Win32 calls on this dialog and its owner. The owner
        // is enabled first, so the activation goes back to it.
        unsafe {
            KillTimer(self.hwnd.get(), PAUSE_TIMER);
            for row in self.rows.borrow().iter() {
                SetWindowTextW(row.edit, wide("").as_ptr());
            }
            EnableWindow(self.owner, 1);
            DestroyWindow(self.hwnd.get());
        }
    }
}
