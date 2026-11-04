mod user_message;

use thiserror::Error;

pub use user_message::FailureContext;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error(transparent)]
    Udisks(#[from] udisks2::Error),
    #[error(transparent)]
    Dbus(#[from] zbus::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0} is no longer connected")]
    Gone(String),
    #[error("the disk changed while the confirmation was open")]
    Changed,
    #[error(transparent)]
    Refused(#[from] crate::guard::Refusal),
    #[error("cancelled")]
    Cancelled,
    #[error("{name} has errors that must be repaired first")]
    NeedsRepair { name: String },
    #[error("the image is larger than the {capacity} target")]
    ImageTooLarge { capacity: crate::model::ByteSize },
    #[error("the data read back from the disk doesn't match the image")]
    VerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserError {
    pub title: String,
    pub message: String,
    pub details: Option<String>,
    pub silent: bool,
}

impl UserError {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            details: None,
            silent: false,
        }
    }

    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}
