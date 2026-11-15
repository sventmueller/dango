//! Which CI service a git remote belongs to, and where its API is.

use std::fmt;

/// The kind of service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// GitHub, on github.com or an Enterprise Server.
    GitHub,
    /// GitLab, on gitlab.com or a self-managed instance.
    GitLab,
}

impl Kind {
    /// The word for it in host rules and keychain accounts.
    pub fn word(self) -> &'static str {
        match self {
            Kind::GitHub => "github",
            Kind::GitLab => "gitlab",
        }
    }

    fn from_word(word: &str) -> Option<Kind> {
        match word.trim().to_ascii_lowercase().as_str() {
            "github" => Some(Kind::GitHub),
            "gitlab" => Some(Kind::GitLab),
            _ => None,
        }
    }
}

/// A repository on a CI service: where its API is and how the API names it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Forge {
    /// The service.
    pub kind: Kind,
    /// The host with its port when it is not the scheme's, such as `gitlab.example.com:8443`; tokens belong to it.
    pub host: String,
    /// Where the instance lives, such as `https://gitlab.example.com:8443/gitlab`.
    pub web: String,
    /// The API's base, such as `https://api.github.com` or `https://gitlab.example.com/api/v4`.
    pub api: String,
    /// The repository's path, such as `owner/repo` or `group/subgroup/project`.
    pub project: String,
}

impl Forge {
    fn new(kind: Kind, scheme: &str, host: &str, prefix: &str, project: &str) -> Forge {
        let web = format!("{scheme}://{host}{prefix}");
        let api = match (kind, host) {
            (Kind::GitHub, "github.com") => "https://api.github.com".to_string(),
            (Kind::GitHub, _) => format!("{web}/api/v3"),
            (Kind::GitLab, _) => format!("{web}/api/v4"),
        };
        Forge {
            kind,
            host: host.to_string(),
            web,
            api,
            project: project.to_string(),
        }
    }

    /// The repository's page.
    pub fn page(&self) -> String {
        format!("{}/{}", self.web, self.project)
    }
}

impl fmt::Display for Forge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} · {}", self.host, self.project)
    }
}

/// The user's word on a host the name does not give away: `host[:port][/prefix]=github|gitlab`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRule {
    /// The host with its port, as remotes name it.
    pub host: String,
    /// The path an instance lives under, such as `/gitlab`; empty for none.
    pub prefix: String,
    /// What runs there.
    pub kind: Kind,
}

impl HostRule {
    /// The rule a settings entry writes; `None` for one that makes no sense.
    pub fn parse(text: &str) -> Option<HostRule> {
        let (place, kind) = text.split_once('=')?;
        let kind = Kind::from_word(kind)?;
        let place = place.trim().trim_end_matches('/');
        let (host, prefix) = match place.split_once('/') {
            Some((host, rest)) => (host, format!("/{rest}")),
            None => (place, String::new()),
        };
        let (name, port) = host.split_once(':').unwrap_or((host, "0"));
        let fits = !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
            && port.parse::<u16>().is_ok()
            && !prefix
                .chars()
                .any(|c| c.is_whitespace() || matches!(c, '?' | '#' | '='));
        fits.then(|| HostRule {
            host: host.to_ascii_lowercase(),
            prefix,
            kind,
        })
    }

    /// The settings entry that says this rule.
    pub fn entry(&self) -> String {
        format!("{}{}={}", self.host, self.prefix, self.kind.word())
    }
}

/// A remote URL taken apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteUrl {
    /// `https`, or `http` for a remote that says so; ssh remotes reach the API over https.
    pub(crate) scheme: String,
    /// The host, with the port of an http(s) remote that names one.
    pub host: String,
    /// The path without slashes around it and without `.git`.
    pub(crate) path: String,
}

