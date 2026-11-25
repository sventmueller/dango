//! The split view controller of a window tab, whose sidebar folds and unfolds within 150 ms (SN-W3).

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSSplitViewController, NSSplitViewItemBehavior, NSWorkspace,
};

use crate::util::guarded;

/// How long the sidebar takes to fold or unfold.
const FOLD: f64 = 0.15;

define_class!(
    // SAFETY: NSSplitViewController may be subclassed; `TabSplit` adds no ivars and does not implement Drop.
    #[unsafe(super(NSSplitViewController))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDETabSplit"]
    pub(crate) struct TabSplit;

    impl TabSplit {
        #[unsafe(method(toggleSidebar:))]
        fn toggle_sidebar(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.fold());
        }
    }
);

impl TabSplit {
    pub(crate) fn controller(mtm: MainThreadMarker) -> Retained<NSSplitViewController> {
        // SAFETY: NSSplitViewController's initializer, called once on the newly allocated object.
        let split: Retained<TabSplit> = unsafe { msg_send![TabSplit::alloc(mtm), init] };
        Retained::into_super(split)
    }

    /// Folds the sidebar away or brings it back, animated unless the user asked for reduced motion; a toggle while
    /// one runs turns it around.
    fn fold(&self) {
        let Some(item) = self
            .splitViewItems()
            .iter()
            .find(|item| item.behavior() == NSSplitViewItemBehavior::Sidebar)
        else {
            return;
        };
        let collapsed = !item.isCollapsed();
        if NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion() {
            item.setCollapsed(collapsed);
            return;
        }
        NSAnimationContext::runAnimationGroup(&block2::RcBlock::new(
            move |context: std::ptr::NonNull<NSAnimationContext>| {
                crate::util::guarded((), || {
                    // SAFETY: AppKit hands the block the group's context, alive while the block runs.
                    unsafe { context.as_ref() }.setDuration(FOLD);
                    item.animator().setCollapsed(collapsed);
                })
            },
        ));
    }
}
