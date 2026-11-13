//! Signing in through the browser from the CI panel (E12): GitHub's device flow with its code in a sheet, GitLab's
//! authorization code with PKCE that comes back to a listener on the loopback address. The token goes into the
//! keychain for its host and account.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ide_ci::oauth::{self, LoginError};
use ide_ci::{Client, Forge, Kind};
use ide_core::settings::{Key, Settings};
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSFont, NSPasteboard, NSPasteboardTypeString, NSTextAlignment, NSTextField,
    NSWorkspace,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSURL};
use splat_i18n::{t, tf};

use super::ops::sheet;
use super::panel::{CiPanel, with_panel};
use super::{text, work};
use crate::util::{constant, ns};

/// OAuth apps the user registered on hosts other than github.com and gitlab.com, as `host=id`. Only the user sets
/// them, like the host rules.
pub const CI_OAUTH_CLIENTS: Key<Vec<String>> = Key::user("ciOAuthClients", Vec::new);

/// A login that waits for the browser.
pub struct Login {
    pub ticket: Ticket,
    /// The code the user types on GitHub; `None` for GitLab.
    pub code: Option<String>,
}

/// Names one login, so what an earlier, stopped one finds later changes nothing, and stops it.
#[derive(Clone)]
pub struct Ticket {
    serial: u64,
    cancel: Arc<AtomicBool>,
}

