use crate::command::Completion;

pub(crate) fn completed(message: impl Into<String>) -> Completion {
    Completion {
        message: message.into(),
    }
}
