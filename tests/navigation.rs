mod fixtures;
mod utils;

use assert_cmd::prelude::*;
use assert_fs::fixture::TempDir;
use fixtures::{port, tmpdir, Error, DEEPLY_NESTED_FILE, DIRECTORIES};
use pretty_assertions::{assert_eq, assert_ne};
use rstest::rstest;
use select::document::Document;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;
use url::Url;
use utils::get_link_from_text;

#[rstest]
/// The index directory gets a trailing slash.
fn index_gets_trailing_slash(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg("-p")
        .arg(port.to_string())
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let base_url = Url::parse(&format!("http://localhost:{}", port))?;
    let expected_url = format!("{}", base_url);
    let resp = reqwest::get(base_url.as_str())?;
    assert_eq!(resp.url().as_str(), expected_url);

    child.kill()?;

    Ok(())
}

#[rstest]
/// We can navigate into directories and back using shown links.
fn can_navigate_into_dirs_and_back(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg("-p")
        .arg(port.to_string())
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    let base_url = Url::parse(&format!("http://localhost:{}/", port))?;
    let initial_body = reqwest::get(base_url.as_str())?.error_for_status()?;
    let initial_parsed = Document::from_read(initial_body)?;
    for &directory in DIRECTORIES {
        let dir_elem = get_link_from_text(&initial_parsed, &directory).expect("Dir not found.");
        let body = reqwest::get(&format!("{}{}", base_url, dir_elem))?.error_for_status()?;
        let parsed = Document::from_read(body)?;
        let back_link =
            get_link_from_text(&parsed, "Parent directory").expect("Back link not found.");
        let resp = reqwest::get(&format!("{}{}", base_url, back_link))?;

        // Now check that we can actually get back to the original location we came from using the
        // link.
        assert_eq!(resp.url().as_str(), base_url.as_str());
    }

    child.kill()?;

    Ok(())
}

#[rstest]
/// We can navigate deep into the file tree and back using shown links.
fn can_navigate_deep_into_dirs_and_back(tmpdir: TempDir, port: u16) -> Result<(), Error> {
    let mut child = Command::cargo_bin("miniserve")?
        .arg("-p")
        .arg(port.to_string())
        .arg(tmpdir.path())
        .stdout(Stdio::null())
        .spawn()?;

    sleep(Duration::from_secs(1));

    // Create a vector of directory names. We don't need to fetch the file and so we'll
    // remove that part.
    let dir_names = {
        let mut comps = DEEPLY_NESTED_FILE
            .split("/")
            .map(|d| format!("{}/", d))
            .collect::<Vec<String>>();
        comps.pop();
        comps
    };
    let base_url = Url::parse(&format!("http://localhost:{}/", port))?;

    // First we'll go forwards through the directory tree and then we'll go backwards.
    // In the end, we'll have to end up where we came from.
    let mut next_url = base_url.clone();
    for dir_name in dir_names.iter() {
        let resp = reqwest::get(next_url.as_str())?;
        let body = resp.error_for_status()?;
        let parsed = Document::from_read(body)?;
        let dir_elem = get_link_from_text(&parsed, &dir_name).expect("Dir not found.");
        next_url = next_url.join(&dir_elem)?;
    }
    assert_ne!(base_url, next_url);

    // Now try to get out the tree again using links only.
    while next_url != base_url {
        let resp = reqwest::get(next_url.as_str())?;
        let body = resp.error_for_status()?;
        let parsed = Document::from_read(body)?;
        let dir_elem =
            get_link_from_text(&parsed, "Parent directory").expect("Back link not found.");
        next_url = next_url.join(&dir_elem)?;
    }
    assert_eq!(base_url, next_url);

    child.kill()?;

    Ok(())
}
