use super::{StorageError, UserError};

const SERVICE_TITLE: &str = "Storage service unavailable";
const SERVICE_MESSAGE: &str =
    "The storage service isn't running. Disk Manager reconnects when it is back.";
const DENIED_TITLE: &str = "Permission denied";
const DENIED_MESSAGE: &str = "You don't have permission to change this disk.";
const CANCELLED_TITLE: &str = "Cancelled";
const CANCELLED_MESSAGE: &str = "Cancelled. The disk was left unchanged.";
const FAILED_MESSAGE: &str = "The disk reported an error.";
const UDISKS_ERROR_PREFIX: &str = "org.freedesktop.UDisks2.Error.";

/// What the failing operation was and which volume or disk it acted on, for wording the error.
#[derive(Debug, Clone, Copy)]
pub struct FailureContext<'a> {
    /// Title shown for failures without a more specific explanation, e.g. "Couldn't format Photos".
    pub title: &'a str,
    /// Name of the disk or volume the operation acted on.
    pub subject: &'a str,
}

impl super::UserError {
    /// The error shown while the storage service is not running.
    pub fn service_unavailable() -> Self {
        Self::new(SERVICE_TITLE, SERVICE_MESSAGE)
    }
}

impl StorageError {
    /// Plain-language explanation of this error for the user.
    pub fn to_user_error(&self, context: FailureContext<'_>) -> UserError {
        match self {
            Self::Udisks(udisks2::Error::Zbus(error)) | Self::Dbus(error) => {
                dbus_error(error, context)
            }
            Self::Udisks(error) => udisks_error(error, None, context),
            Self::Io(error) => UserError::new(
                context.title,
                "Something went wrong while reading or writing.",
            )
            .with_details(error.to_string()),
            Self::Gone(name) => UserError::new(
                "Disk disconnected",
                format!("{name} is no longer connected. Nothing was changed."),
            ),
            Self::Changed => UserError::new(
                "The disk changed",
                "The disk changed while this window was open, so nothing was done. Check the disk and try again.",
            ),
            Self::Refused(refusal) => UserError::new(context.title, refusal.to_string()),
            Self::Cancelled => UserError::new(CANCELLED_TITLE, CANCELLED_MESSAGE),
            Self::NeedsRepair { name } => UserError::new(
                context.title,
                format!(
                    "{name} has errors, so nothing was changed. Use Check & Repair, then try again."
                ),
            ),
            Self::ImageTooLarge { capacity } => UserError::new(
                context.title,
                format!(
                    "The image doesn't fit on {}, which holds {capacity}.",
                    context.subject
                ),
            ),
            Self::VerificationFailed => UserError::new(
                context.title,
                format!(
                    "The data read back from {} doesn't match the image. The disk may be failing.",
                    context.subject
                ),
            ),
        }
    }

    /// True when the storage service itself went away, rather than one call failing.
    pub fn is_service_lost(&self) -> bool {
        match self {
            Self::Udisks(udisks2::Error::Zbus(error)) | Self::Dbus(error) => is_service_lost(error),
            _ => false,
        }
    }
}

fn dbus_error(error: &zbus::Error, context: FailureContext<'_>) -> UserError {
    if is_service_lost(error) {
        return UserError::new(SERVICE_TITLE, SERVICE_MESSAGE);
    }
    match error {
        zbus::Error::MethodError(name, detail, _)
            if name.as_str().starts_with(UDISKS_ERROR_PREFIX) =>
        {
            udisks_error(
                &udisks2::Error::from(error.clone()),
                detail.as_deref(),
                context,
            )
        }
        zbus::Error::FDO(fdo) if matches!(**fdo, zbus::fdo::Error::AccessDenied(_)) => {
            UserError::new(DENIED_TITLE, DENIED_MESSAGE)
        }
        _ => UserError::new(context.title, FAILED_MESSAGE).with_details(error.to_string()),
    }
}

fn is_service_lost(error: &zbus::Error) -> bool {
    use zbus::fdo::Error as Fdo;
    const LOST_SERVICE_NAMES: [&str; 4] = [
        "org.freedesktop.DBus.Error.ServiceUnknown",
        "org.freedesktop.DBus.Error.NameHasNoOwner",
        "org.freedesktop.DBus.Error.NoReply",
        "org.freedesktop.DBus.Error.Disconnected",
    ];
    match error {
        zbus::Error::InputOutput(_) => true,
        zbus::Error::MethodError(name, _, _) => LOST_SERVICE_NAMES.contains(&name.as_str()),
        zbus::Error::FDO(fdo) => matches!(
            **fdo,
            Fdo::ServiceUnknown(_)
                | Fdo::NameHasNoOwner(_)
                | Fdo::NoReply(_)
                | Fdo::Disconnected(_)
        ),
        _ => false,
    }
}

