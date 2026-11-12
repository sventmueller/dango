//! The app bundle: its property list, its XPC services, Sparkle for the distribution, its entitlements and its
//! signature.

use std::path::{Path, PathBuf};
use std::process::Command;

use ide_product::{BUNDLE_ID, NAME, URL_SCHEME};

use crate::*;

pub(crate) const MINIMUM_SYSTEM: &str = "14.0";

const PLIST_HEAD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
"#;

/// Which app a bundle holds. Debug and selftest builds are apps of their own: they keep their settings, data and URL
/// scheme apart from the app in use, and macOS cannot mistake an outdated debug build for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Build {
    /// Built for this Mac without optimisation.
    Debug,
    /// Built for this Mac as it ships, signed with the local certificate.
    Release,
    /// The app with its selftest built in, optimised or not, and with dictation for the scenarios that need it.
    Selftest { release: bool, dictation: bool },
    /// The universal release with Sparkle, for the disk image and Homebrew; the same app as `Release` to macOS.
    Distribution,
}

impl Build {
    pub(crate) fn of(release: bool, selftest: bool) -> Build {
        match (selftest, release) {
            (true, release) => Build::Selftest {
                release,
                dictation: false,
            },
            (false, true) => Build::Release,
            (false, false) => Build::Debug,
        }
    }

    pub(crate) fn release(self) -> bool {
        match self {
            Build::Debug => false,
            Build::Release | Build::Distribution => true,
            Build::Selftest { release, .. } => release,
        }
    }

    /// Only the disk image carries Sparkle: an update must not replace a build of one's own.
    pub(crate) fn updates(self) -> bool {
        self == Build::Distribution
    }

    /// The folder under `target/` the bundle lands in.
    pub(crate) fn folder(self) -> &'static str {
        match self {
            Build::Distribution => "distribution",
            _ => self.profile(),
        }
    }

    /// Whether the program has dictation, which builds whisper.cpp; every app build has it, only some scenarios do.
    pub(crate) fn dictation(self) -> bool {
        match self {
            Build::Debug | Build::Release | Build::Distribution => true,
            Build::Selftest { dictation, .. } => dictation,
        }
    }

    /// The cargo features the build turns on.
    pub(crate) fn features(self) -> Vec<&'static str> {
        [(self.selftest(), "selftest"), (self.dictation(), "dictation")]
            .into_iter()
            .filter_map(|(on, feature)| on.then_some(feature))
            .collect()
    }

    pub(crate) fn selftest(self) -> bool {
        matches!(self, Build::Selftest { .. })
    }

    /// The cargo profile, which is also the folder under `target/` the bundle lands in.
    pub(crate) fn profile(self) -> &'static str {
        if self.release() { "release" } else { "debug" }
    }

    /// The word behind the app's name, which keeps the builds apart in the Finder and in Launch Services.
    pub(crate) fn variant(self) -> Option<&'static str> {
        match self {
            Build::Debug => Some("Debug"),
            Build::Release | Build::Distribution => None,
            Build::Selftest { .. } => Some("Selftest"),
        }
    }

    /// The bundle identifier, which also names the build's defaults domain, keychain service and data folders.
    pub(crate) fn bundle_id(self) -> String {
        match self.variant() {
            Some(variant) => format!("{BUNDLE_ID}.{}", variant.to_lowercase()),
            None => BUNDLE_ID.to_string(),
        }
    }

    /// The URL scheme the build answers. Launch Services hands a URL to any one of the apps that claim its scheme, so
    /// a debug build has a scheme of its own and the selftest, which lives only as long as a scenario, claims none.
    pub(crate) fn url_scheme(self) -> Option<String> {
        match self {
            Build::Debug => Some(format!("{URL_SCHEME}-debug")),
            Build::Release | Build::Distribution => Some(URL_SCHEME.to_string()),
            Build::Selftest { .. } => None,
        }
    }

    /// The bundle's file name.
    pub(crate) fn app_name(self) -> String {
        match self.variant() {
            Some(variant) => format!("{NAME} {variant}.app"),
            None => format!("{NAME}.app"),
        }
    }

    /// Where the bundle is built.
    pub(crate) fn app(self) -> PathBuf {
        target_dir().join(self.folder()).join(self.app_name())
    }
}

