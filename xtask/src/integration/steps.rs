//! The groups of `make test-integration` and their steps.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{Action, Context, Evidence, Group, Need, Probe, Step, ci_token, remote_host};

pub(crate) const GROUPS: &[Group] = &[
    Group {
        name: "lsp",
        title: "real language servers and their installers",
        paid: false,
        uses: "downloads typescript, vtsls, rust-analyzer, gopls and the npm language servers",
        steps: lsp,
    },
    Group {
        name: "git",
        title: "checkpoints restored around the user's edits with their own git",
        paid: false,
        uses: "temporary folders only",
        steps: git,
    },
    Group {
        name: "ci",
        title: "CI runs read on github.com and on a self-managed GitLab",
        paid: false,
        uses: "reads only, with the token the app finds through gh, glab or the login shell",
        steps: ci,
    },
    Group {
        name: "dap",
        title: "real debug adapters launching and attaching to small programs",
        paid: false,
        uses: "downloads js-debug from github.com into a temporary folder",
        steps: dap,
    },
    Group {
        name: "claude",
        title: "the claude CLI and a live turn in the app",
        paid: true,
        uses: "the Claude account: two short turns with haiku",
        steps: claude,
    },
    Group {
        name: "paste",
        title: "an image pasted into each installed agent CLI",
        paid: true,
        uses: "starts claude, codex, gemini and agy with their sign-ins and sends no prompt",
        steps: paste,
    },
    Group {
        name: "acp",
        title: "ACP agents on the PATH and as pinned (E35), one tiny prompt each, recorded for replay",
        paid: true,
        uses: "the Google and OpenAI accounts behind gemini, agy and codex-acp",
        steps: acp,
    },
    Group {
        name: "predict",
        title: "edit predictions from a real fill-in-the-middle model under Ollama and LM Studio",
        paid: false,
        uses: "local models only; nothing leaves the Mac",
        steps: predict,
    },
    Group {
        name: "copilot",
        title: "edit predictions from GitHub Copilot for the signed-in user",
        paid: true,
        uses: "the Copilot account: two completions of a tiny made-up file",
        steps: copilot,
    },
];

const NPM: &str = "registry.npmjs.org";

pub(super) const CLAUDE: Need = Need::SignIn {
    service: "Claude",
    evidence: &[
        Evidence::Var("ANTHROPIC_API_KEY"),
        Evidence::Var("CLAUDE_CODE_OAUTH_TOKEN"),
        Evidence::File(".claude/.credentials.json"),
        Evidence::Keychain("Claude Code-credentials"),
    ],
};

pub(super) const GEMINI: Need = Need::SignIn {
    service: "Gemini",
    evidence: &[
        Evidence::Var("GEMINI_API_KEY"),
        Evidence::Var("GOOGLE_API_KEY"),
        Evidence::File(".gemini/oauth_creds.json"),
    ],
};

pub(super) const OPENAI: Need = Need::SignIn {
    service: "OpenAI",
    evidence: &[
        Evidence::Var("OPENAI_API_KEY"),
        Evidence::Var("CODEX_API_KEY"),
        Evidence::File(".codex/auth.json"),
    ],
};

fn step(name: &str, needs: Vec<Need>, action: Action) -> Step {
    Step {
        name: name.to_string(),
        needs,
        action,
    }
}

fn test(package: &'static str, file: &'static str, name: &'static str) -> Action {
    Action::Test {
        package,
        file,
        name,
        env: Vec::new(),
    }
}

fn selftest(scenario: &'static str, env: Vec<(String, String)>) -> Action {
    Action::Selftest { scenario, env }
}

fn lsp(_: &Context) -> Vec<Step> {
    let real = |name| test("ide-lsp", "real_servers", name);
    let node = || vec![Need::Program("node"), Need::Program("npm"), Need::Network(NPM)];
    vec![
        step(
            "gopls",
            vec![Need::Program("gopls"), Need::Program("go")],
            real("gopls_reports_diagnostics_and_answers_hover_completion_and_definition"),
        ),
        step(
            "rust-analyzer",
            vec![Need::RustAnalyzer],
            real("rust_analyzer_reports_diagnostics_and_answers_hover_completion_and_definition"),
        ),
        step(
            "TypeScript 7 through tsc --lsp",
            node(),
            real("typescript_7_serves_the_project_with_its_own_compiler"),
        ),
        step(
            "TypeScript 5 through vtsls",
            node(),
            real("an_older_typescript_falls_back_to_vtsls_installed_into_the_tools_folder"),
        ),
        step(
            "rust-analyzer installed from its release",
            vec![Need::Network("github.com")],
            real("rust_analyzer_is_installed_from_its_release_and_runs"),
        ),
        step(
            "gopls installed with go install",
            vec![Need::Program("go"), Need::Network("proxy.golang.org")],
            real("gopls_is_installed_with_go_install_into_the_tools_folder"),
        ),
        step(
            "clangd from the developer tools",
            vec![Need::DeveloperTools],
            real("clangd_from_the_developer_tools_reports_diagnostics_and_symbols"),
        ),
        step(
            "sourcekit-lsp from the developer tools",
            vec![Need::DeveloperTools],
            real("sourcekit_lsp_reports_diagnostics_in_a_swift_package"),
        ),
        step(
            "npm servers for Python, JSON, YAML, CSS, HTML and shell installed into the tools folder",
            node(),
            real("the_npm_servers_install_into_the_tools_folder_and_serve_their_languages"),
        ),
        step(
            "taplo, marksman and lua-language-server",
            vec![
                Need::Program("taplo"),
                Need::Program("marksman"),
                Need::Program("lua-language-server"),
            ],
            real("the_homebrew_servers_serve_toml_markdown_and_lua"),
        ),
        step("selftest lsp", vec![Need::RustAnalyzer], selftest("lsp", Vec::new())),
        step(
            "selftest lsp-actions",
            vec![Need::RustAnalyzer],
            selftest("lsp-actions", Vec::new()),
        ),
        step(
            "selftest lsp-actions-go",
            vec![Need::Program("gopls"), Need::Program("go")],
            selftest("lsp-actions-go", Vec::new()),
        ),
        step(
            "selftest lsp-navigation",
            vec![Need::RustAnalyzer],
            selftest("lsp-navigation", Vec::new()),
        ),
    ]
}

