//! Which model writes commit messages and how the checked request reaches it (E88): the model chosen in the settings
//! from the own agent's providers (E6), else `claude -p` with a small model and no tools (E21), else none. A provider
//! outside the Mac needs the user's consent once (E101), which the settings can take back.

use std::path::PathBuf;

use ide_agent::{ApiKey, Cancel, Delta, Endpoint, Http, Message, Provider, Request, Stop};
use ide_core::settings::{Key, Settings};
use splat_i18n::t;

use ide_agent::ModelChoice;

/// The model the user chose for commit messages; `claude -p` when none is chosen.
pub const MODEL: Key<Option<ModelChoice>> =
    Key::user("commitMessageModel", ide_core::settings::Sharing::Shared, || None);
/// The providers the user allowed to get the request, by endpoint id or [`CLAUDE`].
pub const CONSENT: Key<Vec<String>> = Key::user("commitMessageConsent", ide_core::settings::Sharing::PerMac, Vec::new);
/// The consent's name for `claude -p`.
pub const CLAUDE: &str = "claude";

/// The instruction every request goes with; the request's own text carries everything else.
pub const SYSTEM: &str = "You write git commit messages.";
/// The longest message a model may write, in tokens.
const MAX_TOKENS: u64 = 1024;

/// What writes the message.
#[derive(Clone)]
pub enum Source {
    /// A model of one of the own agent's endpoints.
    Provider {
        endpoint: Endpoint,
        model: String,
        key: Option<ApiKey>,
    },
    /// `claude -p` at this path.
    Claude(PathBuf),
}

/// Why a request ends without a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    Cancelled,
    /// What went wrong, in the user's words.
    Failed(String),
}

impl Source {
    /// The model the settings name, or `claude` at `claude` when they name none; `Err` says why there is none.
    pub fn resolve(
        choice: Option<ModelChoice>,
        endpoint: impl Fn(&str) -> Option<Endpoint>,
        key: impl Fn(&Endpoint) -> Option<ApiKey>,
        claude: Option<PathBuf>,
    ) -> Result<Source, &'static str> {
        if let Some(choice) = choice {
            let Some(endpoint) = endpoint(&choice.endpoint) else {
                return Err(t(
                    "Den Anbieter für Commit-Nachrichten gibt es nicht mehr. Wähle einen in den Einstellungen unter „Agent“.",
                ));
            };
            let key = key(&endpoint);
            if key.is_none() && endpoint.kind.takes_key() && endpoint.is_first_party() {
                return Err(t(
                    "Der Anbieter für Commit-Nachrichten braucht einen Schlüssel; trag ihn in den Einstellungen unter „Agent“ ein.",
                ));
            }
            return Ok(Source::Provider {
                endpoint,
                model: choice.model,
                key,
            });
        }
        claude.map(Source::Claude).ok_or(t(
            "Kein Modell für Commit-Nachrichten: Wähle eines in den Einstellungen unter „Agent“ oder installiere claude.",
        ))
    }

    /// The source the settings and this Mac give now.
    pub fn current() -> Result<Source, &'static str> {
        Source::resolve(
            Settings::load_user().get(&MODEL),
            |id| crate::host::host().endpoint(id),
            |endpoint| crate::host::host().key(endpoint),
            claude(),
        )
    }

    /// The name the consent is kept under; `None` for a model on this Mac, which needs none.
    pub fn consent_id(&self) -> Option<String> {
        match self {
            Source::Provider { endpoint, .. } if on_this_mac(endpoint) => None,
            Source::Provider { endpoint, .. } => Some(endpoint.id.clone()),
            Source::Claude(_) => Some(CLAUDE.to_string()),
        }
    }

    /// The provider's name for the user.
    pub fn name(&self) -> String {
        match self {
            Source::Provider { endpoint, .. } => endpoint.name.clone(),
            Source::Claude(_) => "Claude (claude -p)".to_string(),
        }
    }

    /// Sends `text` and hands each piece of the answer to `on` as it arrives; the whole message at the end.
    pub fn send(&self, text: &str, cancel: &Cancel, on: &mut dyn FnMut(&str)) -> Result<String, Failure> {
        match self {
            Source::Provider { endpoint, model, key } => {
                let provider = Provider::new(endpoint.clone(), key.clone(), Http::new());
                let messages = [Message::user(text)];
                let request = Request {
                    model,
                    system: SYSTEM,
                    messages: &messages,
                    tools: &[],
                    max_tokens: Some(MAX_TOKENS),
                    effort: None,
                };
                let mut forward = |delta: Delta| {
                    if let Delta::Text(piece) = delta {
                        on(&piece);
                    }
                };
                match provider.stream(&request, cancel, &mut forward) {
                    Ok(reply) if reply.stop == Stop::Cancelled => Err(Failure::Cancelled),
                    Ok(reply) => Ok(reply.message.text()),
                    Err(error) => Err(Failure::Failed(crate::host::host().provider_error(&error))),
                }
            }
            Source::Claude(program) => {
                let environment = ide_ui_common::window::environment(std::time::Duration::ZERO);
                super::claude::run(program, &environment, text, cancel, on)
            }
        }
    }
}

