//! What the CI panel does: re-running, starting and cancelling runs and jobs, opening logs and pages, handing a failure
//! to an agent (I8), starting merge requests and issues in the browser, and taking or forgetting a token.

use std::cell::Cell;
use std::rc::Rc;

use block2::RcBlock;
use ide_ci::{Client, Job, Run};
use objc2::MainThreadOnly;
use objc2::runtime::AnyObject;
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAlertThirdButtonReturn, NSModalResponse, NSPasteboard, NSPasteboardTypeString,
    NSPopUpButton, NSSecureTextField, NSStackView, NSTextField, NSUserInterfaceLayoutOrientation, NSWorkspace,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSURL};
use splat_i18n::{t, tf};

use super::model::Tab;
use super::panel::{CiPanel, with_panel};
use super::{log_tab, text, work};
use crate::util::{constant, guarded, ns};

/// How many lines of a failed job's log an agent gets.
const EXCERPT_LINES: usize = 120;

impl CiPanel {
    fn client(&self) -> Option<Client> {
        self.read().client.clone()
    }

    /// Starts the failed jobs of `run` again.
    pub fn rerun(&self, run: Option<Run>) {
        let (Some(run), Some(client)) = (run, self.client()) else {
            return;
        };
        self.act(move || client.rerun_failed(&run));
    }

    /// Starts every job of `run` again, on GitHub.
    pub fn rerun_all(&self, run: Option<Run>) {
        let (Some(run), Some(client)) = (run, self.client()) else {
            return;
        };
        self.act(move || client.rerun_all(&run));
    }

    /// Starts the finished `job` again.
    pub fn retry_job(&self, job: Option<Job>) {
        let (Some(job), Some(client)) = (job, self.client()) else {
            return;
        };
        self.act(move || client.retry_job(&job));
    }

    /// Starts `job`, which waits for someone: plays it on GitLab, approves its deployments on GitHub.
    pub fn start_job(&self, job: Option<Job>) {
        let (Some(job), Some(client)) = (job, self.client()) else {
            return;
        };
        self.act(move || client.start_job(&job));
    }