/// A helper program the app starts as an XPC service, from `Contents/XPCServices/<name>.xpc`.
pub(crate) struct Service {
    /// The bundle's name, which the app looks the service up by.
    pub(crate) name: &'static str,
    /// The program of the app's package that the service runs.
    pub(crate) program: &'static str,
    /// The entitlements it is signed with, each set to true.
    pub(crate) entitlements: &'static [&'static str],
}

/// The app's XPC services, signed before the app. A service that reads untrusted data, such as a decoder, runs in
/// the App Sandbox, which the app itself never does (E2): `entitlements: &["com.apple.security.app-sandbox"]`.
pub(crate) const SERVICES: &[Service] = &[Service {
    // Decodes the pictures of the previews (docs/splat-render.md); the app finds it as its identifier with `.decoder`.
    name: "Decoder",
    program: "ide-decoder",
    entitlements: &["com.apple.security.app-sandbox"],
}];

impl Service {
    pub(crate) fn bundle(&self, app: &Path) -> PathBuf {
        app.join("Contents/XPCServices").join(format!("{}.xpc", self.name))
    }

    pub(crate) fn executable(&self, app: &Path) -> PathBuf {
        self.bundle(app).join("Contents/MacOS").join(self.program)
    }

    /// The app's identifier with the service's name, in the build's variant.
    pub(crate) fn bundle_id(&self, build: Build) -> String {
        format!("{}.{}", build.bundle_id(), self.name.to_lowercase())
    }

    pub(crate) fn info_plist(&self, build: Build, version: &str) -> String {
        let (name, program, bundle_id) = (self.name, self.program, self.bundle_id(build));
        format!(
            r#"{PLIST_HEAD}<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleExecutable</key>
	<string>{program}</string>
	<key>CFBundleIdentifier</key>
	<string>{bundle_id}</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>{name}</string>
	<key>CFBundlePackageType</key>
	<string>XPC!</string>
	<key>CFBundleShortVersionString</key>
	<string>{version}</string>
	<key>CFBundleVersion</key>
	<string>{version}</string>
	<key>LSMinimumSystemVersion</key>
	<string>{MINIMUM_SYSTEM}</string>
	<key>XPCService</key>
	<dict>
		<key>ServiceType</key>
		<string>Application</string>
	</dict>
</dict>
</plist>
"#
        )
    }
}

/// What of splat's resources the hidden renderer of formulas and diagrams needs: its page, and the libraries beside it
/// without those of splat's formula field.
const RENDER_PAGE: &str = "render.html";
pub(crate) const FORMULA_FIELD: [&str; 4] = [
    "mathlive.min.js",
    "mathlive-fonts",
    "mathfield-page.js",
    "LICENSE-mathlive",
];

/// Copies the renderer's page and libraries from splat's `resources` into the bundle's `resources`.
pub(crate) fn copy_render_page(splat: &Path, resources: &Path) -> Result<(), String> {
    files::copy(splat.join(RENDER_PAGE), resources.join(RENDER_PAGE))?;
    let (from, to) = (splat.join("js"), resources.join("js"));
    files::create_dir_all(&to)?;
    for entry in files::read_dir(&from)? {
        let entry = entry.map_err(|e| format!("{}: {e}", from.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || FORMULA_FIELD.contains(&name.as_str()) {
            continue;
        }
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &to.join(&name))?;
        } else {
            files::copy(entry.path(), to.join(&name))?;
        }
    }
    Ok(())
}

/// The program inside a bundle.
pub(crate) fn executable(app: &Path) -> PathBuf {
    app.join("Contents/MacOS").join(NAME)
}

/// The entitlements a build is signed with, each set to true. Never the App Sandbox: the terminal, agents, git and
/// language servers run as child processes, which would inherit it (E2).
pub(crate) fn entitlements(build: Build) -> &'static [&'static str] {
    if build.dictation() {
        // The hardened runtime keeps the microphone closed without it.
        &["com.apple.security.device.audio-input"]
    } else {
        &[]
    }
}

pub(crate) fn entitlements_plist(keys: &[&str]) -> String {
    let keys: String = keys
        .iter()
        .map(|key| format!("\t<key>{key}</key>\n\t<true/>\n"))
        .collect();
    format!("{PLIST_HEAD}<dict>\n{keys}</dict>\n</plist>\n")
}