impl Ticket {
    fn new() -> Ticket {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Ticket {
            serial: NEXT.fetch_add(1, Ordering::Relaxed),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    fn stopped(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// Held while a login listens on the callback port: a login that took over from a stopped one waits until that one
/// has let the port go, instead of finding it taken.
static CALLBACK_PORT: Mutex<()> = Mutex::new(());

/// Whether the browser login can start for `forge`; else the note that says why not.
pub fn availability(forge: &Forge) -> Result<(), &'static str> {
    if ide_ci::is_public_host(forge) && ide_ci::product_client(forge).is_none() {
        return Err(t("Browser-Anmeldung noch nicht eingerichtet"));
    }
    Ok(())
}

/// The OAuth app for `forge`: the product's, or the one the user named for its host.
fn client_for(forge: &Forge) -> Option<String> {
    if let Some(id) = ide_ci::product_client(forge) {
        return Some(id.to_string());
    }
    if ide_ci::is_public_host(forge) {
        return None;
    }
    Settings::load_user()
        .get(&CI_OAUTH_CLIENTS)
        .iter()
        .find_map(|entry| entry.split_once('=').filter(|(host, _)| *host == forge.host))
        .map(|(_, id)| id.trim().to_string())
        .filter(|id| !id.is_empty())
}

fn remember_client(host: &str, id: &str) {
    let mut settings = Settings::load_user();
    let mut entries: Vec<String> = settings
        .get(&CI_OAUTH_CLIENTS)
        .into_iter()
        .filter(|entry| entry.split_once('=').is_none_or(|(h, _)| h != host))
        .collect();
    entries.push(format!("{host}={id}"));
    if let Err(error) = settings.set(&CI_OAUTH_CLIENTS, &entries) {
        crate::diag::report("saving the OAuth app of a host", error);
    }
    crate::settings::ci_changed();
}

/// What a failed login says; nothing for one the user stopped.
fn message(error: &LoginError) -> Option<String> {
    Some(match error {
        LoginError::Cancelled => return None,
        LoginError::Denied => t("Die Anmeldung wurde im Browser abgelehnt.").into(),
        LoginError::Expired => t("Die Anmeldung ist abgelaufen; starte sie neu.").into(),
        LoginError::StateMismatch => {
            t("Die Antwort des Browsers gehörte zu einer anderen Anmeldung und wurde nicht angenommen.").into()
        }
        LoginError::PortInUse(port) => tf(
            "Port {} ist von einem anderen Programm belegt; über ihn kehrt der Browser zur App zurück.",
            &[&port.to_string()],
        ),
        LoginError::Refused(reason) => tf("Der Dienst hat die Anmeldung abgelehnt: {}", &[reason]),
        LoginError::Api(error) => text::problem(error),
    })
}

/// What the self-test would have opened in the browser or put on the pasteboard, which belong to the user.
static NOTED: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn under_selftest() -> bool {
    cfg!(feature = "selftest") && std::env::var_os("IDE_SELFTEST").is_some()
}

fn note(text: &str) {
    NOTED.lock().unwrap_or_else(|e| e.into_inner()).push(text.to_string());
}

/// Opens `url` in the browser.
fn open_url(url: &str) {
    if under_selftest() {
        return note(url);
    }
    if let Some(url) = NSURL::URLWithString(&ns(url)) {
        NSWorkspace::sharedWorkspace().openURL(&url);
    }
}

/// Puts `text` on the pasteboard.
fn copy(text: &str) {
    if under_selftest() {
        return note(&format!("pasteboard:{text}"));
    }
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    pasteboard.setString_forType(&ns(text), constant!(NSPasteboardTypeString));
}

/// What the self-test would have opened or copied, in order.
#[cfg(feature = "selftest")]
pub fn opened() -> Vec<String> {
    NOTED.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Waits `duration` in small steps; false once `cancel` is set.
fn wait(duration: Duration, cancel: &AtomicBool) -> bool {
    let mut left = duration;
    while !left.is_zero() {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let step = left.min(Duration::from_millis(100));
        std::thread::sleep(step);
        left -= step;
    }
    !cancel.load(Ordering::Relaxed)
}

/// Keeps `grant` for the account it belongs to.
fn keep(forge: &Forge, client: &str, grant: &oauth::Grant) -> Result<(), LoginError> {
    let account = Client::new(forge.clone(), Some(grant.token.clone()), work::network()).account()?;
    let auth = work::auth().ok_or(LoginError::Api(ide_ci::Error::Network("no environment".into())))?;
    auth.save_grant(forge, &account, grant, client)
        .map_err(|e| LoginError::Api(ide_ci::Error::Network(e)))
}

impl CiPanel {
    /// Starts signing in through the browser, asking first for the OAuth app of a host other than the public ones.
    pub fn browser_login(&self) {
        let Some(forge) = self.read().forge.clone() else { return };
        if self.read().login.is_some() {
            return;
        }
        if let Err(note) = availability(&forge) {
            self.set_problem(Some(note.to_string()));
            self.show();
            return;
        }
        match client_for(&forge) {
            Some(client) => self.start_login(forge, client),
            None => self.ask_client(forge),
        }
    }

    /// Stops a login that waits for the browser.
    pub fn cancel_login(&self) {
        if let Some(login) = self.state().login.take() {
            login.ticket.cancel.store(true, Ordering::Relaxed);
        }
        self.show();
    }

    /// Whether `ticket` names the login that waits now.
    fn current(&self, ticket: &Ticket) -> bool {
        self.read()
            .login
            .as_ref()
            .is_some_and(|login| login.ticket.serial == ticket.serial)
    }

    /// Stops the logins that wait in other windows: only one can own the callback port and the browser's attention.
    fn stop_others(&self) {
        for other in super::panel::all() {
            if other.id() != self.id() && other.read().login.is_some() {
                other.cancel_login();
                other.set_problem(Some(
                    t("Die Anmeldung wurde beendet, weil in einem anderen Fenster eine begonnen hat.").into(),
                ));
                other.show();
            }
        }
    }

    fn ask_client(&self, forge: Forge) {
        let Some(window) = self.controller().and_then(|c| c.window()) else {
            return;
        };
        let mtm = self.mtm();
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&ns(&tf("OAuth-App auf {}", &[&forge.host])));
        alert.setInformativeText(&ns(&match forge.kind {
            Kind::GitHub => t(
                "Trag die Client-ID einer OAuth-App ein, die auf diesem Host registriert ist und den Device Flow erlaubt.",
            )
            .to_string(),
            Kind::GitLab => tf(
                "Trag die Application-ID einer Anwendung ein, die auf diesem Host registriert ist: nicht vertraulich, Bereich api, Redirect-URI {}.",
                &[&oauth::redirect_uri(oauth::CALLBACK_PORT)],
            ),
        }));
        alert.addButtonWithTitle(&ns(t("Weiter")));
        alert.addButtonWithTitle(&ns(t("Abbrechen")));
        let field = NSTextField::textFieldWithString(&ns(""), mtm);
        field.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(300.0, 24.0)));
        alert.setAccessoryView(Some(&field));
        alert.window().setInitialFirstResponder(Some(&field));
        let id = self.id();
        sheet(&alert, &window, move |response| {
            let client = field.stringValue().to_string().trim().to_string();
            if response == NSAlertFirstButtonReturn && !client.is_empty() {
                remember_client(&forge.host, &client);
                with_panel(id, |panel| panel.start_login(forge, client));
            }
        });
    }

    fn start_login(&self, forge: Forge, client: String) {
        self.stop_others();
        let ticket = Ticket::new();
        self.state().login = Some(Login {
            ticket: ticket.clone(),
            code: None,
        });
        self.set_problem(None);
        self.show();
        match forge.kind {
            Kind::GitHub => self.device_flow(forge, client, ticket),
            Kind::GitLab => self.pkce_flow(forge, client, ticket),
        }
    }

    fn device_flow(&self, forge: Forge, client: String, ticket: Ticket) {
        let id = self.id();
        let (web, asked_client) = (forge.web.clone(), client.clone());
        work::run(
            move || oauth::device_code(&*work::network(), &web, &asked_client, oauth::GITHUB_SCOPES),
            move |code| {
                with_panel(id, |panel| match code {
                    Some(Ok(code)) if panel.current(&ticket) => panel.show_code(forge, client, code, ticket),
                    Some(Ok(_)) => {}
                    Some(Err(error)) => panel.login_done(&ticket, Err(error)),
                    None => panel.login_done(&ticket, Err(LoginError::Cancelled)),
                });
            },
        );
    }

    /// The sheet with the code to type on GitHub; its button copies the code, opens the page and starts waiting.
    fn show_code(&self, forge: Forge, client: String, code: oauth::DeviceCode, ticket: Ticket) {
        let Some(window) = self.controller().and_then(|c| c.window()) else {
            return;
        };
        if let Some(login) = self.state().login.as_mut() {
            login.code = Some(code.user_code.clone());
        }
        self.show();
        let mtm = self.mtm();
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&ns(&tf("Bei {} anmelden", &[&forge.host])));
        alert.setInformativeText(&ns(&tf(
            "Gib auf {} diesen Code ein. Danach holt die App den Token von selbst.",
            &[&code.verification_uri],
        )));
        let shown = NSTextField::labelWithString(&ns(&code.user_code), mtm);
        shown.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(
            24.0,
            // SAFETY: AppKit sets its font weight constants when it loads.
            unsafe { objc2_app_kit::NSFontWeightSemibold },
        )));
        shown.setSelectable(true);
        shown.setAlignment(NSTextAlignment::Center);
        shown.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(300.0, 34.0)));
        alert.setAccessoryView(Some(&shown));
        alert.addButtonWithTitle(&ns(&tf("Code kopieren und {} öffnen", &[&forge.host])));
        alert.addButtonWithTitle(&ns(t("Abbrechen")));
        let id = self.id();
        sheet(&alert, &window, move |response| {
            with_panel(id, |panel| {
                if !panel.current(&ticket) {
                    return;
                }
                if response != NSAlertFirstButtonReturn {
                    panel.cancel_login();
                    return;
                }
                copy(&code.user_code);
                open_url(&code.verification_uri);
                panel.poll_device(forge, client, code, ticket);
            });
        });
    }

    fn poll_device(&self, forge: Forge, client: String, code: oauth::DeviceCode, ticket: Ticket) {
        let id = self.id();
        let waiting = ticket.clone();
        work::long(
            move || {
                let grant = oauth::device_token(&*work::network(), &forge.web, &client, &code, &mut |d| {
                    wait(d, &waiting.cancel)
                })?;
                keep(&forge, &client, &grant)
            },
            move |result| {
                with_panel(id, |panel| {
                    panel.login_done(&ticket, result.unwrap_or(Err(LoginError::Cancelled)))
                })
            },
        );
    }

    fn pkce_flow(&self, forge: Forge, client: String, ticket: Ticket) {
        let id = self.id();
        let page =
            t("Anmeldung abgeschlossen. Du kannst dieses Fenster schließen und zur App zurückkehren.").to_string();
        let waiting = ticket.clone();
        work::long(
            move || {
                let pkce = oauth::Pkce::new()?;
                let redirect = oauth::redirect_uri(oauth::CALLBACK_PORT);
                let code = {
                    let _port = CALLBACK_PORT.lock().unwrap_or_else(|e| e.into_inner());
                    if waiting.stopped() {
                        return Err(LoginError::Cancelled);
                    }
                    let listener = oauth::listen(oauth::CALLBACK_PORT)?;
                    let url = pkce.authorize_url(&forge.web, &client, &redirect, oauth::GITLAB_SCOPES);
                    crate::util::main_soon(move || open_url(&url));
                    oauth::callback_code(
                        &listener,
                        &pkce.state,
                        oauth::CALLBACK_TIMEOUT,
                        &|| waiting.stopped(),
                        &page,
                    )?
                };
                let grant = oauth::exchange_code(&*work::network(), &forge.web, &client, &code, &pkce, &redirect)?;
                keep(&forge, &client, &grant)
            },
            move |result| {
                with_panel(id, |panel| {
                    panel.login_done(&ticket, result.unwrap_or(Err(LoginError::Cancelled)))
                })
            },
        );
    }

    /// Ends the login `ticket` names; one that was stopped meanwhile only has the panel read a token it still kept.
    fn login_done(&self, ticket: &Ticket, result: Result<(), LoginError>) {
        if !self.current(ticket) {
            if result.is_ok() {
                self.refresh(true);
            }
            return;
        }
        self.state().login = None;
        match &result {
            Ok(()) => self.set_problem(None),
            Err(error) => {
                if let Some(message) = message(error) {
                    self.set_problem(Some(message));
                }
            }
        }
        self.show();
        self.refresh(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_browser_login_waits_for_the_products_apps_on_the_public_hosts_only() {
        let forge = |url: &str, rules: &[ide_ci::HostRule]| ide_ci::candidates(url, rules).remove(0);
        for url in ["git@github.com:a/b.git", "https://gitlab.com/a/b.git"] {
            assert_eq!(
                availability(&forge(url, &[])),
                Err(t("Browser-Anmeldung noch nicht eingerichtet")),
                "{url}"
            );
        }
        let rules = [ide_ci::HostRule::parse("git.example.com=gitlab").unwrap()];
        assert_eq!(availability(&forge("https://git.example.com/a/b.git", &rules)), Ok(()));
    }

    #[test]
    fn a_failed_login_says_why_and_one_the_user_stopped_says_nothing() {
        assert_eq!(message(&LoginError::Cancelled), None);
        let said = |error: LoginError| message(&error).unwrap_or_default();
        assert_eq!(said(LoginError::Denied), t("Die Anmeldung wurde im Browser abgelehnt."));
        assert_eq!(
            said(LoginError::Expired),
            t("Die Anmeldung ist abgelaufen; starte sie neu.")
        );
        assert_eq!(
            said(LoginError::StateMismatch),
            t("Die Antwort des Browsers gehörte zu einer anderen Anmeldung und wurde nicht angenommen.")
        );
        assert!(said(LoginError::PortInUse(47819)).contains("47819"));
        assert!(said(LoginError::Refused("invalid_client".into())).ends_with("invalid_client"));
        let api = ide_ci::Error::Network("refused".into());
        assert_eq!(said(LoginError::Api(api.clone())), text::problem(&api));
    }

    #[test]
    fn a_wait_ends_early_once_its_login_is_stopped() {
        let cancel = AtomicBool::new(false);
        assert!(wait(Duration::from_millis(20), &cancel));
        cancel.store(true, Ordering::Relaxed);
        let started = std::time::Instant::now();
        assert!(!wait(Duration::from_secs(30), &cancel));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
