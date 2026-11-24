//! Update check: asks GitHub Releases whether a newer version exists. Notify only: nothing is
//! downloaded or installed.

use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

/// The version of this build, from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The GitHub API the check asks.
pub const API: &str = "https://api.github.com";
const REPO: &str = "Piya-Boy/Meeting-Recorder";
/// The longest answer that is read (a release answer is a few KiB).
const MAX_BODY: u64 = 256 * 1024;
const DAY_SECS: u64 = 24 * 60 * 60;

/// A newer release: its version (`N.N.N`) and the page the app opens for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub url: String,
}

/// What GitHub said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    UpToDate,
    Available(Release),
}

/// What the app shows about the check.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Failed,
}

/// One-shot text for a toast (taken once per frame by `AppState`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateNotice {
    Available(String),
    UpToDate,
    Failed,
}

/// Asks `api` about the latest release as seen from `current`; tests give their own.
pub type Fetch = fn(&str, &str) -> Result<Check, String>;

/// `N.N.N` with or without a leading `v`; anything else (pre-releases, extra parts) is `None`.
pub fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let mut parts = text.strip_prefix('v').unwrap_or(text).split('.');
    let mut number = || {
        let p = parts.next()?;
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        p.parse::<u32>().ok()
    };
    let version = (number()?, number()?, number()?);
    parts.next().is_none().then_some(version)
}

/// Whether `candidate` is a greater version than `current` (either one not a version: no).
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

/// The release page of `version`, built here and not taken from the network.
pub fn release_url(version: &str) -> String {
    format!("https://github.com/{REPO}/releases/tag/v{version}")
}

/// Whether `url` is exactly a release page of this project (the only thing the app opens).
pub fn is_safe_release_url(url: &str) -> bool {
    url.strip_prefix(&format!("https://github.com/{REPO}/releases/tag/"))
        .is_some_and(|tag| tag.starts_with('v') && parse_version(tag).is_some())
}

/// The release in a `releases/latest` answer. `Ok(None)`: a draft, a pre-release or a tag that is
/// not `vN.N.N`. `Err`: not the answer GitHub gives.
pub fn release_from_json(body: &str) -> Result<Option<Release>, String> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("unexpected answer: {e}"))?;
    let tag = json
        .get("tag_name")
        .and_then(|t| t.as_str())
        .ok_or("unexpected answer: no tag_name")?;
    let flagged = |key: &str| json.get(key).and_then(|v| v.as_bool()) == Some(true);
    if flagged("draft") || flagged("prerelease") {
        return Ok(None);
    }
    Ok(parse_version(tag).map(|(a, b, c)| {
        let version = format!("{a}.{b}.{c}");
        Release {
            url: release_url(&version),
            version,
        }
    }))
}

/// Whether the start-up check should run: the switch is on and the last good check is a day old
/// (or the clock went back).
pub fn due(enabled: bool, last_check: u64, now: u64) -> bool {
    enabled && (last_check > now || now - last_check >= DAY_SECS)
}

/// Seconds since 1970 (0 if the clock is before that).
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Asks `api` for the latest release. A 404 is "no release yet", which is up to date.
pub fn check(api: &str, current: &str) -> Result<Check, String> {
    use ureq::tls::{TlsConfig, TlsProvider};
    let agent = ureq::Agent::config_builder()
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .build(),
        )
        .timeout_global(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .build()
        .new_agent();
    let url = format!("{}/repos/{REPO}/releases/latest", api.trim_end_matches('/'));
    let mut response = agent
        .get(&url)
        .header("User-Agent", format!("meeting-recorder/{current}"))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;
    match response.status().as_u16() {
        200 => {}
        404 => return Ok(Check::UpToDate),
        other => return Err(format!("GitHub answered {other}")),
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_BODY)
        .read_to_string()
        .map_err(|e| e.to_string())?;
    Ok(match release_from_json(&body)? {
        Some(release) if is_newer(&release.version, current) => Check::Available(release),
        _ => Check::UpToDate,
    })
}

