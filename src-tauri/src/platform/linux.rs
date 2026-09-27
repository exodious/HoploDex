//! Linux: logind on the system bus for sleep, shutdown and the session's
//! lock, and the ScreenSaver interfaces on the session bus (research.md
//! §14). Each signal stream is read on its own thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread;

use zbus::blocking::fdo::DBusProxy;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedFd, OwnedObjectPath};

use super::{Ack, SystemEvent};

const LOGIND: &str = "org.freedesktop.login1";
const LOGIND_PATH: &str = "/org/freedesktop/login1";
const LOGIND_MANAGER: &str = "org.freedesktop.login1.Manager";
const LOGIND_SESSION: &str = "org.freedesktop.login1.Session";

/// The ScreenSaver interfaces that report `ActiveChanged`, with their
/// object paths.
const SCREENSAVERS: [(&str, &str); 2] = [
    ("org.freedesktop.ScreenSaver", "/org/freedesktop/ScreenSaver"),
    ("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver"),
];

/// Starts the listeners. Returns whether a screen lock can be reported: the
/// logind session object or a ScreenSaver interface is reachable now.
pub fn spawn(sender: Sender<SystemEvent>) -> bool {
    // Several sources report the same lock; only a change is sent on.
    let locked = Arc::new(AtomicBool::new(false));
    let mut supported = false;
    match Connection::system() {
        Ok(system) => {
            listen_for_power(&system, "sleep", "PrepareForSleep", sender.clone());
            listen_for_power(&system, "shutdown", "PrepareForShutdown", sender.clone());
            supported |= listen_to_session(&system, &sender, &locked);
        }
        Err(err) => log::warn!("no system bus, so no sleep or shutdown notices: {err}"),
    }
    match Connection::session() {
        Ok(session) => {
            for (name, path) in SCREENSAVERS {
                supported |= listen_to_screensaver(&session, name, path, &sender, &locked);
            }
        }
        Err(err) => log::warn!("no session bus, so no screensaver notices: {err}"),
    }
    supported
}

/// A logind delay inhibitor for `what`: the OS waits, up to its limit, for
/// it to be released before going ahead (research.md §14).
fn inhibit(manager: &Proxy<'_>, what: &str) -> Option<OwnedFd> {
    manager
        .call("Inhibit", &(what, "HoploDex", "Locking the open database", "delay"))
        .map_err(|err| log::warn!("could not delay {what}: {err}"))
        .ok()
}

/// Reads logind's `PrepareForSleep` or `PrepareForShutdown`, holding a
/// delay inhibitor for `what` that is released once the lock has finished
/// (the event's ack) and taken again on waking.
fn listen_for_power(
    system: &Connection,
    what: &'static str,
    signal: &'static str,
    sender: Sender<SystemEvent>,
) {
    let system = system.clone();
    let spawned = thread::Builder::new().name(format!("logind-{what}")).spawn(move || {
        let manager = match Proxy::new(&system, LOGIND, LOGIND_PATH, LOGIND_MANAGER) {
            Ok(manager) => manager,
            Err(err) => return log::warn!("no logind, so no {what} notices: {err}"),
        };
        let signals = match manager.receive_signal(signal) {
            Ok(signals) => signals,
            Err(err) => return log::warn!("could not listen for {signal}: {err}"),
        };
        let mut inhibitor = inhibit(&manager, what);
        for message in signals {
            let Ok(starting) = message.body().deserialize::<bool>() else { continue };
            let event = if starting {
                let held = inhibitor.take();
                let ack = Ack::new(move || drop(held));
                if what == "sleep" {
                    SystemEvent::WillSleep { ack }
                } else {
                    SystemEvent::WillShutDown { ack }
                }
            } else {
                // Awake again (a shutdown that was cancelled, for the other).
                inhibitor = inhibitor.or_else(|| inhibit(&manager, what));
                if what == "sleep" {
                    SystemEvent::Woke
                } else {
                    continue;
                }
            };
            if sender.send(event).is_err() {
                return;
            }
        }
    });
    if let Err(err) = spawned {
        log::error!("could not start the {what} listener: {err}");
    }
}