/// Folders and text files the app can open, from the Finder, the Dock and the `open` command. The rank `None` keeps it
/// from becoming the default app for any of them.
const DOCUMENT_TYPES: &str = "\t<key>CFBundleDocumentTypes</key>\n\t<array>\n\t\t<dict>\n\
\t\t\t<key>CFBundleTypeName</key>\n\t\t\t<string>Folder</string>\n\
\t\t\t<key>CFBundleTypeRole</key>\n\t\t\t<string>Editor</string>\n\
\t\t\t<key>LSHandlerRank</key>\n\t\t\t<string>None</string>\n\
\t\t\t<key>LSItemContentTypes</key>\n\t\t\t<array>\n\t\t\t\t<string>public.folder</string>\n\t\t\t</array>\n\
\t\t</dict>\n\t\t<dict>\n\
\t\t\t<key>CFBundleTypeName</key>\n\t\t\t<string>Text</string>\n\
\t\t\t<key>CFBundleTypeRole</key>\n\t\t\t<string>Editor</string>\n\
\t\t\t<key>LSHandlerRank</key>\n\t\t\t<string>None</string>\n\
\t\t\t<key>LSItemContentTypes</key>\n\t\t\t<array>\n\t\t\t\t<string>public.text</string>\n\
\t\t\t\t<string>public.source-code</string>\n\t\t\t\t<string>public.data</string>\n\t\t\t</array>\n\
\t\t</dict>\n\t</array>\n";

/// App Transport Security applies to web views too and would refuse every plain-HTTP site in the browser tab; this
/// lifts it for web views only, so the app's own connections keep it.
/// Why the app asks for the microphone, shown in macOS's question; translated with the other texts in the release.
const MICROPHONE_USE: &str = "Dictation turns what you say into text in the editor and in the composer of AI tabs.";

const WEB_CONTENT_TRANSPORT: &str = "\t<key>NSAppTransportSecurity</key>\n\t<dict>\n\
\t\t<key>NSAllowsArbitraryLoadsInWebContent</key>\n\t\t<true/>\n\t</dict>\n";

/// The property list of the app; `feed` is what the distribution's Sparkle reads.
pub(crate) fn info_plist(build: Build, version: &str, languages: &[String], feed: Option<&updates::Feed>) -> String {
    let bundle_id = build.bundle_id();
    let updates = feed.filter(|_| build.updates()).map_or_else(String::new, |feed| {
        format!(
            "\t<key>SUFeedURL</key>\n\t<string>{}</string>\n\t<key>SUPublicEDKey</key>\n\t<string>{}</string>\n",
            feed.url, feed.key
        )
    });
    let localizations: String = languages
        .iter()
        .map(|language| format!("\t\t<string>{language}</string>\n"))
        .collect();
    let microphone = if build.dictation() {
        format!("\t<key>NSMicrophoneUsageDescription</key>\n\t<string>{MICROPHONE_USE}</string>\n")
    } else {
        String::new()
    };
    let url_types = build.url_scheme().map_or_else(String::new, |scheme| {
        format!(
            "\t<key>CFBundleURLTypes</key>\n\t<array>\n\t\t<dict>\n\t\t\t<key>CFBundleURLName</key>\n\
             \t\t\t<string>{bundle_id}</string>\n\t\t\t<key>CFBundleURLSchemes</key>\n\t\t\t<array>\n\
             \t\t\t\t<string>{scheme}</string>\n\t\t\t</array>\n\t\t</dict>\n\t</array>\n"
        )
    });
    format!(
        r#"{PLIST_HEAD}<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleLocalizations</key>
	<array>
{localizations}	</array>
	<key>CFBundleExecutable</key>
	<string>{NAME}</string>
	<key>CFBundleIdentifier</key>
	<string>{bundle_id}</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>{NAME}</string>
	<key>CFBundleDisplayName</key>
	<string>{NAME}</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>{version}</string>
	<key>CFBundleVersion</key>
	<string>{version}</string>
	<key>CFBundleIconFile</key>
	<string>{ICON_NAME}</string>
	<key>CFBundleIconName</key>
	<string>{ICON_NAME}</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.developer-tools</string>
	<key>LSMinimumSystemVersion</key>
	<string>{MINIMUM_SYSTEM}</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSPrincipalClass</key>
	<string>NSApplication</string>
	<key>NSHumanReadableCopyright</key>
	<string>© 2026 Sven Müller</string>
	<key>ITSAppUsesNonExemptEncryption</key>
	<false/>
{WEB_CONTENT_TRANSPORT}{DOCUMENT_TYPES}{url_types}{microphone}{updates}</dict>
</plist>
"#
    )
}