/// Runs `fetch` on its own thread; the one answer arrives on the returned channel.
pub fn spawn(fetch: Fetch, api: String, current: String) -> Receiver<Result<Check, String>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(fetch(&api, &current));
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn release_json(tag: &str) -> String {
        format!(
            r#"{{"tag_name":"{tag}","html_url":"https://evil.example/x","draft":false,"prerelease":false}}"#
        )
    }

    /// Answers each connection with the next `(status, body)` (the last one repeats) and keeps
    /// the request heads.
    fn serve(answers: Vec<(u16, String)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let heads = Arc::new(Mutex::new(Vec::new()));
        let log = heads.clone();
        std::thread::spawn(move || {
            for (n, conn) in listener.incoming().enumerate() {
                let Ok(mut conn) = conn else { continue };
                conn.set_read_timeout(Some(Duration::from_secs(5))).ok();
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    if conn.read(&mut byte).unwrap_or(0) == 0 {
                        break;
                    }
                    head.push(byte[0]);
                }
                log.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&head).into_owned());
                let (status, body) = &answers[n.min(answers.len() - 1)];
                let reply = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = conn.write_all(reply.as_bytes());
            }
        });
        (base, heads)
    }

    #[test]
    fn versions_parse_with_or_without_a_v_and_nothing_else() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("0.4.2"), Some((0, 4, 2)));
        for bad in [
            "",
            "v",
            "1.2",
            "1.2.3.4",
            "1.2.3-rc1",
            "v1.2.3+x",
            " 1.2.3",
            "1.2.3\n",
            "a.b.c",
            "1..3",
            "1.2.99999999999",
        ] {
            assert_eq!(parse_version(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn newer_means_a_greater_number_not_a_greater_text() {
        assert!(is_newer("0.5.0", "0.4.2"));
        assert!(is_newer("v1.0.0", "0.99.99"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(!is_newer("0.4.2", "0.4.2"));
        assert!(!is_newer("0.4.1", "0.4.2"));
        assert!(!is_newer("nightly", "0.4.2"));
        assert!(!is_newer("0.5.0", "broken"));
    }

    #[test]
    fn the_link_is_built_by_the_app_and_only_such_links_are_safe() {
        assert_eq!(
            release_url("0.5.0"),
            "https://github.com/Piya-Boy/Meeting-Recorder/releases/tag/v0.5.0"
        );
        assert!(is_safe_release_url(&release_url("12.0.31")));
        for bad in [
            "",
            "http://github.com/Piya-Boy/Meeting-Recorder/releases/tag/v0.5.0",
            "https://github.com.evil.example/Piya-Boy/Meeting-Recorder/releases/tag/v0.5.0",
            "https://github.com/other/Meeting-Recorder/releases/tag/v0.5.0",
            "https://github.com/Piya-Boy/Meeting-Recorder/releases/tag/v0.5.0/../../x",
            "https://github.com/Piya-Boy/Meeting-Recorder/releases/tag/v0.5.0?x=1",
            "https://github.com/Piya-Boy/Meeting-Recorder/releases/tag/v0.5.0 --flag",
            "https://github.com/Piya-Boy/Meeting-Recorder/releases/tag/nightly",
        ] {
            assert!(!is_safe_release_url(bad), "{bad:?}");
        }
    }

    #[test]
    fn a_release_answer_gives_the_version_and_our_own_link() {
        let r = release_from_json(&release_json("v0.5.0")).unwrap().unwrap();
        assert_eq!(r.version, "0.5.0");
        assert_eq!(
            r.url,
            release_url("0.5.0"),
            "html_url from the answer is not used"
        );
    }

    #[test]
    fn drafts_prereleases_and_odd_tags_are_no_release_and_bad_json_is_an_error() {
        let flagged = |k: &str| format!(r#"{{"tag_name":"v0.5.0","{k}":true}}"#);
        assert_eq!(release_from_json(&flagged("draft")), Ok(None));
        assert_eq!(release_from_json(&flagged("prerelease")), Ok(None));
        assert_eq!(release_from_json(&release_json("nightly")), Ok(None));
        assert_eq!(release_from_json(&release_json("v1.2.3-rc1")), Ok(None));
        for bad in ["", "not json", "[]", r#"{"name":"x"}"#, r#"{"tag_name":5}"#] {
            assert!(release_from_json(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn due_when_on_and_a_day_has_passed_since_the_last_good_check() {
        let now = 1_800_000_000;
        assert!(!due(false, 0, now));
        assert!(due(true, 0, now));
        assert!(!due(true, now - 86_399, now));
        assert!(due(true, now - 86_400, now));
        assert!(
            due(true, now + 5, now),
            "a clock that went back checks again"
        );
    }

    #[test]
    fn check_finds_a_newer_release_and_sends_a_plain_user_agent() {
        let (base, heads) = serve(vec![(200, release_json("v0.5.0"))]);
        let got = check(&base, "0.4.2").unwrap();
        assert_eq!(
            got,
            Check::Available(Release {
                version: "0.5.0".to_owned(),
                url: release_url("0.5.0")
            })
        );
        let head = heads.lock().unwrap()[0].to_lowercase();
        assert!(
            head.starts_with("get /repos/piya-boy/meeting-recorder/releases/latest "),
            "{head}"
        );
        assert!(
            head.contains("user-agent: meeting-recorder/0.4.2"),
            "{head}"
        );
    }

    #[test]
    fn check_says_up_to_date_for_the_same_or_an_older_release_a_draft_and_no_release_yet() {
        for (status, body) in [
            (200, release_json("v0.4.2")),
            (200, release_json("v0.3.0")),
            (200, r#"{"tag_name":"v9.9.9","draft":true}"#.to_owned()),
            (404, r#"{"message":"Not Found"}"#.to_owned()),
        ] {
            let (base, _) = serve(vec![(status, body.clone())]);
            assert_eq!(
                check(&base, "0.4.2"),
                Ok(Check::UpToDate),
                "{status} {body}"
            );
        }
    }

    #[test]
    fn check_fails_on_server_errors_bad_answers_huge_answers_and_no_server() {
        for (status, body) in [
            (500, "oops".to_owned()),
            (403, r#"{"message":"rate limit"}"#.to_owned()),
            (200, "not json".to_owned()),
            (
                200,
                format!(r#"{{"tag_name":"v9.9.9"}}{}"#, " ".repeat(1 << 20)),
            ),
        ] {
            let (base, _) = serve(vec![(status, body)]);
            assert!(check(&base, "0.4.2").is_err(), "{status}");
        }
        let closed = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", closed.local_addr().unwrap().port());
        drop(closed);
        assert!(check(&base, "0.4.2").is_err());
    }

    fn fake_up_to_date(_api: &str, _current: &str) -> Result<Check, String> {
        Ok(Check::UpToDate)
    }

    #[test]
    fn spawn_gives_exactly_one_answer_from_the_fetch_it_was_given() {
        let rx = spawn(
            fake_up_to_date,
            "http://unused".to_owned(),
            "0.4.2".to_owned(),
        );
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2)),
            Ok(Ok(Check::UpToDate))
        );
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
    }
}
