use std::collections::HashMap;
use std::future::poll_fn;
use std::path::PathBuf;
use std::pin::pin;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use zbus::export::futures_core::Stream;
use zbus::{Connection, OwnedVariant, Proxy, Variant};

use super::file_uri::path_from_file_uri;

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const DESKTOP_PATH: &str = "/org/freedesktop/portal/desktop";
const CHOOSER_INTERFACE: &str = "org.freedesktop.portal.FileChooser";
const REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
const REQUEST_PATH_PREFIX: &str = "/org/freedesktop/portal/desktop/request";
const RESPONSE_SIGNAL: &str = "Response";
const RESPONSE_SUCCESS: u32 = 0;
const URIS_KEY: &str = "uris";
const TOKEN_PREFIX: &str = "crownos_diskmanager";
const GLOB_FILTER: u32 = 0;
const IMAGE_FILTER_NAME: &str = "Disk images";
pub const IMAGE_PATTERNS: [&str; 5] = ["*.img", "*.iso", "*.raw", "*.img.xz", "*.img.zst"];

static NEXT_TOKEN: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChoice {
    Open,
    Save { suggested_name: String },
}

impl FileChoice {
    const fn method(&self) -> &'static str {
        match self {
            Self::Open => "OpenFile",
            Self::Save { .. } => "SaveFile",
        }
    }
}

type Filter = (&'static str, Vec<(u32, &'static str)>);

fn image_filter() -> Filter {
    (
        IMAGE_FILTER_NAME,
        IMAGE_PATTERNS
            .iter()
            .map(|pattern| (GLOB_FILTER, *pattern))
            .collect(),
    )
}

fn handle_token() -> String {
    let sequence = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
    format!("{TOKEN_PREFIX}_{}_{sequence}", std::process::id())
}

/// The object path the portal answers on, per the `Request` documentation.
fn request_path(unique_name: &str, token: &str) -> String {
    let sender = unique_name.trim_start_matches(':').replace('.', "_");
    format!("{REQUEST_PATH_PREFIX}/{sender}/{token}")
}

fn chosen_path(response: u32, uris: Option<Vec<String>>) -> Option<PathBuf> {
    if response != RESPONSE_SUCCESS {
        return None;
    }
    path_from_file_uri(uris?.first()?)
}

/// Asks the desktop's file chooser for one disk image; `None` when the person cancels.
pub async fn choose_image(title: String, choice: FileChoice) -> Result<Option<PathBuf>> {
    let connection = Connection::session()
        .await
        .context("Couldn't reach the session bus")?;
    let token = handle_token();
    let unique_name = connection
        .unique_name()
        .context("The session bus gave no name")?;
    let path = request_path(unique_name.as_str(), &token);
    let request = Proxy::new(&connection, DESTINATION, path.as_str(), REQUEST_INTERFACE).await?;
    let responses = request.receive_signal(RESPONSE_SIGNAL).await?;
    let chooser = Proxy::new(&connection, DESTINATION, DESKTOP_PATH, CHOOSER_INTERFACE).await?;

    let filter = image_filter();
    let mut options: HashMap<&str, Variant<'_>> = HashMap::from([
        ("handle_token", Variant::from(token.as_str())),
        ("modal", Variant::from(true)),
        ("filters", Variant::from(vec![filter.clone()])),
        ("current_filter", Variant::from(filter)),
    ]);
    if let FileChoice::Save { suggested_name } = &choice {
        options.insert("current_name", Variant::from(suggested_name.as_str()));
    }
    let _: zbus::OwnedObjectPath = chooser
        .call(choice.method(), &("", title.as_str(), options))
        .await
        .context("The file chooser refused the request")?;

    let mut responses = pin!(responses);
    let message = poll_fn(|context| responses.as_mut().poll_next(context))
        .await
        .context("The file chooser closed without answering")?;
    let (response, mut results): (u32, HashMap<String, OwnedVariant>) =
        message.body().deserialize()?;
    let uris = results
        .remove(URIS_KEY)
        .map(Vec::<String>::try_from)
        .transpose()?;
    Ok(chosen_path(response, uris))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_answered_on_the_sender_and_token_path() {
        assert_eq!(
            request_path(":1.42", "crownos_diskmanager_7_0"),
            "/org/freedesktop/portal/desktop/request/1_42/crownos_diskmanager_7_0"
        );
    }

    #[test]
    fn tokens_are_unique_object_path_elements() {
        let (first, second) = (handle_token(), handle_token());
        assert_ne!(first, second);
        assert!(
            first
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        );
    }

    #[test]
    fn only_a_successful_response_yields_a_path() {
        let uris = || Some(vec!["file:///home/me/ubuntu.iso".to_owned()]);
        assert_eq!(
            chosen_path(RESPONSE_SUCCESS, uris()),
            Some("/home/me/ubuntu.iso".into())
        );
        assert_eq!(chosen_path(1, uris()), None);
        assert_eq!(chosen_path(2, uris()), None);
        assert_eq!(chosen_path(RESPONSE_SUCCESS, None), None);
        assert_eq!(chosen_path(RESPONSE_SUCCESS, Some(Vec::new())), None);
    }

    #[test]
    fn the_image_filter_lists_every_supported_extension() {
        let (name, patterns) = image_filter();
        assert_eq!(name, IMAGE_FILTER_NAME);
        assert_eq!(patterns.len(), IMAGE_PATTERNS.len());
        assert!(patterns.iter().all(|(kind, _)| *kind == GLOB_FILTER));
    }

    #[test]
    fn each_choice_calls_its_portal_method() {
        assert_eq!(FileChoice::Open.method(), "OpenFile");
        assert_eq!(
            FileChoice::Save {
                suggested_name: "a.img".into()
            }
            .method(),
            "SaveFile"
        );
    }
}
