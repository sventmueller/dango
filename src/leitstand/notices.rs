//! Notifications of orders (E175, I9): when one waits, failed, is ready for acceptance, spent its budget or was taken
//! over; none during self-tests (R12), and no number on the Dock icon (E72).

use ide_factory::OrderId;
use ide_factory::effect::Notice;
use splat_i18n::t;

/// What the notification about `notice` says.
pub(super) fn body(notice: Notice) -> &'static str {
    match notice {
        Notice::Waiting => t("Der Auftrag wartet auf dich."),
        Notice::Failed => t("Eine Station des Auftrags ist gescheitert."),
        Notice::ReadyForAcceptance => t("Der Nachweis ist bereit zur Abnahme."),
        Notice::BudgetSpent => t("Das Budget ist aufgebraucht; der Auftrag pausiert."),
        Notice::Done => t("Der Auftrag ist übernommen."),
    }
}

/// Posts the notification about `notice` for the order `order` titled `title`.
pub(super) fn post(order: &OrderId, title: &str, notice: Notice) {
    ide_ui_common::notify::post(&format!("order-{order}"), title, body(notice));
}