    /// Asks which run to start for the current branch, then starts it: a new pipeline on GitLab, a workflow that may
    /// be started by hand on GitHub.
    pub fn start_run(&self) {
        let (Some(client), Some(window)) = (self.client(), self.controller().and_then(|c| c.window())) else {
            return;
        };
        let Some(branch) = self.read().head.branch.clone() else {
            return;
        };
        let workflows = self.read().workflows.clone();
        let github = self.is_github();
        if github && workflows.is_empty() {
            return;
        }
        let mtm = self.mtm();
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&ns(&tf("Lauf für „{}“ starten", &[&branch])));
        alert.setInformativeText(&ns(&if github {
            tf(
                "GitHub startet den gewählten Workflow mit dem Stand von {} auf {}; was nur lokal liegt, läuft nicht mit.",
                &[&branch, &client.forge().host],
            )
        } else {
            tf(
                "GitLab legt eine neue Pipeline für den Stand von {} auf {} an; was nur lokal liegt, läuft nicht mit.",
                &[&branch, &client.forge().host],
            )
        }));
        alert.addButtonWithTitle(&ns(t("Starten")));
        alert.addButtonWithTitle(&ns(t("Abbrechen")));
        let popup = github.then(|| {
            let popup = NSPopUpButton::initWithFrame_pullsDown(
                NSPopUpButton::alloc(mtm),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(300.0, 26.0)),
                false,
            );
            for workflow in &workflows {
                popup.addItemWithTitle(&ns(&workflow.name));
            }
            alert.setAccessoryView(Some(&popup));
            popup
        });
        let id = self.id();
        sheet(&alert, &window, move |response| {
            if response != NSAlertFirstButtonReturn {
                return;
            }
            let chosen = popup
                .and_then(|p| usize::try_from(p.indexOfSelectedItem()).ok())
                .and_then(|i| workflows.get(i))
                .map(|w| w.file.clone());
            with_panel(id, |panel| {
                panel.act(move || client.start_run(&branch, chosen.as_deref()));
            });
        });
    }

    /// Opens the page for a new merge request of the current branch, or a new issue, by the list's tab.
    pub fn new_item(&self) {
        let Some(client) = self.client() else { return };
        let (tab, branch) = {
            let state = self.read();
            (state.tab, state.head.branch.clone())
        };
        match (tab, branch) {
            (Tab::Changes, Some(branch)) => self.open_url(&client.new_change_page(&branch)),
            (Tab::Issues, _) => self.open_url(&client.new_issue_page()),
            _ => {}
        }
    }

    /// Puts the link of the merge request or issue the action came from on the clipboard.
    pub fn copy_link(&self, sender: Option<&AnyObject>) {
        let Some(item) = self.item_for_action(sender) else {
            return;
        };
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        pasteboard.setString_forType(&ns(&item.url), constant!(NSPasteboardTypeString));
    }

    /// Asks, then cancels `run`.
    pub fn cancel(&self, run: Option<Run>) {
        let (Some(run), Some(client), Some(window)) = (run, self.client(), self.controller().and_then(|c| c.window()))
        else {
            return;
        };
        let id = self.id();
        let message = tf("„{} #{}“ abbrechen?", &[&run.name, &run.number.to_string()]);
        let alert = NSAlert::new(self.mtm());
        alert.setMessageText(&ns(&message));
        alert.setInformativeText(&ns(t(
            "Laufende Jobs werden gestoppt; neu starten lässt sich der Lauf danach.",
        )));
        alert.addButtonWithTitle(&ns(t("Lauf abbrechen")));
        alert.addButtonWithTitle(&ns(t("Weiterlaufen lassen")));
        sheet(&alert, &window, move |response| {
            if response == NSAlertFirstButtonReturn {
                with_panel(id, |panel| panel.act(move || client.cancel(&run)));
            }
        });
    }

    /// Runs `call` against the service, then reads again.
    fn act(&self, call: impl FnOnce() -> ide_ci::Result<()> + Send + 'static) {
        {
            let mut state = self.state();
            if state.busy {
                return;
            }
            state.busy = true;
        }
        self.show();
        let id = self.id();
        let queued = work::run(call, move |result| {
            with_panel(id, |panel| {
                panel.state().busy = false;
                if let Some(Err(error)) = result {
                    panel.set_problem(Some(text::problem(&error)));
                }
                panel.refresh(true);
            });
        });
        if !queued {
            self.state().busy = false;
        }
    }

    /// Opens the log of `job` in a tab.
    pub fn open_log(&self, job: Option<Job>) {
        let (Some(job), Some(client), Some(controller)) = (job, self.client(), self.controller()) else {
            return;
        };
        let run = self
            .find_run(job.run)
            .map(|r| format!("{} #{}", r.name, r.number))
            .unwrap_or_default();
        log_tab::show(&controller, self.root(), client, job, run);
    }

    /// Reads the log of the failed `job` and puts its failing part into a new AI tab's composer (I8).
    pub fn send_to_agent(&self, job: Option<Job>) {
        let (Some(job), Some(client)) = (job, self.client()) else {
            return;
        };
        let run = self
            .find_run(job.run)
            .map(|r| format!("{} #{}", r.name, r.number))
            .unwrap_or_default();
        let id = self.id();
        work::run(
            move || client.log(&job).map(|raw| (job, client.forge().to_string(), raw)),
            move |result| {
                with_panel(id, |panel| match result {
                    Some(Ok((job, place, raw))) => panel.draft_for_agent(&job, &run, &place, &raw),
                    Some(Err(error)) => panel.set_problem(Some(text::problem(&error))),
                    None => {}
                });
            },
        );
    }

    fn draft_for_agent(&self, job: &Job, run: &str, place: &str, raw: &str) {
        let Some(controller) = self.controller() else { return };
        let excerpt = ide_ci::log::excerpt(&ide_ci::log::lines(raw), EXCERPT_LINES);
        if let Some(tab) = open_agent(&controller) {
            tab.set_draft(&draft(&job.name, run, place));
            tab.attach_outside(ide_ui_agent::ci_log(&job.name, run, excerpt));
        }
    }

    /// Opens the page of the row the action came from, else the repository's.
    pub fn open_web(&self, sender: Option<&AnyObject>) {
        if let Some(item) = self.item_for_action(sender) {
            return self.open_url(&item.url);
        }
        let url = match (self.job_for(sender), self.run_for(sender, ide_ci::State::Neutral)) {
            (Some(job), _) if !job.url.is_empty() => job.url,
            (_, Some(run)) if sender.is_some_and(|s| s.downcast_ref::<objc2_app_kit::NSMenuItem>().is_some()) => {
                run.url
            }
            _ => match &self.read().forge {
                Some(forge) => forge.page(),
                None => return,
            },
        };
        self.open_url(&url);
    }

    /// Asks for a token of the host, checks it with the service and keeps it in the keychain.
    pub fn sign_in(&self) {
        let (Some(forge), Some(window)) = (self.read().forge.clone(), self.controller().and_then(|c| c.window()))
        else {
            return;
        };
        let mtm = self.mtm();
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&ns(&tf("Token für {}", &[&forge.host])));
        alert.setInformativeText(&ns(&match forge.kind {
            ide_ci::Kind::GitHub => tf(
                "Ein Personal Access Token mit Lesezugriff auf Actions, zum Neustarten und Abbrechen mit Schreibzugriff. Die App schickt ihn nur an {} und behält ihn im Schlüsselbund.",
                &[&forge.host],
            ),
            ide_ci::Kind::GitLab => tf(
                "Ein Personal Access Token mit dem Bereich read_api, zum Neustarten und Abbrechen api. Die App schickt ihn nur an {} und behält ihn im Schlüsselbund.",
                &[&forge.host],
            ),
        }));
        alert.addButtonWithTitle(&ns(t("Speichern")));
        alert.addButtonWithTitle(&ns(t("Abbrechen")));
        if super::login::availability(&forge).is_ok() {
            alert.addButtonWithTitle(&ns(t("Im Browser anmelden")));
        }
        let field = NSSecureTextField::initWithFrame(
            NSSecureTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(300.0, 24.0)),
        );
        field.setPlaceholderString(Some(&ns(t("Token"))));
        let accessory = NSStackView::new(mtm);
        accessory.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        accessory.addArrangedSubview(&field);
        accessory.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(300.0, 24.0)));
        alert.setAccessoryView(Some(&accessory));
        alert.window().setInitialFirstResponder(Some(&field));
        let id = self.id();
        let typed: objc2::rc::Retained<NSTextField> = field.into_super();
        sheet(&alert, &window, move |response| {
            let token = typed.stringValue().to_string().trim().to_string();
            if response == NSAlertFirstButtonReturn && !token.is_empty() {
                with_panel(id, |panel| panel.check_and_save(forge, token));
            } else if response == NSAlertThirdButtonReturn {
                with_panel(id, CiPanel::browser_login);
            }
        });
    }

    /// Keeps `token` once the service took it.
    fn check_and_save(&self, forge: ide_ci::Forge, token: String) {
        let id = self.id();
        work::run(
            move || {
                let client = Client::new(forge.clone(), Some(token.clone()), work::network());
                let account = client.account()?;
                let auth = work::auth().ok_or(ide_ci::Error::Network("no environment".into()))?;
                auth.save(&forge, &account, &token)
                    .map_err(|e| ide_ci::Error::Network(e.to_string()))
            },
            move |result| {
                with_panel(id, |panel| {
                    match result {
                        Some(Ok(())) => panel.set_problem(None),
                        Some(Err(error)) => panel.set_problem(Some(text::problem(&error))),
                        None => {}
                    }
                    panel.refresh(true);
                });
            },
        );
    }

    pub(super) fn open_url(&self, url: &str) {
        open_url(self.controller().as_deref(), url);
    }

    /// Forgets the app's own token of the host; those of `gh`, `glab` and the environment stay.
    pub fn sign_out(&self) {
        let Some(forge) = self.read().forge.clone() else { return };
        let id = self.id();
        work::run(
            move || work::auth().map(|auth| auth.forget(&forge)),
            move |_| with_panel(id, |panel| panel.refresh(true)),
        );
    }
}

