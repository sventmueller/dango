//! What goes wrong out of the user's sight goes to the system log, where Console.app finds it under the app's name. A
//! message names what failed and never holds the text of a file.

use objc2_foundation::NSString;

use crate::util::ns;

unsafe extern "C" {
    fn NSLog(format: *const NSString, ...);
}

pub fn report(what: &str, detail: impl std::fmt::Display) {
    let message = ns(&format!("{}: {what}: {detail}", ide_product::NAME));
    // SAFETY: A valid format with one object placeholder and that one object, so nothing in the message is read as
    // format.
    unsafe { NSLog(&*ns("%@"), &*message as *const NSString) };
}