fn dap(_: &Context) -> Vec<Step> {
    let real = |name| test("ide-dap", "real_adapters", name);
    vec![
        step(
            "lldb-dap on a Rust program",
            vec![Need::DeveloperTools, Need::Debugging, Need::Program("cargo")],
            real("lldb_dap_launches_and_attaches_to_a_rust_program"),
        ),
        step(
            "dlv on a Go program",
            vec![Need::Program("dlv"), Need::Program("go"), Need::Debugging],
            real("dlv_launches_and_attaches_to_a_go_program"),
        ),
        step(
            "js-debug on a Node program",
            vec![Need::Program("node"), Need::Network("github.com")],
            real("js_debug_launches_and_attaches_to_a_node_program"),
        ),
    ]
}

fn git(_: &Context) -> Vec<Step> {
    vec![step(
        "checkpoint restore with an edit of the user's in between",
        vec![Need::Program("git")],
        test(
            "ide-git",
            "checkpoints",
            "the_users_own_git_restores_around_their_edits_in_a_repository_and_a_plain_folder",
        ),
    )]
}

fn ci(cx: &Context) -> Vec<Step> {
    let probe = cx.probe;
    let github = probe
        .process_var("IDE_INTEGRATION_GITHUB_REPO")
        .map_or_else(|| probe.home().join("Repos/splat"), PathBuf::from);
    let github_needs = match host_of(probe, &github) {
        Some(host) if host == "github.com" => vec![Need::CiToken { tool: "gh", host }],
        Some(host) => vec![Need::Unmet(format!(
            "{} has its origin on {host}, not github.com; set IDE_INTEGRATION_GITHUB_REPO",
            github.display()
        ))],
        None => vec![Need::Unmet(format!(
            "{} is no clone with an origin; set IDE_INTEGRATION_GITHUB_REPO",
            github.display()
        ))],
    };
    let (gitlab, gitlab_needs) = match gitlab_clone(probe) {
        Ok((repository, host)) => (repository, vec![Need::CiToken { tool: "glab", host }]),
        Err(why) => (PathBuf::new(), vec![Need::Unmet(why)]),
    };
    vec![
        step("GitHub Actions through gh", github_needs, read_runs(&github)),
        step("GitLab self-managed through glab", gitlab_needs, read_runs(&gitlab)),
    ]
}

fn read_runs(repository: &Path) -> Action {
    selftest(
        "ci-live",
        vec![("IDE_CI_LIVE_REPO".into(), repository.display().to_string())],
    )
}

fn host_of(probe: &dyn Probe, repository: &Path) -> Option<String> {
    remote_host(&probe.origin(repository)?)
}

fn self_managed(host: &str) -> bool {
    host != "github.com" && host != "gitlab.com"
}

/// The clone whose CI on a self-managed GitLab is read, with its host: `IDE_INTEGRATION_GITLAB_REPO`, else the first
/// clone in `~/Repos` whose origin is on a host other than github.com and gitlab.com that the app finds a token for.
fn gitlab_clone(probe: &dyn Probe) -> Result<(PathBuf, String), String> {
    if let Some(given) = probe.process_var("IDE_INTEGRATION_GITLAB_REPO") {
        let repository = PathBuf::from(given);
        return match host_of(probe, &repository) {
            Some(host) if self_managed(&host) => Ok((repository, host)),
            Some(host) => Err(format!(
                "{} has its origin on {host}, not on a self-managed GitLab",
                repository.display()
            )),
            None => Err(format!("{} is no clone with an origin", repository.display())),
        };
    }
    let mut found = HashMap::new();
    for repository in probe.clones() {
        let Some(host) = host_of(probe, &repository).filter(|host| self_managed(host)) else {
            continue;
        };
        let token = *found
            .entry(host.clone())
            .or_insert_with(|| ci_token("glab", &host, probe));
        if token {
            return Ok((repository, host));
        }
    }
    Err(
        "no clone in ~/Repos has its origin on a self-managed GitLab the app's CI finds a token for, through glab or \
         GITLAB_HOST and GITLAB_TOKEN; set IDE_INTEGRATION_GITLAB_REPO"
            .into(),
    )
}