/// Sends a lock or unlock when `now_locked` changes what is known.
fn send_lock(sender: &Sender<SystemEvent>, locked: &AtomicBool, now_locked: bool) -> bool {
    if locked.swap(now_locked, Ordering::SeqCst) == now_locked {
        return true;
    }
    let event = if now_locked { SystemEvent::ScreenLocked } else { SystemEvent::ScreenUnlocked };
    sender.send(event).is_ok()
}

/// This process's logind session: its `Lock` and `Unlock` signals and its
/// `LockedHint` property. Returns whether the session object is reachable.
fn listen_to_session(
    system: &Connection,
    sender: &Sender<SystemEvent>,
    locked: &Arc<AtomicBool>,
) -> bool {
    let path = Proxy::new(system, LOGIND, LOGIND_PATH, LOGIND_MANAGER)
        .and_then(|manager| {
            manager.call::<_, _, OwnedObjectPath>("GetSessionByPID", &(std::process::id(),))
        })
        .map(|path| path.to_string())
        .unwrap_or_else(|_| format!("{LOGIND_PATH}/session/auto"));
    let reachable = Proxy::new(system, LOGIND, path.as_str(), LOGIND_SESSION)
        .and_then(|session| session.get_property::<bool>("LockedHint"))
        .is_ok();
    if !reachable {
        return false;
    }
    for (signal, now_locked) in [("Lock", true), ("Unlock", false)] {
        let (system, path, sender, locked) =
            (system.clone(), path.clone(), sender.clone(), Arc::clone(locked));
        let spawned = thread::Builder::new().name(format!("logind-{signal}")).spawn(move || {
            let Ok(session) = Proxy::new(&system, LOGIND, path.as_str(), LOGIND_SESSION) else {
                return;
            };
            let Ok(signals) = session.receive_signal(signal) else { return };
            for _ in signals {
                if !send_lock(&sender, &locked, now_locked) {
                    return;
                }
            }
        });
        if let Err(err) = spawned {
            log::error!("could not start the session {signal} listener: {err}");
        }
    }
    let (system, sender, locked) = (system.clone(), sender.clone(), Arc::clone(locked));
    let spawned = thread::Builder::new().name("logind-locked-hint".into()).spawn(move || {
        let Ok(session) = Proxy::new(&system, LOGIND, path.as_str(), LOGIND_SESSION) else {
            return;
        };
        for changed in session.receive_property_changed::<bool>("LockedHint") {
            let Ok(now_locked) = changed.get() else { continue };
            if !send_lock(&sender, &locked, now_locked) {
                return;
            }
        }
    });
    if let Err(err) = spawned {
        log::error!("could not start the LockedHint listener: {err}");
    }
    true
}

/// A ScreenSaver interface's `ActiveChanged`. Returns whether a service
/// owns its name now.
fn listen_to_screensaver(
    session: &Connection,
    name: &'static str,
    path: &'static str,
    sender: &Sender<SystemEvent>,
    locked: &Arc<AtomicBool>,
) -> bool {
    let owned = DBusProxy::new(session)
        .and_then(|bus| Ok(bus.name_has_owner(name.try_into()?)?))
        .unwrap_or(false);
    if !owned {
        return false;
    }
    let (session, sender, locked) = (session.clone(), sender.clone(), Arc::clone(locked));
    let spawned = thread::Builder::new().name(format!("{name}-listener")).spawn(move || {
        let Ok(screensaver) = Proxy::new(&session, name, path, name) else { return };
        let Ok(signals) = screensaver.receive_signal("ActiveChanged") else { return };
        for message in signals {
            let Ok(active) = message.body().deserialize::<bool>() else { continue };
            if !send_lock(&sender, &locked, active) {
                return;
            }
        }
    });
    if let Err(err) = spawned {
        log::error!("could not start the {name} listener: {err}");
    }
    true
}
