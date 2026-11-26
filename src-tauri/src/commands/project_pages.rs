//! The project's web pages, opened from About HoploDex in the default browser
//! (#256). The page names one of two pages, never an address: each is a fixed
//! address built in, with no query or fragment and nothing about the user,
//! the machine, the collection, the version or the build. Touches no database,
//! so it is unscoped.

use std::sync::Arc;

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::commands::documents::{OpenFailure, Opener};
use crate::commands::error::CommandError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectPage {
    Source,
    LatestRelease,
}

impl ProjectPage {
    pub fn address(self) -> &'static str {
        match self {
            ProjectPage::Source => "https://github.com/exodious/HoploDex",
            ProjectPage::LatestRelease => "https://github.com/exodious/HoploDex/releases/latest",
        }
    }
}

/// Hands the page's address to the OS's handler for web addresses.
pub fn open(opener: &dyn Opener, page: ProjectPage) -> Result<(), CommandError> {
    opener.open_url(page.address()).map_err(|failure| {
        let kind = match failure {
            OpenFailure::NoApp => "no browser is set",
            OpenFailure::Other(_) => "the OS refused",
        };
        log::warn!("could not open a project page: {kind}");
        CommandError::new("INTERNAL_ERROR", "The page couldn't be opened in your web browser.")
    })
}

/// The launcher may take a moment, so not on an async thread.
#[tauri::command]
pub async fn open_project_page(app: AppHandle, page: ProjectPage) -> Result<(), CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let opener = app.state::<Arc<dyn Opener>>();
        open(&**opener, page)
    })
    .await
    .map_err(|_| {
        CommandError::new("INTERNAL_ERROR", "The page couldn't be opened in your web browser.")
    })?
}
