//! MIXEL and project links for the Help menu and About dialog.

/// The upstream application slug retained for compatible project routing.
pub const APP: &str = "lightcraft";
/// The MIXEL website.
pub const WEBSITE: &str = "https://www.mixel.ch";
/// This app's MIXEL project page.
pub const APP_PAGE: &str = "https://github.com/jangopeopleV2/mixel-light";
/// This app's source repository.
pub const GITHUB: &str = "https://github.com/jangopeopleV2/mixel-light";

/// (UI command id, menu label, URL) for each link, in Help-menu order.
/// The user documentation (docs/ in the repository).
pub const HELP: &str = "https://github.com/jangopeopleV2/mixel-light/tree/main/docs";

pub const LINKS: &[(&str, &str, &str)] = &[
    ("app.help", "MIXEL Light Help", HELP),
    ("app.website", "MIXEL Light Website", APP_PAGE),
    ("app.github", "MIXEL Light on GitHub", GITHUB),
    ("app.mixelWebsite", "MIXEL Website", WEBSITE),
    ("app.feedback", "Send Feedback…", FEEDBACK),
];

/// Where feedback and bug reports go.
pub const FEEDBACK: &str = "https://github.com/jangopeopleV2/mixel-light/issues/new";

/// The URL behind a link command id.
pub fn url_of(cmd: &str) -> Option<&'static str> {
    LINKS.iter().find(|(id, _, _)| *id == cmd).map(|(_, _, u)| *u)
}

/// Open `url` in the user's browser (through the host's `open_url` service).
pub fn open(app: &mut crate::LightcraftApp, url: &str) -> Result<serde_json::Value, String> {
    let open = app.services.open_url.as_mut().ok_or("can't open links here")?;
    open(url)?;
    Ok(serde_json::json!({ "url": url }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_https_and_name_this_app() {
        let mut ids = std::collections::BTreeSet::new();
        for (id, label, url) in LINKS {
            assert!(ids.insert(id), "duplicate link command: {id}");
            assert!(!label.to_ascii_lowercase().contains("artcraft"));
            assert!(!url.to_ascii_lowercase().contains("artcraft"));
            assert!(url.starts_with("https://"), "{id}");
            assert!(!label.is_empty());
            assert_eq!(url_of(id), Some(*url));
        }
        assert!(APP_PAGE.starts_with("https://github.com/jangopeopleV2/mixel-light"));
        assert!(GITHUB.starts_with("https://github.com/jangopeopleV2/mixel-light"));
    }
}
