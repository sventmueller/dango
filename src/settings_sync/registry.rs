//! Every setting the app and its crates declare, each with whether it follows the user to their other Macs (E113).

use ide_core::settings::{Declared, Sharing};

/// The settings of each crate and of the app, as their declarations say.
const SETTINGS: &[&[Declared]] = &[
    ide_core::settings::keys::SETTINGS,
    ide_ui_agent::SETTINGS,
    ide_ui_git::SETTINGS,
    ide_ui_lsp::SETTINGS,
    #[cfg(feature = "dictation")]
    ide_ui_dictation::SETTINGS,
    &[
        super::ENABLED.declared(),
        super::FOLDER.declared(),
        crate::ci_panel::CI_HOSTS.declared(),
        crate::ci_panel::CI_OAUTH_CLIENTS.declared(),
        crate::editor::predict::model::PROVIDER.declared(),
        crate::editor::predict::model::OFF_FOR.declared(),
        crate::editor::predict::model::EXCLUDE.declared(),
        crate::editor::predict::model::OLLAMA.declared(),
        crate::editor::predict::model::LM_STUDIO.declared(),
        crate::editor::predict::model::CONSENT.declared(),
        crate::leitstand::DEFAULTS.declared(),
    ],
];

/// Every declared setting.
pub(super) fn all() -> impl Iterator<Item = Declared> {
    SETTINGS.iter().flat_map(|settings| settings.iter().copied())
}

/// Whether the setting `name` follows the user to their other Macs, as its declaration says; a name nothing declares
/// stays on this Mac.
pub(super) fn is_shared(name: &str) -> bool {
    all().any(|setting| setting.name == name && setting.sync == Sharing::Shared)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The settings that follow the user to their other Macs, as docs/phase-12.md lists them for E113.
    const SHARED: &[&str] = &[
        "agentEffort",
        "agentModel",
        "agentPrices",
        "blameAtLineEnd",
        "ciHosts",
        "ciOAuthClients",
        "commitMessageModel",
        "diagnosticsAtLineEnd",
        #[cfg(feature = "dictation")]
        "dictation",
        #[cfg(feature = "dictation")]
        "dictationWithFn",
        "editPrediction",
        "editPredictionExclude",
        "editPredictionOffFor",
        "emmet",
        "eslintFixOnSave",
        "fontSize",
        "formatOnSave",
        "formatter",
        "hiddenSidebarPanels",
        "inlayHints",
        "inlayHintsOffFor",
        "markdownPreviewBeside",
        "minimap",
        "optionAsMeta",
        "previewTabs",
        "reopenProjects",
        "searchEngine",
        "showIgnoredFiles",
        "showWhitespace",
        "stickyScrollLines",
        "vimClipboard",
        "vimMode",
    ];

    /// The settings that stay on each Mac: paths, local addresses, consents and reminders.
    const PER_MAC: &[&str] = &[
        "agentAllowedFolders",
        "agentEndpoints",
        "agentRules",
        "commitMessageConsent",
        "companionsDeclined",
        #[cfg(feature = "dictation")]
        "dictationLowspeakOffered",
        "editPredictionConsent",
        "editPredictionLmStudio",
        "editPredictionOllama",
        "factoryDefaults",
        "languageServers",
        "settingsSync",
        "settingsSyncFolder",
    ];

    fn names(sync: Sharing) -> BTreeSet<&'static str> {
        all()
            .filter(|setting| setting.sync == sync)
            .map(|setting| setting.name)
            .collect()
    }

    #[test]
    fn every_setting_is_declared_once() {
        let declared: Vec<&str> = all().map(|setting| setting.name).collect();
        let unique: BTreeSet<&str> = declared.iter().copied().collect();
        assert_eq!(unique.len(), declared.len(), "a name declared twice: {declared:?}");
        assert!(declared.iter().all(|name| !name.is_empty() && !name.contains(' ')));
    }

    #[test]
    fn the_synced_settings_are_those_of_e113() {
        assert_eq!(names(Sharing::Shared), SHARED.iter().copied().collect());
        assert_eq!(names(Sharing::PerMac), PER_MAC.iter().copied().collect());
    }

    #[test]
    fn the_sync_asks_the_declarations() {
        assert!(is_shared("fontSize"));
        assert!(is_shared("agentModel"));
        assert!(!is_shared("agentEndpoints"));
        assert!(!is_shared("settingsSync"));
        assert!(!is_shared("openProjects"), "an entry of the state file is no setting");
        assert!(!is_shared("unknownSetting"));
    }
}
