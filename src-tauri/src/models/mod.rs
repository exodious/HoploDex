/// A Rust enum stored as fixed text: `text` is both its SQL value and its
/// IPC (serde) form, so the two can never drift apart.
macro_rules! text_enum {
    ($name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        pub enum $name {
            $(#[serde(rename = $text)] $variant),+
        }

        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }
        }

        impl rusqlite::types::ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
                Ok(rusqlite::types::ToSqlOutput::from(self.as_str()))
            }
        }

        impl rusqlite::types::FromSql for $name {
            fn column_result(
                value: rusqlite::types::ValueRef<'_>,
            ) -> rusqlite::types::FromSqlResult<Self> {
                let text = value.as_str()?;
                match text {
                    $($text => Ok(Self::$variant)),+,
                    other => Err(rusqlite::types::FromSqlError::Other(
                        format!("unrecognized {} value: {other}", stringify!($name)).into(),
                    )),
                }
            }
        }
    };
}

pub mod action_type;
pub mod database;
pub mod disposition_history;
pub mod document_attachment;
pub mod firearm;
pub mod firearm_type;
pub mod insurance_policy;
pub mod photo;
pub mod registration;

pub use document_attachment::DocumentAttachment;
pub use firearm::{DispositionType, Firearm, FirearmInput, FirearmStatus};
pub use insurance_policy::{InsurancePolicy, InsurancePolicyInput};
pub use photo::Photo;