impl RemoteUrl {
    /// Takes apart the URLs git takes: `https://`, `http://`, `ssh://`, `git://` and `user@host:path`.
    pub fn parse(url: &str) -> Option<RemoteUrl> {
        let url = url.trim();
        let (scheme, rest) = match url.split_once("://") {
            Some((scheme, rest)) => (scheme.to_ascii_lowercase(), rest),
            None => {
                let (authority, path) = url.split_once(':')?;
                if authority.contains('/') || path.starts_with("//") {
                    return None;
                }
                return Self::finish("https", without_user(authority), path, false);
            }
        };
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let host = without_user(authority);
        match scheme.as_str() {
            "https" | "http" => Self::finish(&scheme, host, path, true),
            "ssh" | "git" | "git+ssh" | "ssh+git" => Self::finish("https", host, path, false),
            _ => None,
        }
    }

    fn finish(scheme: &str, host: &str, path: &str, keep_port: bool) -> Option<RemoteUrl> {
        let host = if keep_port {
            host.to_ascii_lowercase()
        } else {
            host.split(':').next().unwrap_or(host).to_ascii_lowercase()
        };
        let host = match (scheme, host.strip_suffix(":443"), host.strip_suffix(":80")) {
            ("https", Some(bare), _) | ("http", _, Some(bare)) => bare.to_string(),
            _ => host,
        };
        let path = path.trim_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path).trim_end_matches('/');
        (!host.is_empty() && path.contains('/')).then(|| RemoteUrl {
            scheme: scheme.to_string(),
            host,
            path: path.to_string(),
        })
    }
}

fn without_user(authority: &str) -> &str {
    authority.rsplit_once('@').map_or(authority, |(_, host)| host)
}

/// The places the repository of `url` may live: one for a known service or a host rule; for a GitLab instance the
/// name points at, one per way to split the path into an instance prefix and a project, shortest prefix first, which
/// the API tells apart. Empty when nothing says which service runs there.
pub fn candidates(url: &str, rules: &[HostRule]) -> Vec<Forge> {
    let Some(remote) = RemoteUrl::parse(url) else {
        return Vec::new();
    };
    let RemoteUrl { scheme, host, path } = &remote;
    if let Some(rule) = rules.iter().find(|r| r.host == *host && under(path, &r.prefix)) {
        let project = path[rule.prefix.trim_start_matches('/').len()..].trim_start_matches('/');
        return vec![Forge::new(rule.kind, scheme, host, &rule.prefix, project)];
    }
    let name = host.split(':').next().unwrap_or(host);
    let kind = match name {
        "github.com" => return vec![Forge::new(Kind::GitHub, "https", "github.com", "", path)],
        "gitlab.com" => return vec![Forge::new(Kind::GitLab, "https", "gitlab.com", "", path)],
        _ if name.split('.').any(|part| part.contains("gitlab")) => Kind::GitLab,
        _ if name.split('.').any(|part| part.contains("github")) => Kind::GitHub,
        _ => return Vec::new(),
    };
    if kind == Kind::GitHub {
        return vec![Forge::new(kind, scheme, host, "", path)];
    }
    let parts: Vec<&str> = path.split('/').collect();
    (0..parts.len().saturating_sub(1))
        .map(|cut| {
            let prefix: String = parts[..cut].iter().map(|p| format!("/{p}")).collect();
            Forge::new(kind, scheme, host, &prefix, &parts[cut..].join("/"))
        })
        .collect()
}

