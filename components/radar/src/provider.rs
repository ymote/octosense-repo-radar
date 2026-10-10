use crate::functions::*;
use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub const API: &str = "https://api.github.com";
pub const BODY_LIMIT: usize = 2 * 1024 * 1024;
pub struct Wire {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}
impl Wire {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    pub fn json(&self) -> Result<Value, String> {
        if self.body.len() > BODY_LIMIT {
            return Err("GitHub response exceeds the 2 MiB parsing limit. Open the project on GitHub instead.".into());
        }
        serde_json::from_slice(&self.body)
            .map_err(|_| "GitHub returned invalid JSON; saved data was retained.".into())
    }
}
pub trait Transport {
    fn get(&self, url: &str, etag: Option<&str>) -> Result<Wire, String>;
}
pub struct Github;
impl Transport for Github {
    fn get(&self, url: &str, etag: Option<&str>) -> Result<Wire, String> {
        if !url.starts_with("https://api.github.com/") {
            return Err("Only the public GitHub API is supported.".into());
        }
        let mut request = octosense_component::http::Request::get(url)
            .header("user-agent", "Repo-Radar-OctoSense/0.1.0")
            .header("accept", "application/vnd.github+json")
            .header("x-github-api-version", "2026-03-10")
            .timeout(std::time::Duration::from_secs(3));
        if let Some(tag) = etag.filter(|s| s.len() <= 512 && !s.contains(['\r', '\n'])) {
            request = request.header("if-none-match", tag);
        }
        // The SDK buffers the body. This component's post-read cap bounds JSON
        // parsing/storage, while the host retains its memory and call deadline.
        let r = request.send().map_err(|_| {
            "Could not reach GitHub. Check the connection and retry; saved data is unchanged."
                .to_string()
        })?;
        Ok(Wire {
            status: r.status,
            headers: r.headers,
            body: r.body,
        })
    }
}
pub fn timestamp(seconds: i64) -> String {
    OffsetDateTime::from_unix_timestamp(seconds)
        .ok()
        .and_then(|d| d.format(&Rfc3339).ok())
        .unwrap_or_default()
}
pub fn optional_date(v: &Value) -> Option<String> {
    v.as_str()
        .and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok())
        .and_then(|d| d.to_offset(time::UtcOffset::UTC).format(&Rfc3339).ok())
}
pub fn required_date(v: &Value) -> Result<String, String> {
    optional_date(v).ok_or_else(|| "GitHub returned an invalid timestamp.".into())
}
pub fn text(v: &Value, max: usize) -> String {
    clean(v.as_str().unwrap_or_default(), max)
}
pub fn clean(s: &str, max: usize) -> String {
    s.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .take(max)
        .collect()
}
pub fn full_name(s: &str) -> Result<String, String> {
    let s = s.trim();
    let parts: Vec<_> = s.split('/').collect();
    if parts.len() != 2
        || parts[0].is_empty()
        || parts[0].len() > 39
        || parts[1].is_empty()
        || parts[1].len() > 100
        || !parts[0]
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        || !parts[1]
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
        || matches!(parts[1], "." | "..")
    {
        return Err("Use a public GitHub owner/repository name.".into());
    }
    Ok(s.to_string())
}
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            use std::fmt::Write;
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}
pub fn count(v: &Value, key: &str) -> Result<u64, String> {
    v[key]
        .as_u64()
        .ok_or_else(|| format!("GitHub response is missing a valid {key}."))
}
pub fn repo(v: &Value) -> Result<Repo, String> {
    if v["private"].as_bool() != Some(false) {
        return Err("Only verified public repositories can be added.".into());
    }
    let name = full_name(v["full_name"].as_str().unwrap_or_default())?;
    let id = count(v, "id")?;
    if id == 0 {
        return Err("GitHub returned an invalid repository ID.".into());
    }
    Ok(Repo {
        id,
        html_url: format!("https://github.com/{name}"),
        full_name: name,
        description: text(&v["description"], 512),
        language: text(&v["language"], 64),
        stars: count(v, "stargazers_count")?,
        forks: count(v, "forks_count")?,
        open_issues_and_prs: count(v, "open_issues_count")?,
        archived: v["archived"].as_bool().unwrap_or(false),
        pushed_at: optional_date(&v["pushed_at"]),
    })
}
pub fn overview(v: &Value, now: i64) -> Result<Overview, String> {
    let topics = v["topics"]
        .as_array()
        .map(|a| a.iter().take(20).map(|x| text(x, 64)).collect())
        .unwrap_or_default();
    Ok(Overview {
        repo: repo(v)?,
        subscribers: v["subscribers_count"].as_u64(),
        default_branch: text(&v["default_branch"], 128),
        license: text(&v["license"]["spdx_id"], 64),
        topics,
        created_at: required_date(&v["created_at"])?,
        observed_at: timestamp(now),
        previous_observed_at: None,
        star_delta: None,
        fork_delta: None,
        snapshots: vec![],
    })
}
pub fn pulls(v: &Value, name: &str) -> Result<Vec<Pull>, String> {
    let a = v.as_array().ok_or("GitHub returned an invalid PR list.")?;
    a.iter()
        .take(10)
        .map(|x| {
            let number = count(x, "number")?;
            let merged_at = optional_date(&x["merged_at"]);
            let state = match x["state"].as_str() {
                Some("closed") if merged_at.is_some() => PullState::Merged,
                Some("closed") => PullState::Closed,
                Some("open") if x["draft"].as_bool().unwrap_or(false) => PullState::Draft,
                Some("open") => PullState::Open,
                _ => return Err("GitHub returned an unknown PR state.".into()),
            };
            Ok(Pull {
                number,
                title: text(&x["title"], 256),
                html_url: format!("https://github.com/{name}/pull/{number}"),
                author: text(&x["user"]["login"], 64),
                state,
                created_at: required_date(&x["created_at"])?,
                updated_at: required_date(&x["updated_at"])?,
                closed_at: optional_date(&x["closed_at"]),
                merged_at,
                labels: x["labels"]
                    .as_array()
                    .map(|a| a.iter().take(10).map(|l| text(&l["name"], 64)).collect())
                    .unwrap_or_default(),
            })
        })
        .collect()
}
pub fn releases(v: &Value, name: &str) -> Result<Vec<Release>, String> {
    let a = v
        .as_array()
        .ok_or("GitHub returned an invalid release list.")?;
    a.iter()
        .filter(|x| x["draft"].as_bool() != Some(true))
        .take(5)
        .map(|x| {
            let tag = text(&x["tag_name"], 256);
            if tag.is_empty() {
                return Err("GitHub returned a release without a tag.".into());
            }
            let raw_body = x["body"].as_str().unwrap_or_default();
            let parsed = semver::Version::parse(tag.strip_prefix('v').unwrap_or(&tag))
                .ok()
                .map(|v| v.to_string());
            Ok(Release {
                id: count(x, "id")?,
                html_url: format!("https://github.com/{name}/releases/tag/{}", encode(&tag)),
                tag,
                name: text(&x["name"], 256),
                published_at: optional_date(&x["published_at"]),
                prerelease: x["prerelease"].as_bool().unwrap_or(false),
                body_preview: clean(raw_body, 2048),
                body_truncated: raw_body
                    .chars()
                    .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
                    .count()
                    > 2048,
                parsed_semver: parsed,
            })
        })
        .collect()
}
pub fn has_next(w: &Wire) -> bool {
    w.header("link")
        .is_some_and(|s| s.split(',').any(|x| x.contains("rel=\"next\"")))
}
pub fn page(
    n: usize,
    next: Option<u32>,
    total: Option<u64>,
    incomplete: bool,
    label: &str,
) -> Page {
    Page {
        returned: n as u32,
        next_page: next,
        total_count: total,
        incomplete,
        coverage: label.to_string(),
    }
}
