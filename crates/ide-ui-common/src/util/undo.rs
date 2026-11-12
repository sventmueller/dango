use objc2_app_kit::NSTextView;

/// Runs `edit`, whose changes of `view` become one undoable step of their own, also right after typing in the same
/// turn of the run loop; inside a group a caller opened, they join it.
pub fn undo_step(view: &NSTextView, edit: impl FnOnce()) {
    view.breakUndoCoalescing();
    let undo = view.undoManager();
    if let Some(undo) = &undo
        && undo.groupsByEvent()
        && undo.groupingLevel() == 1
    {
        // AppKit closes the turn's group only at its end and until then refuses registrations without an open group,
        // so a fresh one replaces it, which the end of the turn or the next undo closes.
        undo.endUndoGrouping();
        undo.beginUndoGrouping();
    }
    let own = undo
        .as_ref()
        .is_some_and(|u| u.groupsByEvent() && u.groupingLevel() == 0);
    if let Some(undo) = &undo {
        if own {
            undo.setGroupsByEvent(false);
        }
        undo.beginUndoGrouping();
    }
    edit();
    if let Some(undo) = &undo {
        undo.endUndoGrouping();
        if own {
            undo.setGroupsByEvent(true);
        }
    }
    view.breakUndoCoalescing();
}