/// Whether `path` starts with the folders of `prefix`.
fn under(path: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_matches('/');
    prefix.is_empty() || path.strip_prefix(prefix).is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_url_form_git_takes_names_host_and_path() {
        for (url, scheme, host, path) in [
            (
                "https://github.com/acme/widget.git",
                "https",
                "github.com",
                "acme/widget",
            ),
            (
                "https://user:secret@github.com/acme/widget",
                "https",
                "github.com",
                "acme/widget",
            ),
            ("git@github.com:acme/widget.git", "https", "github.com", "acme/widget"),
            (
                "ssh://git@gitlab.example.com:2222/group/sub/app.git",
                "https",
                "gitlab.example.com",
                "group/sub/app",
            ),
            (
                "https://gitlab.example.com:8443/gitlab/group/app.git/",
                "https",
                "gitlab.example.com:8443",
                "gitlab/group/app",
            ),
            (
                "https://github.com:443/acme/widget.git",
                "https",
                "github.com",
                "acme/widget",
            ),
            (
                "http://127.0.0.1:8080/acme/widget.git",
                "http",
                "127.0.0.1:8080",
                "acme/widget",
            ),
        ] {
            let remote = RemoteUrl::parse(url).unwrap_or_else(|| panic!("{url}"));
            assert_eq!(
                (remote.scheme.as_str(), remote.host.as_str(), remote.path.as_str()),
                (scheme, host, path),
                "{url}"
            );
        }
        for url in ["/local/path/repo", "file:///repo.git", "https://github.com/onlyone", ""] {
            assert_eq!(RemoteUrl::parse(url), None, "{url}");
        }
    }

    #[test]
    fn known_services_and_names_choose_the_api() {
        let github = candidates("git@github.com:acme/widget.git", &[]);
        assert_eq!(github.len(), 1);
        assert_eq!(github[0].api, "https://api.github.com");
        assert_eq!(github[0].project, "acme/widget");
        let enterprise = candidates("https://github.corp.example/acme/widget.git", &[]);
        assert_eq!(enterprise[0].api, "https://github.corp.example/api/v3");
        assert_eq!(enterprise[0].kind, Kind::GitHub);
        let gitlab = candidates("git@gitlab.com:group/sub/app.git", &[]);
        assert_eq!(gitlab[0].api, "https://gitlab.com/api/v4");
        assert_eq!(gitlab[0].project, "group/sub/app");
        assert!(candidates("https://git.example.com/a/b.git", &[]).is_empty());
    }

    #[test]
    fn a_self_managed_gitlab_under_a_path_offers_each_split_shortest_prefix_first() {
        let forges = candidates("https://gitlab.example.com:8443/tools/gitlab/group/app.git", &[]);
        let splits: Vec<(&str, &str)> = forges.iter().map(|f| (f.api.as_str(), f.project.as_str())).collect();
        assert_eq!(
            splits,
            [
                ("https://gitlab.example.com:8443/api/v4", "tools/gitlab/group/app"),
                ("https://gitlab.example.com:8443/tools/api/v4", "gitlab/group/app"),
                ("https://gitlab.example.com:8443/tools/gitlab/api/v4", "group/app"),
            ]
        );
        assert!(forges.iter().all(|f| f.host == "gitlab.example.com:8443"));
    }

    #[test]
    fn a_host_rule_names_the_service_and_its_prefix() {
        let rules = [
            HostRule::parse("git.example.com/scm = GitLab").unwrap(),
            HostRule::parse("127.0.0.1:8080=github").unwrap(),
        ];
        assert_eq!(rules[0].prefix, "/scm");
        let forge = &candidates("ssh://git@git.example.com/scm/group/app.git", &rules)[0];
        assert_eq!(forge.api, "https://git.example.com/scm/api/v4");
        assert_eq!(forge.project, "group/app");
        assert_eq!(forge.page(), "https://git.example.com/scm/group/app");
        let local = &candidates("http://127.0.0.1:8080/acme/widget.git", &rules)[0];
        assert_eq!(local.api, "http://127.0.0.1:8080/api/v3");
        assert!(candidates("https://git.example.com/other/group/app.git", &rules).is_empty());
        assert_eq!(HostRule::parse("example.com=bitbucket"), None);
        assert_eq!(HostRule::parse("=github"), None);
        for odd in [
            "bad host!=gitlab",
            "git.example.com:http=gitlab",
            "a.b/x y=github",
            "a.b:99999=github",
        ] {
            assert_eq!(HostRule::parse(odd), None, "{odd}");
        }
        assert_eq!(
            HostRule::parse(" Git.Example.com:8443/scm/ = GitLab ").unwrap().entry(),
            "git.example.com:8443/scm=gitlab"
        );
    }
}