/// Opens a web page in a browser tab of `controller`'s window; other addresses, or no window, go to macOS.
pub fn open_url(controller: Option<&crate::window::WindowController>, url: &str) {
    let lower = url.to_ascii_lowercase();
    if let Some(controller) = controller.filter(|_| lower.starts_with("https://") || lower.starts_with("http://")) {
        return crate::browser::open_tab(controller, Some(url));
    }
    if let Some(url) = NSURL::URLWithString(&ns(url)) {
        NSWorkspace::sharedWorkspace().openURL(&url);
    }
}

/// The composer text for an agent: what failed; the log excerpt goes along as outside text (E22).
pub fn draft(job: &str, run: &str, place: &str) -> String {
    tf(
        "Der CI-Job „{}“ von „{}“ ({}) ist fehlgeschlagen. Finde die Ursache und schlag eine Korrektur vor; der Log-Ausschnitt hängt an.",
        &[job, run, place],
    )
}

/// A new AI tab; the self-test's plays recordings instead of starting `claude`.
fn open_agent(controller: &crate::window::WindowController) -> Option<Rc<ide_ui_agent::AgentTab>> {
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return ide_ui_agent::open_replay(controller);
    }
    ide_ui_agent::open_tab(controller, None)
}

/// Shows `alert` as a sheet on `window`; `then` gets the button that ended it.
pub fn sheet(alert: &NSAlert, window: &objc2_app_kit::NSWindow, then: impl FnOnce(NSModalResponse) + 'static) {
    let then = Cell::new(Some(then));
    let handler = RcBlock::new(move |response: NSModalResponse| {
        guarded((), || {
            if let Some(then) = then.take() {
                then(response);
            }
        });
    });
    alert.beginSheetModalForWindow_completionHandler(window, Some(&handler));
}
