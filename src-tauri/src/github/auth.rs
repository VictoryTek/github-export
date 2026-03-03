use anyhow::{Context, Result};
use octocrab::Octocrab;

const KEYRING_SERVICE: &str = "github-export";
const KEYRING_USER: &str = "github-token";

/// Build an authenticated Octocrab client from a personal access token.
pub async fn authenticate_with_token(token: &str) -> Result<Octocrab> {
    let client = Octocrab::builder()
        .personal_token(token.to_string())
        .build()
        .context("Failed to build GitHub client")?;
    Ok(client)
}

/// Persist the token in the OS credential store (Keychain / Credential Manager / Secret Service).
pub fn store_token(token: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .context("Failed to create keyring entry")?;
    entry
        .set_password(token)
        .context("Failed to store token in keyring")?;
    Ok(())
}

/// Load a previously stored token from the OS credential store.
pub fn load_token() -> Result<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .context("Failed to access keyring entry")?;
    let token = entry
        .get_password()
        .context("No stored token found")?;
    Ok(token)
}

/// Remove the stored token from the OS credential store.
pub fn delete_token() -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .context("Failed to access keyring entry")?;
    entry
        .delete_credential()
        .context("Failed to delete stored token")?;
    Ok(())
}