fn udisks_error(
    error: &udisks2::Error,
    detail: Option<&str>,
    context: FailureContext<'_>,
) -> UserError {
    use udisks2::Error as Udisks;
    let subject = context.subject;
    let explained = |message: String| {
        let user_error = UserError::new(context.title, message);
        match detail {
            Some(detail) => user_error.with_details(detail),
            None => user_error,
        }
    };
    match error {
        Udisks::NotAuthorizedDismissed => UserError {
            silent: true,
            ..UserError::new(CANCELLED_TITLE, CANCELLED_MESSAGE)
        },
        Udisks::NotAuthorized | Udisks::NotAuthorizedCanObtain | Udisks::OptionNotPermitted => {
            UserError::new(DENIED_TITLE, DENIED_MESSAGE)
        }
        Udisks::Cancelled | Udisks::AlreadyCancelled => {
            UserError::new(CANCELLED_TITLE, CANCELLED_MESSAGE)
        }
        Udisks::DeviceBusy => explained(format!(
            "{subject} is in use. Unmount it or close the files and apps using it, then try again."
        )),
        Udisks::NotSupported => explained(
            "This system is missing the tools needed for this. Install them and try again."
                .to_owned(),
        ),
        Udisks::AlreadyMounted => explained(format!("{subject} is already mounted.")),
        Udisks::NotMounted => explained(format!("{subject} isn't mounted.")),
        Udisks::AlreadyUnmounting => explained(format!("{subject} is already being unmounted.")),
        Udisks::MountedByOtherUser => explained(format!("{subject} is mounted by another user.")),
        Udisks::TimedOut => explained(format!("{subject} didn't respond in time. Try again.")),
        Udisks::WouldWakeup => explained(format!("{subject} is asleep. Use it, then try again.")),
        Udisks::Zbus(error) => dbus_error(error, context),
        Udisks::Failed | Udisks::Iscsi(_) => explained(FAILED_MESSAGE.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard::Refusal;

    const CONTEXT: FailureContext<'static> = FailureContext {
        title: "Couldn't format Photos",
        subject: "Photos",
    };

    fn method_error(name: &str, detail: &str) -> StorageError {
        let message = zbus::message::Message::method_call("/", "Format")
            .build(&())
            .expect("message");
        let name = zbus::names::OwnedErrorName::try_from(name.to_owned()).expect("error name");
        StorageError::Dbus(zbus::Error::MethodError(
            name,
            Some(detail.to_owned()),
            message,
        ))
    }

    #[test]
    fn dismissed_authentication_is_silent() {
        let error =
            StorageError::Udisks(udisks2::Error::NotAuthorizedDismissed).to_user_error(CONTEXT);
        assert!(error.silent);
        assert!(
            !StorageError::Udisks(udisks2::Error::NotAuthorized)
                .to_user_error(CONTEXT)
                .silent
        );
    }

    #[test]
    fn plan_messages() {
        let message =
            |error: udisks2::Error| StorageError::Udisks(error).to_user_error(CONTEXT).message;
        assert_eq!(message(udisks2::Error::NotAuthorized), DENIED_MESSAGE);
        assert_eq!(
            message(udisks2::Error::DeviceBusy),
            "Photos is in use. Unmount it or close the files and apps using it, then try again."
        );
        assert_eq!(message(udisks2::Error::Cancelled), CANCELLED_MESSAGE);
    }

    #[test]
    fn failures_keep_the_tool_output_in_details() {
        let error = method_error(
            "org.freedesktop.UDisks2.Error.Failed",
            "mkfs.exfat: device is read-only",
        )
        .to_user_error(CONTEXT);
        assert_eq!(error.title, CONTEXT.title);
        assert_eq!(error.message, FAILED_MESSAGE);
        assert_eq!(
            error.details.as_deref(),
            Some("mkfs.exfat: device is read-only")
        );

        let busy = method_error("org.freedesktop.UDisks2.Error.DeviceBusy", "target is busy")
            .to_user_error(CONTEXT);
        assert!(busy.message.starts_with("Photos is in use"));
        assert!(
            method_error(
                "org.freedesktop.UDisks2.Error.NotAuthorizedDismissed",
                "dismissed"
            )
            .to_user_error(CONTEXT)
            .silent
        );
    }

    #[test]
    fn lost_service_is_recognised() {
        let lost = method_error(
            "org.freedesktop.DBus.Error.ServiceUnknown",
            "udisksd is gone",
        );
        assert!(lost.is_service_lost());
        assert_eq!(lost.to_user_error(CONTEXT).title, SERVICE_TITLE);
        assert!(!StorageError::Cancelled.is_service_lost());
    }

    #[test]
    fn refusals_and_changes_read_plainly() {
        let refused = StorageError::Refused(Refusal::Busy).to_user_error(CONTEXT);
        assert_eq!(
            refused.message,
            "Another operation is running on this disk."
        );
        assert!(
            StorageError::Changed
                .to_user_error(CONTEXT)
                .message
                .contains("changed while this window was open")
        );
    }
}