/// Why no model can write commit messages now, if none can; reads no key.
pub fn unavailable() -> Option<&'static str> {
    match Settings::load_user().get(&MODEL) {
        Some(choice) => crate::host::host().endpoint(&choice.endpoint).is_none().then(|| {
            t("Den Anbieter für Commit-Nachrichten gibt es nicht mehr. Wähle einen in den Einstellungen unter „Agent“.")
        }),
        None => claude().is_none().then(|| {
            t("Kein Modell für Commit-Nachrichten: Wähle eines in den Einstellungen unter „Agent“ oder installiere claude.")
        }),
    }
}

/// `claude` on the login shell's PATH, once that is known.
fn claude() -> Option<PathBuf> {
    let environment = ide_ui_common::window::environment(std::time::Duration::ZERO);
    if environment.is_empty() {
        return None;
    }
    crate::host::host().claude_program(&environment)
}

/// Whether the model of `endpoint` runs on this Mac: Ollama or LM Studio at a loopback address. A server of another
/// kind on this Mac may pass the request on to a service outside it, as proxies do.
pub fn on_this_mac(endpoint: &Endpoint) -> bool {
    endpoint.kind.is_local() && loopback(&endpoint.base_url)
}

fn loopback(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split('/').next().unwrap_or("");
    let authority = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let host = if let Some(bracketed) = authority.strip_prefix('[') {
        bracketed.split(']').next().unwrap_or("")
    } else {
        authority.split(':').next().unwrap_or("")
    };
    host.eq_ignore_ascii_case("localhost")
        || host == "::1"
        || host.parse::<std::net::Ipv4Addr>().is_ok_and(|ip| ip.is_loopback())
}

/// Whether the user allowed `id` to get the request.
pub fn consented(id: &str) -> bool {
    Settings::load_user().get(&CONSENT).iter().any(|c| c == id)
}

/// Keeps the user's consent for `id`.
pub fn consent(id: &str) {
    let mut settings = Settings::load_user();
    let mut all = settings.get(&CONSENT);
    if !all.iter().any(|c| c == id) {
        all.push(id.to_string());
        if let Err(error) = settings.set(&CONSENT, &all) {
            ide_ui_common::diag::report("saving the consent for commit messages", error);
        }
    }
}