fn claude(cx: &Context) -> Vec<Step> {
    let probe = cx.probe;
    let home = probe.home();
    let cli = probe
        .process_var("CLAUDE_BIN")
        .map_or_else(|| home.join(".local/bin/claude"), PathBuf::from);
    let cli_need = (!probe.exists(&cli)).then(|| Need::Unmet(format!("{} is missing; set CLAUDE_BIN", cli.display())));
    // The places `agent::source::find_claude` looks after the PATH.
    let fallbacks = [
        home.join(".local/bin/claude"),
        home.join(".claude/local/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
    ];
    let found = probe.program("claude").is_some() || fallbacks.iter().any(|path| probe.exists(path));
    let app_need =
        (!found).then(|| Need::Unmet("the app finds no claude, on the PATH or where its installers put it".into()));
    vec![
        step(
            "the claude CLI answers a prompt",
            cli_need.into_iter().chain([CLAUDE]).collect(),
            test("ide-claude", "session", "the_real_cli_answers_a_prompt"),
        ),
        step(
            "selftest agent-live",
            app_need.into_iter().chain([CLAUDE]).collect(),
            selftest("agent-live", Vec::new()),
        ),
    ]
}

fn paste(_: &Context) -> Vec<Step> {
    vec![step(
        "selftest paste-cli",
        vec![Need::AnyProgram(&["claude", "codex", "gemini", "agy"])],
        selftest("paste-cli", Vec::new()),
    )]
}

fn acp(cx: &Context) -> Vec<Step> {
    let recorded = |name| Action::Test {
        package: "ide-acp",
        file: "live",
        name,
        env: vec![("IDE_ACP_RECORDINGS".into(), cx.recordings.display().to_string())],
    };
    let pinned_npm = |sign_in| vec![Need::Program("node"), Need::Program("npm"), Need::Network(NPM), sign_in];
    vec![
        step(
            "gemini --acp on the PATH",
            vec![Need::Program("gemini"), GEMINI],
            recorded("gemini_on_the_path_answers_a_tiny_prompt"),
        ),
        step(
            "Gemini CLI as pinned",
            pinned_npm(GEMINI),
            recorded("gemini_as_pinned_answers_a_tiny_prompt"),
        ),
        step(
            "agy acp on the PATH",
            vec![Need::Program("agy")],
            recorded("antigravity_on_the_path_answers_a_tiny_prompt"),
        ),
        step(
            "Antigravity as pinned",
            vec![Need::Network("dl.google.com")],
            recorded("antigravity_as_pinned_answers_a_tiny_prompt"),
        ),
        step(
            "codex-acp on the PATH",
            vec![Need::Program("codex-acp"), OPENAI],
            recorded("codex_on_the_path_answers_a_tiny_prompt"),
        ),
        step(
            "codex-acp as pinned",
            pinned_npm(OPENAI),
            recorded("codex_as_pinned_answers_a_tiny_prompt"),
        ),
    ]
}

fn predict(cx: &Context) -> Vec<Step> {
    let probe = cx.probe;
    let real = |name| test("ide-predict", "real_providers", name);
    let ollama_model = probe
        .process_var("IDE_INTEGRATION_OLLAMA_MODEL")
        .unwrap_or_else(|| "qwen2.5-coder:1.5b".into());
    let ollama_needs = match probe.program("ollama") {
        None => vec![Need::Program("ollama")],
        Some(ollama) if probe.succeeds(&ollama, &["show", &ollama_model]) => Vec::new(),
        Some(_) => vec![Need::Unmet(format!(
            "Ollama does not run or lacks {ollama_model}: ollama pull {ollama_model}, or set IDE_INTEGRATION_OLLAMA_MODEL"
        ))],
    };
    vec![
        step(
            "Ollama fills a gap with a real model",
            ollama_needs,
            real("ollama_fills_the_gap_with_a_real_model"),
        ),
        step(
            "LM Studio fills a gap with a real model",
            vec![Need::Program("lms")],
            real("lm_studio_fills_the_gap_with_a_real_model"),
        ),
    ]
}

/// Where copilot-language-server keeps the user's sign-in.
const COPILOT: Need = Need::SignIn {
    service: "GitHub Copilot",
    evidence: &[
        Evidence::File(".config/github-copilot/apps.json"),
        Evidence::File(".config/github-copilot/hosts.json"),
    ],
};

fn copilot(_: &Context) -> Vec<Step> {
    vec![step(
        "Copilot proposes a completion",
        vec![
            Need::Program("node"),
            Need::Program("npm"),
            Need::Network(NPM),
            Need::Network("api.githubcopilot.com"),
            COPILOT,
        ],
        test(
            "ide-predict",
            "real_providers",
            "copilot_proposes_a_completion_for_the_signed_in_user",
        ),
    )]
}