/// The localizations a bundle declares: one per `<language>.lproj` in `resources`, sorted.
pub(crate) fn languages(resources: &Path) -> Result<Vec<String>, String> {
    if !resources.exists() {
        return Ok(Vec::new());
    }
    let mut languages: Vec<String> = files::read_dir(resources)?
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().to_str()?.strip_suffix(".lproj").map(str::to_string))
        .collect();
    languages.sort();
    Ok(languages)
}

/// Copies a folder, without the hidden files the Finder leaves in it.
pub(crate) fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    files::create_dir_all(to)?;
    for entry in files::read_dir(from)? {
        let entry = entry.map_err(|e| format!("{}: {e}", from.display()))?;
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            files::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Builds and signs a bundle for this Mac. A release build then raises the build number; debug and selftest builds
/// leave it.
pub(crate) fn bundle(build: Build) -> Result<PathBuf, String> {
    let version = version()?;
    let app = assemble(build, &version, None)?;
    sign(&app, &signing_identity()?, build, false)?;
    if build == Build::Release {
        next_build(&version)?;
    }
    Ok(app)
}

/// Builds the programs and lays out the bundle around them, unsigned. The distribution carries the universal programs,
/// Sparkle reading `feed` and the license texts of everything in it; the other builds the programs for this Mac.
pub(crate) fn assemble(build: Build, version: &str, feed: Option<&updates::Feed>) -> Result<PathBuf, String> {
    let root = root();
    let programs = programs(build)?;
    let app = build.app();
    if app.exists() {
        files::remove_dir_all(&app)?;
    }
    let contents = app.join("Contents");
    let resources = contents.join("Resources");
    files::create_dir_all(contents.join("MacOS"))?;
    files::create_dir_all(&resources)?;
    files::copy(&programs[0], executable(&app))?;
    for (service, program) in SERVICES.iter().zip(&programs[1..]) {
        files::create_dir_all(service.executable(&app).parent().ok_or("no MacOS folder")?)?;
        files::copy(program, service.executable(&app))?;
        let plist = service.bundle(&app).join("Contents/Info.plist");
        files::write(&plist, service.info_plist(build, version))?;
        run(Command::new("plutil").args(["-lint", "-s"]).arg(&plist))?;
    }
    let source = root.join("resources");
    if source.exists() {
        copy_dir(&source, &resources)?;
    }
    copy_render_page(&root.join("../splat/resources"), &resources)?;
    copy_icon(&resources)?;
    let plist = contents.join("Info.plist");
    files::write(&plist, info_plist(build, version, &languages(&source)?, feed))?;
    run(Command::new("plutil").args(["-lint", "-s"]).arg(&plist))?;
    files::write(contents.join("PkgInfo"), "APPL????")?;
    if build.updates() {
        let sparkle = updates::sparkle()?;
        updates::embed_sparkle(&app, &sparkle)?;
        files::write(
            resources.join(ACKNOWLEDGEMENTS),
            acknowledgements(&build.features(), Some(&sparkle))?,
        )?;
    }
    Ok(app)
}

/// The app's program and its services' programs, built for the build: the distribution's universal, the others' for
/// this Mac.
fn programs(build: Build) -> Result<Vec<PathBuf>, String> {
    let names: Vec<&str> = std::iter::once(PROGRAM)
        .chain(SERVICES.iter().map(|service| service.program))
        .collect();
    let features = build.features();
    if build == Build::Distribution {
        return universal_programs(&names, &features);
    }
    let mut cargo = Command::new(env!("CARGO"));
    cargo.current_dir(root()).args(["build", "--package", PACKAGE]);
    for name in &names {
        cargo.args(["--bin", name]);
    }
    if build.release() {
        cargo.arg("--release");
    }
    if !features.is_empty() {
        cargo.args(["--features", &features.join(",")]);
    }
    run(&mut cargo)?;
    Ok(names
        .iter()
        .map(|name| target_dir().join(build.profile()).join(name))
        .collect())
}

/// Signs with the hardened runtime, inside out as Sparkle's documentation prescribes instead of `--deep`: the services
/// and Sparkle's helpers with their own entitlements first, the app with the build's last; then has codesign check the
/// result. Notarization needs the secure timestamp, a request to Apple that local builds skip.
pub(crate) fn sign(app: &Path, identity: &str, build: Build, timestamp: bool) -> Result<(), String> {
    let mut parts: Vec<(PathBuf, &[&str])> = SERVICES
        .iter()
        .map(|service| (service.bundle(app), service.entitlements))
        .collect();
    if updates::framework(app).exists() {
        let code = updates::framework(app).join("Versions/B");
        parts.extend([
            (code.join("Autoupdate"), &[] as &[&str]),
            (code.join("Updater.app"), &[]),
            (updates::framework(app), &[]),
        ]);
    }
    parts.push((app.to_path_buf(), entitlements(build)));
    for (index, (part, keys)) in parts.iter().enumerate() {
        let file = target_dir().join(format!("{}-{index}.entitlements", build.bundle_id()));
        let mut codesign = Command::new("codesign");
        codesign
            .args(["--force", "--sign", identity, "--options", "runtime"])
            .arg(if timestamp { "--timestamp" } else { "--timestamp=none" });
        if part.as_path() == app || !keys.is_empty() {
            files::write(&file, entitlements_plist(keys))?;
            codesign.arg("--entitlements").arg(&file);
        }
        run(codesign.arg(part))?;
    }
    run(Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(app))
}

/// The name of the local signing certificate. macOS ties granted permissions, such as folder access and the keychain's
/// "Always Allow", to the app's signature; with a certificate of its own, unlike an ad hoc signature, the app stays the
/// same app across rebuilds.
pub(crate) fn identity_name() -> String {
    format!("{BUNDLE_ID} Local Signing")
}

/// The local signing certificate's hash, which codesign takes in place of its name; the certificate is created in the
/// login keychain on first use.
pub(crate) fn signing_identity() -> Result<String, String> {
    let name = identity_name();
    if let Some(hash) = find_identity(&name)? {
        return Ok(hash);
    }
    let dir = std::env::temp_dir().join(format!("{BUNDLE_ID}-signing-{}", std::process::id()));
    files::create_private_dir(&dir)?;
    let created = create_identity(&dir, &name);
    let _ = std::fs::remove_dir_all(&dir);
    created?;
    find_identity(&name)?.ok_or_else(|| format!("the new certificate {name:?} cannot sign code"))
}

fn find_identity(name: &str) -> Result<Option<String>, String> {
    let listed = stdout(Command::new("security").args(["find-identity", "-p", "codesigning"]))?;
    Ok(identity_hash(&listed, name).map(str::to_string))
}

/// The hash of the first identity called `name` in the output of `security find-identity`. Signing by hash keeps
/// working when the keychain holds two certificates of that name, where codesign refuses the name as ambiguous.
pub(crate) fn identity_hash<'a>(listed: &'a str, name: &str) -> Option<&'a str> {
    let quoted = format!("\"{name}\"");
    listed.lines().find_map(|line| {
        let (_, entry) = line.trim().split_once(") ")?;
        let (hash, label) = entry.split_once(' ')?;
        (label == quoted || label.starts_with(&format!("{quoted} "))).then_some(hash)
    })
}

