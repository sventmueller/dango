//! The Leitstand tab's table: a table view where Return opens the selected order as a double click does, so the
//! table works from the keyboard alone (E101), and the cells of its columns.

use ide_factory::panel::Column;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSEvent, NSLineBreakMode, NSTableView, NSView};
use objc2_foundation::NSObjectProtocol;

use crate::model::board::TableRow;
use crate::ui;
use ide_ui_common::style::space;
use ide_ui_common::util::{guarded, label};

/// The key codes of Return and of Enter on the number pad.
const RETURN_KEYS: [u16; 2] = [36, 76];

define_class!(
    // SAFETY: NSTableView has no subclassing requirements and `OrderTable` does not implement Drop.
    /// The table of orders, which opens the selected one on Return.
    #[unsafe(super(NSTableView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFactoryOrderTable"]
    pub(crate) struct OrderTable;

    unsafe impl NSObjectProtocol for OrderTable {}

    impl OrderTable {
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let opened = guarded(false, || {
                if !RETURN_KEYS.contains(&event.keyCode()) || self.selectedRow() < 0 {
                    return false;
                }
                let (Some(target), Some(action)) = (self.target(), self.doubleAction()) else {
                    return false;
                };
                let app = objc2_app_kit::NSApplication::sharedApplication(self.mtm());
                // SAFETY: The double action takes the sender, as every action does, and its target answers it.
                unsafe { app.sendAction_to_from(action, Some(&target), Some(self)) }
            });
            if !opened {
                // SAFETY: The superclass's implementation, with this method's signature.
                let _: () = unsafe { msg_send![super(self), keyDown: event] };
            }
        }
    }
);

impl OrderTable {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<OrderTable> {
        // SAFETY: NSTableView's initializer, called once on the newly allocated object.
        unsafe { msg_send![OrderTable::alloc(mtm), init] }
    }
}

/// The cell of `row` in `column`: the station with the shape of the condition, the others as text.
pub(crate) fn cell(mtm: MainThreadMarker, row: &TableRow, column: Column) -> Retained<NSView> {
    let text = label(mtm, row.cell(column));
    text.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    if column != Column::Station {
        return text.into_super().into_super();
    }
    let line = ui::row(mtm, space::XS);
    line.addArrangedSubview(&ui::tone_image(mtm, row.tone()));
    line.addArrangedSubview(&text);
    line.into_super()
}
