//! A command's session: the `Session` state together with the id the request
//! named in its `HoploDex-Session` header (008 research.md §2, FR-004).
//!
//! A command that reaches the open database takes `session: ScopedSession`
//! where it took `State<'_, Session>`. A request that names no session, or a
//! malformed one, is refused with `DATABASE_CLOSED` before the command runs:
//! it can't be served in any session.

use tauri::Runtime;
use tauri::http::HeaderMap;
use tauri::ipc::{CommandArg, CommandItem, InvokeError};

use super::{Session, SessionId};
use crate::commands::CommandError;
use rusqlite::Connection;

/// The header every scoped command is sent with: the open session's
/// `DatabaseStatus.sessionId`, as a decimal integer (contracts/
/// tauri-commands.md "Every request names its session").
pub const SESSION_HEADER: &str = "HoploDex-Session";

/// The [`Session`] and the id the request named.
#[derive(Clone)]
pub struct ScopedSession {
    pub session: Session,
    pub id: SessionId,
}

/// The session id a request's headers name: one `HoploDex-Session` header
/// holding a decimal `u64` that isn't 0. Anything else is `DATABASE_CLOSED`.
pub fn session_id_from_headers(headers: &HeaderMap) -> Result<SessionId, CommandError> {
    let mut values = headers.get_all(SESSION_HEADER).iter();
    let (Some(value), None) = (values.next(), values.next()) else {
        return Err(CommandError::database_closed());
    };
    // `str::parse` would also take a leading `+`.
    let digits = value.as_bytes();
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return Err(CommandError::database_closed());
    }
    std::str::from_utf8(digits)
        .ok()
        .and_then(|digits| digits.parse::<u64>().ok())
        .and_then(SessionId::from_request)
        .ok_or_else(CommandError::database_closed)
}

impl ScopedSession {
    /// The scope a request with these `headers` has in `session`.
    pub fn from_headers(session: &Session, headers: &HeaderMap) -> Result<Self, CommandError> {
        Ok(Self { session: session.clone(), id: session_id_from_headers(headers)? })
    }

    /// The request's session is the open one, without taking the session's
    /// mutex (so it can be asked while a close holds it). A check for work
    /// outside the mutex to stop early; whatever reaches the database
    /// compares again under the mutex.
    pub fn is_current(&self) -> bool {
        self.session.current_id() == self.id
    }

    /// [`Session::read`], in the request's session.
    ///
    /// Temporary (008 T004): the id is compared here, without the mutex,
    /// until the accessors take it and compare under the mutex (T010).
    pub fn read<T>(
        &self,
        query: impl FnOnce(&Connection) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        if !self.is_current() {
            return Err(CommandError::database_closed());
        }
        self.session.read(query)
    }
}

impl<'de, R: Runtime> CommandArg<'de, R> for ScopedSession {
    /// Takes the `Session` state, as `State<Session>` does, and the request's
    /// header. A refusal is the command's error, a `CommandError` the
    /// frontend already handles.
    fn from_command(command: CommandItem<'de, R>) -> Result<Self, InvokeError> {
        let Some(session) = command.message.state_ref().try_get::<Session>() else {
            log::error!("no Session managed for `{}` on command `{}`", command.key, command.name);
            return Err(InvokeError::from(CommandError::new(
                "INTERNAL_ERROR",
                "Something went wrong. Please try again.",
            )));
        };
        Self::from_headers(&session, command.message.headers()).map_err(InvokeError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::http::HeaderValue;

    fn headers(value: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(value) = value {
            headers.insert(SESSION_HEADER, HeaderValue::from_str(value).unwrap());
        }
        headers
    }

    fn refused(headers: &HeaderMap) -> bool {
        session_id_from_headers(headers).is_err_and(|e| e.code == "DATABASE_CLOSED")
    }

    #[test]
    fn a_good_header_names_its_session() {
        let id = session_id_from_headers(&headers(Some("42"))).unwrap();
        assert_eq!(id.get(), 42);
        let session = Session::default();
        let scoped = ScopedSession::from_headers(&session, &headers(Some("42"))).unwrap();
        assert_eq!(scoped.id, id);
        // Nothing is open, so no request is for the current session.
        assert!(!scoped.is_current());
    }

    #[test]
    fn a_missing_header_is_refused() {
        assert!(refused(&headers(None)));
    }

    #[test]
    fn a_malformed_header_is_refused() {
        for value in ["", "abc", "-1", "+7", " 7", "7 ", "1.5", "0x10", "18446744073709551616"] {
            assert!(refused(&headers(Some(value))), "{value:?} should be refused");
        }
        // Two headers can't both be the request's session.
        let mut two = headers(Some("1"));
        two.append(SESSION_HEADER, HeaderValue::from_static("2"));
        assert!(refused(&two));
    }

    #[test]
    fn a_zero_header_is_refused() {
        assert!(refused(&headers(Some("0"))));
        assert!(refused(&headers(Some("000"))));
    }

    #[test]
    fn the_header_name_is_case_insensitive() {
        let mut headers = HeaderMap::new();
        headers.insert("hoplodex-session", HeaderValue::from_static("9"));
        assert_eq!(session_id_from_headers(&headers).unwrap().get(), 9);
    }
}