/// Creates a self-signed code signing certificate in the login keychain, working in `dir`.
pub(crate) fn create_identity(dir: &Path, name: &str) -> Result<(), String> {
    let (key, cert, p12) = (dir.join("key.pem"), dir.join("cert.pem"), dir.join("identity.p12"));
    let password = format!(
        "{}{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    );
    // The system's LibreSSL, since `security import` only reads PKCS#12 files with the older ciphers it writes.
    let openssl = "/usr/bin/openssl";
    run(Command::new(openssl)
        .args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "7300"])
        .args(["-subj", &format!("/CN={name}")])
        .args(["-addext", "extendedKeyUsage=critical,codeSigning"])
        .args(["-addext", "keyUsage=critical,digitalSignature"])
        .arg("-keyout")
        .arg(&key)
        .arg("-out")
        .arg(&cert)
        .stderr(std::process::Stdio::null()))?;
    run(Command::new(openssl)
        .args(["pkcs12", "-export", "-inkey"])
        .arg(&key)
        .arg("-in")
        .arg(&cert)
        .arg("-out")
        .arg(&p12)
        .args(["-passout", &format!("pass:{password}")]))?;
    run(Command::new("security")
        .arg("import")
        .arg(&p12)
        .args(["-P", &password, "-T", "/usr/bin/codesign"]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILDS: [Build; 4] = [
        Build::Debug,
        Build::Release,
        Build::Selftest {
            release: false,
            dictation: false,
        },
        Build::Selftest {
            release: true,
            dictation: false,
        },
    ];

    const DICTATING_SELFTEST: Build = Build::Selftest {
        release: true,
        dictation: true,
    };

    fn de_en() -> Vec<String> {
        vec!["de".to_string(), "en".to_string()]
    }

    fn value_of(plist: &str, key: &str) -> String {
        let after = plist
            .split(&format!("<key>{key}</key>\n"))
            .nth(1)
            .unwrap_or_else(|| panic!("no {key}"));
        after.lines().next().unwrap().trim().to_string()
    }

    #[test]
    fn each_build_has_its_profile_bundle_and_identifier() {
        let seen: Vec<_> = BUILDS
            .iter()
            .map(|b| (b.profile(), b.app_name(), b.bundle_id()))
            .collect();
        assert_eq!(
            seen,
            [
                ("debug", format!("{NAME} Debug.app"), format!("{BUNDLE_ID}.debug")),
                ("release", format!("{NAME}.app"), BUNDLE_ID.to_string()),
                ("debug", format!("{NAME} Selftest.app"), format!("{BUNDLE_ID}.selftest")),
                (
                    "release",
                    format!("{NAME} Selftest.app"),
                    format!("{BUNDLE_ID}.selftest")
                ),
            ]
        );
        for build in BUILDS {
            assert!(build.app().ends_with(Path::new(build.profile()).join(build.app_name())));
            assert!(!build.bundle_id().to_lowercase().contains(&NAME.to_lowercase()));
        }
        assert_eq!(
            executable(&Build::Debug.app()),
            Build::Debug.app().join("Contents/MacOS").join(NAME)
        );
    }

    #[test]
    fn the_distribution_is_the_release_app_with_sparkle_in_a_folder_of_its_own() {
        let distribution = Build::Distribution;
        assert_eq!(distribution.bundle_id(), Build::Release.bundle_id());
        assert_eq!(distribution.app_name(), Build::Release.app_name());
        assert_eq!(distribution.url_scheme(), Build::Release.url_scheme());
        assert!(distribution.release() && distribution.updates() && !Build::Release.updates());
        assert!(
            distribution
                .app()
                .ends_with(Path::new("distribution").join(distribution.app_name()))
        );
        assert_ne!(distribution.app(), Build::Release.app());
        let feed = updates::Feed {
            url: "https://h.example/latest/download/appcast.xml",
            key: "a2V5",
        };
        let plist = info_plist(distribution, "1.2.3", &de_en(), Some(&feed));
        assert_eq!(value_of(&plist, "SUFeedURL"), format!("<string>{}</string>", feed.url));
        assert_eq!(value_of(&plist, "SUPublicEDKey"), "<string>a2V5</string>");
        assert!(
            !plist.contains("SUEnableInstallerLauncherService"),
            "only a sandboxed app needs it"
        );
        for build in BUILDS {
            assert!(!info_plist(build, "1.2.3", &de_en(), Some(&feed)).contains("SUFeedURL"));
        }
        assert_eq!(distribution.features(), ["dictation"], "E46: the release dictates");
        assert_eq!(entitlements(distribution), entitlements(Build::Release));
        assert!(
            !entitlements_plist(entitlements(distribution)).contains("app-sandbox"),
            "E2: no App Sandbox"
        );
        assert!(plist.contains("<key>NSMicrophoneUsageDescription</key>"));
    }

    #[test]
    fn a_service_lies_in_the_app_and_carries_its_own_identifier() {
        let service = Service {
            name: "Decoder",
            program: "ide-decoder",
            entitlements: &["com.apple.security.app-sandbox"],
        };
        let app = Build::Release.app();
        assert_eq!(
            service.executable(&app),
            app.join("Contents/XPCServices/Decoder.xpc/Contents/MacOS/ide-decoder")
        );
        assert_eq!(service.bundle_id(Build::Debug), format!("{BUNDLE_ID}.debug.decoder"));
        let plist = service.info_plist(Build::Release, "1.2.3");
        assert_eq!(value_of(&plist, "CFBundlePackageType"), "<string>XPC!</string>");
        assert_eq!(value_of(&plist, "CFBundleExecutable"), "<string>ide-decoder</string>");
        assert_eq!(
            value_of(&plist, "CFBundleIdentifier"),
            format!("<string>{BUNDLE_ID}.decoder</string>")
        );
        assert!(entitlements_plist(service.entitlements).contains("app-sandbox"));
        for service in SERVICES {
            assert!(!service.name.contains(' ') && !service.program.is_empty());
        }
    }

    #[test]
    fn the_render_page_comes_without_the_formula_field() {
        let dir = std::env::temp_dir().join(format!("xtask-render-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (splat, resources) = (dir.join("splat"), dir.join("bundle"));
        for folder in ["js/mathjax", "js/mathlive-fonts"] {
            std::fs::create_dir_all(splat.join(folder)).unwrap();
        }
        std::fs::create_dir_all(&resources).unwrap();
        for file in [
            "render.html",
            "mathfield.html",
            "js/render.js",
            "js/mermaid.min.js",
            "js/LICENSE-mermaid",
            "js/mathjax/tex.js",
            "js/mathlive.min.js",
            "js/mathfield-page.js",
            "js/LICENSE-mathlive",
            "js/mathlive-fonts/a.woff2",
            "js/.DS_Store",
        ] {
            std::fs::write(splat.join(file), file).unwrap();
        }
        let copied = copy_render_page(&splat, &resources);
        let mut found: Vec<String> = Vec::new();
        let mut pending = vec![resources.clone()];
        while let Some(at) = pending.pop() {
            for entry in std::fs::read_dir(&at).unwrap().flatten() {
                if entry.path().is_dir() {
                    pending.push(entry.path());
                } else {
                    found.push(entry.path().strip_prefix(&resources).unwrap().display().to_string());
                }
            }
        }
        found.sort();
        let _ = std::fs::remove_dir_all(&dir);
        copied.unwrap();
        assert_eq!(
            found,
            [
                "js/LICENSE-mermaid",
                "js/mathjax/tex.js",
                "js/mermaid.min.js",
                "js/render.js",
                "render.html"
            ]
        );
    }

    #[test]
    fn options_choose_the_build() {
        assert_eq!(Build::of(false, false), Build::Debug);
        assert_eq!(Build::of(true, false), Build::Release);
        assert_eq!(
            Build::of(false, true),
            Build::Selftest {
                release: false,
                dictation: false
            }
        );
        assert_eq!(Build::of(true, true), BUILDS[3]);
    }

    #[test]
    fn dictation_is_in_every_app_build_and_where_a_scenario_asks_for_it() {
        assert_eq!(Build::Debug.features(), ["dictation"]);
        assert_eq!(Build::Release.features(), ["dictation"]);
        assert_eq!(BUILDS[3].features(), ["selftest"]);
        assert_eq!(DICTATING_SELFTEST.features(), ["selftest", "dictation"]);
        for build in BUILDS.into_iter().chain([DICTATING_SELFTEST]) {
            let plist = info_plist(build, "1.2.3", &de_en(), None);
            let entitlements = entitlements_plist(entitlements(build));
            assert_eq!(
                plist.contains("<key>NSMicrophoneUsageDescription</key>"),
                build.dictation(),
                "{build:?}"
            );
            assert_eq!(
                entitlements.contains("com.apple.security.device.audio-input"),
                build.dictation(),
                "{build:?}"
            );
        }
    }

    #[test]
    fn the_property_list_names_the_program_the_build_and_the_version() {
        for build in BUILDS {
            let plist = info_plist(build, "1.2.3", &de_en(), None);
            assert_eq!(
                value_of(&plist, "CFBundleIdentifier"),
                format!("<string>{}</string>", build.bundle_id())
            );
            for key in ["CFBundleExecutable", "CFBundleName", "CFBundleDisplayName"] {
                assert_eq!(value_of(&plist, key), format!("<string>{NAME}</string>"));
            }
            for key in ["CFBundleShortVersionString", "CFBundleVersion"] {
                assert_eq!(value_of(&plist, key), "<string>1.2.3</string>");
            }
            assert_eq!(value_of(&plist, "CFBundleDevelopmentRegion"), "<string>en</string>");
            for key in ["CFBundleIconFile", "CFBundleIconName"] {
                assert_eq!(value_of(&plist, key), "<string>AppIcon</string>");
            }
            assert_eq!(value_of(&plist, "LSMinimumSystemVersion"), "<string>14.0</string>");
            assert_eq!(value_of(&plist, "ITSAppUsesNonExemptEncryption"), "<false/>");
            assert_eq!(value_of(&plist, "NSAllowsArbitraryLoadsInWebContent"), "<true/>");
            assert!(!plist.contains("<key>NSAllowsArbitraryLoads</key>"));
            assert!(plist.contains(
                "<key>CFBundleLocalizations</key>\n\t<array>\n\t\t<string>de</string>\n\t\t<string>en</string>\n\t</array>"
            ));
            assert!(plist.contains("<string>public.folder</string>"));
            assert_eq!(plist.contains("UsageDescription"), build.dictation());
            assert!(
                !plist.contains("<string>Default</string>"),
                "the app never claims a type for good"
            );
        }
    }

    #[test]
    fn only_the_release_answers_the_products_url_scheme_and_the_selftest_none() {
        let schemes = |build| {
            let plist = info_plist(build, "1.2.3", &de_en(), None);
            plist
                .split("<key>CFBundleURLSchemes</key>\n\t\t\t<array>\n")
                .skip(1)
                .map(|rest| rest.lines().next().unwrap().trim().to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(schemes(Build::Release), [format!("<string>{URL_SCHEME}</string>")]);
        assert_eq!(schemes(Build::Debug), [format!("<string>{URL_SCHEME}-debug</string>")]);
        assert!(schemes(BUILDS[3]).is_empty());
        assert!(!info_plist(BUILDS[3], "1.2.3", &[], None).contains("CFBundleURLTypes"));
    }

    #[test]
    fn the_localizations_are_the_language_folders_of_the_resources() {
        let dir = std::env::temp_dir().join(format!("xtask-languages-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for folder in ["en.lproj", "de.lproj", "images"] {
            std::fs::create_dir_all(dir.join(folder)).unwrap();
        }
        std::fs::write(dir.join("notes.lproj"), "a file, not a folder").unwrap();
        let found = languages(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(found.unwrap(), ["de", "en"]);
        assert!(languages(&dir).unwrap().is_empty());
    }

    #[test]
    fn no_build_asks_for_the_app_sandbox() {
        for build in BUILDS {
            let plist = entitlements_plist(entitlements(build));
            assert!(plist.starts_with(PLIST_HEAD) && plist.ends_with("</dict>\n</plist>\n"));
            assert!(!plist.contains("app-sandbox"));
        }
    }

    #[test]
    fn an_entitlement_is_switched_on_by_its_key() {
        let plist = entitlements_plist(&["com.apple.security.device.audio-input"]);
        assert!(plist.contains("<dict>\n\t<key>com.apple.security.device.audio-input</key>\n\t<true/>\n</dict>"));
    }

    #[test]
    fn the_certificate_carries_the_identifier_not_the_display_name() {
        assert_eq!(identity_name(), format!("{BUNDLE_ID} Local Signing"));
        assert!(!identity_name().to_lowercase().contains(&NAME.to_lowercase()));
    }

    #[test]
    fn the_signing_certificate_is_found_by_its_exact_name() {
        let listed = "Policy: Code Signing\n  Matching identities\n  \
                      1) 1111111111111111111111111111111111111111 \"Apple Development: Someone (TEAM)\"\n  \
                      2) 2222222222222222222222222222222222222222 \"local.agentide Local Signing Old\" (CSSMERR_TP_NOT_TRUSTED)\n  \
                      3) 3333333333333333333333333333333333333333 \"local.agentide Local Signing\" (CSSMERR_TP_NOT_TRUSTED)\n  \
                      4) 4444444444444444444444444444444444444444 \"local.agentide Local Signing\" (CSSMERR_TP_NOT_TRUSTED)\n     \
                      4 identities found\n\n  Valid identities only\n  \
                      1) 1111111111111111111111111111111111111111 \"Apple Development: Someone (TEAM)\"\n     \
                      1 valid identities found\n";
        assert_eq!(
            identity_hash(listed, "local.agentide Local Signing"),
            Some("3333333333333333333333333333333333333333")
        );
        assert_eq!(
            identity_hash(listed, "Apple Development: Someone (TEAM)"),
            Some("1111111111111111111111111111111111111111")
        );
        assert_eq!(identity_hash(listed, "local.agentide"), None);
        assert_eq!(
            identity_hash("     0 identities found\n", "local.agentide Local Signing"),
            None
        );
    }
}
