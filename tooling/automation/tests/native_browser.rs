//! Real managed-Firefox scenarios. Each test owns its profile and browser process.
use anyhow::Result;
use latch_automation::native_browser::BrowserFixture;
use serde_json::json;

#[test]
fn matching_origin_autofills() -> Result<()> {
    let mut browser = BrowserFixture::start()?;
    browser.seed_login("fixture-user", "fixture-password-one")?;
    browser.navigate_to_login()?;
    browser.assert_password("fixture-password-one")
}

#[test]
fn other_origin_cannot_autofill() -> Result<()> {
    let mut browser = BrowserFixture::start()?;
    browser.seed_login("fixture-user", "fixture-password-one")?;
    browser.navigate_to_other_origin()?;
    browser.assert_password("")
}

#[test]
fn malformed_batch_preserves_existing_login() -> Result<()> {
    let mut browser = BrowserFixture::start()?;
    browser.seed_login("fixture-user", "fixture-password-one")?;
    browser.reject_batch(json!([
        {"id":"fixture-login","origin":browser.origin(),"username":"fixture-user","password":"invalid-update"},
        {"id":"invalid-login","origin":"file:///not-an-origin","username":"other","password":"invalid"}
    ]))?;
    browser.navigate_to_login()?;
    browser.assert_password("fixture-password-one")
}

#[test]
fn updating_login_replaces_autofill() -> Result<()> {
    let mut browser = BrowserFixture::start()?;
    browser.seed_login("fixture-user", "fixture-password-one")?;
    browser.seed_login("fixture-user", "fixture-password-two")?;
    browser.navigate_to_login()?;
    browser.assert_password("fixture-password-two")
}

#[test]
fn reopening_applies_pending_update() -> Result<()> {
    let mut browser = BrowserFixture::start()?;
    browser.seed_login("fixture-user", "fixture-password-one")?;
    browser.reopen_with_login("fixture-user", "fixture-password-three")?;
    browser.navigate_to_login()?;
    browser.assert_password("fixture-password-three")
}

#[test]
fn deleting_login_removes_autofill() -> Result<()> {
    let mut browser = BrowserFixture::start()?;
    browser.seed_login("fixture-user", "fixture-password-one")?;
    browser.delete_login()?;
    browser.navigate_to_login()?;
    browser.assert_password("")
}

#[test]
fn partial_failure_can_recover() -> Result<()> {
    let mut browser = BrowserFixture::start()?;
    browser.reject_batch(json!([
        {"id":"partial-one","origin":browser.origin(),"username":"partial-one","password":"first-insert"},
        {"id":"partial-two","origin":browser.origin(),"username":"partial-two","password":"\0"}
    ]))?;
    browser.seed_login("partial-one", "recovered-insert")?;
    browser.navigate_to_login()?;
    browser.assert_password("recovered-insert")?;
    browser.delete_login()
}

#[test]
fn browser_owned_login_cannot_be_overwritten_or_deleted() -> Result<()> {
    let mut browser = BrowserFixture::start()?;
    browser.seed_login("fixture-user", "browser-owned-password")?;
    browser.revoke_ownership()?;
    browser.reject_batch(json!([
        {"id":"fixture-login","origin":browser.origin(),"username":"fixture-user","password":"must-not-overwrite"}
    ]))?;
    browser.delete_login()?;
    browser.navigate_to_login()?;
    browser.assert_password("browser-owned-password")
}