/// Takes back every consent given.
pub fn revoke_all() {
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&CONSENT, &Vec::new()) {
        ide_ui_common::diag::report("taking back the consent for commit messages", error);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use ide_agent::ProviderKind;
    use ide_agent::stub::{Answer, Stub};
    use ide_git::{Git, RequestScan};
    use ide_process::Supervisor;

    use super::*;

    fn choice(endpoint: &str) -> Option<ModelChoice> {
        Some(ModelChoice {
            endpoint: endpoint.into(),
            model: "m".into(),
        })
    }

    #[test]
    fn the_chosen_model_comes_first_then_claude_and_without_both_there_is_none() {
        let anthropic = Endpoint::preset(ProviderKind::Anthropic).unwrap();
        let found = |id: &str| (id == "anthropic").then(|| anthropic.clone());
        let claude = Some(PathBuf::from("/usr/local/bin/claude"));
        let key = |_: &Endpoint| ApiKey::new("sk-test");
        let no_key = |_: &Endpoint| None;
        assert!(matches!(
            Source::resolve(choice("anthropic"), found, key, claude.clone()),
            Ok(Source::Provider { .. })
        ));
        assert!(Source::resolve(choice("anthropic"), found, no_key, claude.clone()).is_err());
        assert!(Source::resolve(choice("gone"), found, key, claude.clone()).is_err());
        assert!(matches!(
            Source::resolve(None, found, key, claude),
            Ok(Source::Claude(_))
        ));
        let none = Source::resolve(None, found, key, None).err().unwrap();
        assert!(none.contains("claude"), "{none}");
    }

    #[test]
    fn only_ollama_and_lm_studio_on_this_mac_go_without_consent() {
        let consent = |kind: ProviderKind, url: &str| {
            Source::Provider {
                endpoint: Endpoint::new("e", kind, "E", url),
                model: "m".into(),
                key: None,
            }
            .consent_id()
        };
        assert_eq!(consent(ProviderKind::Ollama, "http://localhost:11434/v1"), None);
        assert_eq!(consent(ProviderKind::LmStudio, "http://127.0.0.1:1234/v1"), None);
        assert_eq!(consent(ProviderKind::Ollama, "http://[::1]:11434/v1"), None);
        assert_eq!(
            consent(ProviderKind::Ollama, "http://studio.local:11434/v1"),
            Some("e".into())
        );
        assert_eq!(
            consent(ProviderKind::OpenAiCompatible, "http://127.0.0.1:4000/v1"),
            Some("e".into())
        );
        assert_eq!(
            consent(ProviderKind::Anthropic, "https://api.anthropic.com"),
            Some("e".into())
        );
        assert_eq!(
            Source::Claude(PathBuf::from("claude")).consent_id(),
            Some(CLAUDE.into())
        );
    }

    /// The request is checked, then the file changes on disk, then the request goes out: the stub gets the checked
    /// text and nothing of the later change.
    #[test]
    fn what_goes_out_is_the_checked_state_even_when_the_file_changes_after_the_check() {
        let Some(dir) = ["/opt/homebrew/bin", "/usr/local/bin"]
            .into_iter()
            .find(|d| Path::new(d).join("gitleaks").exists())
        else {
            eprintln!("gitleaks is not installed here; without it nothing is ever sent");
            return;
        };
        let root = std::env::temp_dir().join(format!("ide-commit-message-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("home")).unwrap();
        std::fs::create_dir_all(root.join("r")).unwrap();
        let root = root.canonicalize().unwrap();
        let environment = vec![
            ("PATH".into(), format!("{dir}:/usr/bin:/bin")),
            ("HOME".into(), root.join("home").display().to_string()),
            ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
            ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ];
        let git = Git::new(&Supervisor::new(), environment);
        let repo = git.init(&root.join("r")).unwrap();
        std::fs::write(repo.root().join("a.txt"), "checked line\n").unwrap();
        let request = repo.message_request().unwrap();
        let RequestScan::Clean(checked) = repo.scan_request(request).unwrap() else {
            panic!("the change holds no secret");
        };
        std::fs::write(repo.root().join("a.txt"), "changed after the check\n").unwrap();

        let stub = Stub::script(vec![Answer::events(concat!(
            r#"data: {"choices": [{"index": 0, "delta": {"content": "Add a"}}]}"#,
            "\n\n",
            r#"data: {"choices": [{"index": 0, "delta": {"content": ".txt"}, "finish_reason": "stop"}]}"#,
            "\n\ndata: [DONE]\n\n",
        ))])
        .unwrap();
        let base = format!("{}/v1", stub.url());
        let source = Source::Provider {
            endpoint: Endpoint::new("stub", ProviderKind::OpenAiCompatible, "Stub", &base),
            model: "m".into(),
            key: None,
        };
        let mut pieces = Vec::new();
        let message = source
            .send(checked.text(), &Cancel::new(), &mut |piece| {
                pieces.push(piece.to_string())
            })
            .unwrap();
        assert_eq!((message.as_str(), pieces.len()), ("Add a.txt", 2));
        let seen = stub.seen();
        assert_eq!(seen.len(), 1);
        let messages = seen[0].json()["messages"].clone();
        assert_eq!(messages[0]["content"], SYSTEM);
        assert_eq!(messages[1]["content"], checked.text());
        assert!(seen[0].body.contains("checked line") && !seen[0].body.contains("changed after"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
