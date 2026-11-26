//! About HoploDex's links to the project's pages (#256): the page can name
//! only one of two pages, each a fixed address with nothing added to it.

use std::path::Path;
use std::sync::Mutex;

use hoplodex_lib::commands::documents::{OpenFailure, Opener};
use hoplodex_lib::commands::project_pages::{ProjectPage, open};

const SOURCE: &str = "https://github.com/exodious/HoploDex";
const LATEST: &str = "https://github.com/exodious/HoploDex/releases/latest";

#[derive(Default)]
struct Opened(Mutex<Vec<String>>, bool);

impl Opener for Opened {
    fn open(&self, _path: &Path) -> Result<(), OpenFailure> {
        unreachable!("a project page is never a file")
    }
    fn open_url(&self, url: &str) -> Result<(), OpenFailure> {
        self.0.lock().unwrap().push(url.to_owned());
        if self.1 { Err(OpenFailure::NoApp) } else { Ok(()) }
    }
}

#[test]
fn the_page_names_one_of_two_pages_and_never_an_address() {
    let page = |json: &str| serde_json::from_str::<ProjectPage>(json);
    assert_eq!(page(r#""source""#).unwrap(), ProjectPage::Source);
    assert_eq!(page(r#""latestRelease""#).unwrap(), ProjectPage::LatestRelease);
    for refused in [
        format!("{SOURCE:?}"),
        format!("{:?}", format!("{SOURCE}?from=app")),
        r#""Source""#.into(),
        r#""latest-release""#.into(),
        r#""""#.into(),
        r#"{"source":"https://example.com"}"#.into(),
        "null".into(),
    ] {
        assert!(page(&refused).is_err(), "{refused} was taken");
    }
}

#[test]
fn each_page_opens_its_fixed_address_and_only_that() {
    let opened = Opened::default();
    open(&opened, ProjectPage::Source).unwrap();
    open(&opened, ProjectPage::LatestRelease).unwrap();
    assert_eq!(*opened.0.lock().unwrap(), [SOURCE, LATEST]);
}

#[test]
fn a_page_that_cannot_be_opened_is_an_error() {
    let opened = Opened(Mutex::default(), true);
    assert_eq!(open(&opened, ProjectPage::Source).unwrap_err().code, "INTERNAL_ERROR");
}

/// The address About shows comes from `package.json` (scripts/licenses/notices.mjs);
/// the one opened is built into Rust. They are the same.
#[test]
fn the_source_page_is_the_address_about_shows() {
    let package: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../package.json"))
            .unwrap(),
    )
    .unwrap();
    let url = package["repository"]["url"].as_str().unwrap();
    assert_eq!(url.trim_start_matches("git+").trim_end_matches(".git"), SOURCE);
}
