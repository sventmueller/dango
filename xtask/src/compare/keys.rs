//! Which key of the current keyboard layout types a character, so ⌘Z is ⌘Z on a German keyboard too.

use std::collections::BTreeMap;
use std::ffi::c_void;

/// A key and the modifiers that make it type a character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stroke {
    pub(crate) code: u16,
    pub(crate) shift: bool,
    pub(crate) option: bool,
}

/// Keys whose code no layout changes.
pub(crate) const RETURN: u16 = 36;
pub(crate) const ESCAPE: u16 = 53;
pub(crate) const DELETE: u16 = 51;
pub(crate) const UP: u16 = 126;
pub(crate) const LEFT: u16 = 123;
pub(crate) const RIGHT: u16 = 124;

/// The characters of the current layout.
#[derive(Debug, Clone, Default)]
pub(crate) struct Layout {
    pub(crate) name: String,
    /// The physical keyboard's type, which a posted key event carries like one from the hardware.
    pub(crate) keyboard: u32,
    strokes: BTreeMap<char, Stroke>,
}

impl Layout {
    /// The stroke that types `c`.
    pub(crate) fn stroke(&self, c: char) -> Result<Stroke, String> {
        self.strokes
            .get(&c)
            .copied()
            .ok_or_else(|| format!("the keyboard layout {} has no key for {c:?}", self.name))
    }
}

/// Builds the table from `translate`, which names the character a key types with shift and option as given; the
/// stroke with the fewest modifiers wins.
fn table(translate: impl Fn(u16, bool, bool) -> Option<char>) -> BTreeMap<char, Stroke> {
    let mut strokes = BTreeMap::new();
    for (shift, option) in [(false, false), (true, false), (false, true), (true, true)] {
        for code in 0..128u16 {
            if let Some(c) = translate(code, shift, option) {
                strokes.entry(c).or_insert(Stroke { code, shift, option });
            }
        }
    }
    strokes
}

#[cfg(target_os = "macos")]
mod carbon {
    use super::*;

    const KEY_ACTION_DOWN: u16 = 0;
    const NO_DEAD_KEYS: u32 = 1;
    const SHIFT: u32 = 0x02;
    const OPTION: u32 = 0x08;

    #[link(name = "Carbon", kind = "framework")]
    // SAFETY: the Text Input Sources and Unicode Utilities functions with their C signatures from HIToolbox.
    unsafe extern "C" {
        static kTISPropertyUnicodeKeyLayoutData: *const c_void;
        static kTISPropertyInputSourceID: *const c_void;
        fn TISCopyCurrentKeyboardLayoutInputSource() -> *const c_void;
        fn TISGetInputSourceProperty(source: *const c_void, key: *const c_void) -> *const c_void;
        fn LMGetKbdType() -> u8;
        fn UCKeyTranslate(
            layout: *const c_void,
            code: u16,
            action: u16,
            modifiers: u32,
            keyboard: u32,
            options: u32,
            dead_keys: *mut u32,
            max: usize,
            length: *mut usize,
            chars: *mut u16,
        ) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    // SAFETY: CoreFoundation's functions with their C signatures.
    unsafe extern "C" {
        fn CFDataGetBytePtr(data: *const c_void) -> *const u8;
        fn CFRelease(object: *const c_void);
        fn CFStringGetCStringPtr(string: *const c_void, encoding: u32) -> *const std::ffi::c_char;
    }

    impl Layout {
        /// The layout the keyboard types with now.
        pub(crate) fn current() -> Result<Layout, String> {
            // SAFETY: the copied source is released below; the property is owned by it and only read while it lives.
            unsafe {
                let source = TISCopyCurrentKeyboardLayoutInputSource();
                if source.is_null() {
                    return Err("macOS names no keyboard layout".into());
                }
                let data = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData);
                let name = name(TISGetInputSourceProperty(source, kTISPropertyInputSourceID));
                let layout = if data.is_null() {
                    std::ptr::null()
                } else {
                    CFDataGetBytePtr(data)
                };
                let result = if layout.is_null() {
                    Err(format!("the keyboard layout {name} has no Unicode table"))
                } else {
                    let keyboard = u32::from(LMGetKbdType());
                    Ok(Layout {
                        name: name.clone(),
                        keyboard,
                        strokes: table(|code, shift, option| translate(layout.cast(), keyboard, code, shift, option)),
                    })
                };
                CFRelease(source);
                result
            }
        }
    }

    /// The identifier of an input source, such as `com.apple.keylayout.German`.
    ///
    /// # Safety
    /// `id` is null or a CFString that lives for the call.
    unsafe fn name(id: *const c_void) -> String {
        const UTF8: u32 = 0x0800_0100;
        if id.is_null() {
            return "?".into();
        }
        // SAFETY: `id` is a live CFString; the pointer it returns, if any, lives as long as it.
        let text = unsafe { CFStringGetCStringPtr(id, UTF8) };
        if text.is_null() {
            return "?".into();
        }
        // SAFETY: CoreFoundation returned a NUL-terminated C string.
        unsafe { std::ffi::CStr::from_ptr(text) }.to_string_lossy().into_owned()
    }

    /// The single character `code` types with the modifiers, if it types one that is printable.
    fn translate(layout: *const c_void, keyboard: u32, code: u16, shift: bool, option: bool) -> Option<char> {
        let modifiers = if shift { SHIFT } else { 0 } | if option { OPTION } else { 0 };
        let mut dead = 0u32;
        let mut length = 0usize;
        let mut chars = [0u16; 4];
        // SAFETY: `layout` points at the layout's `UCKeyboardLayout`, kept alive by its source; the out-parameters
        // are valid and `chars` holds the `max` it is given.
        let status = unsafe {
            UCKeyTranslate(
                layout,
                code,
                KEY_ACTION_DOWN,
                modifiers,
                keyboard,
                NO_DEAD_KEYS,
                &raw mut dead,
                chars.len(),
                &raw mut length,
                chars.as_mut_ptr(),
            )
        };
        if status != 0 || length != 1 {
            return None;
        }
        char::from_u32(u32::from(chars[0])).filter(|c| !c.is_control())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stroke_with_the_fewest_modifiers_types_a_character() {
        let strokes = table(|code, shift, option| match (code, shift, option) {
            (6, false, false) => Some('z'),
            (6, true, false) => Some('Z'),
            (7, true, true) => Some('z'),
            (26, false, true) => Some(']'),
            _ => None,
        });
        let layout = Layout {
            name: "test".into(),
            keyboard: 0,
            strokes,
        };
        assert_eq!(
            layout.stroke('z'),
            Ok(Stroke {
                code: 6,
                shift: false,
                option: false
            })
        );
        assert!(layout.stroke('Z').unwrap().shift);
        assert!(layout.stroke(']').unwrap().option);
        assert!(layout.stroke('q').unwrap_err().contains("test"));
    }
}
