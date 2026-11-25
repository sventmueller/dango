//! The defaults domain this app shares with every instance of it: a measured instance saves its window there, under
//! the folder's name and as the last window, which the guard puts back as it found them.

use objc2::AllocAnyThread;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSString, NSUserDefaults};

/// The name a window's values are kept under when no folder has values of its own (`src/window/frame.rs`).
const LAST: &str = "ProjectWindow Last";

/// The keys of the domain that a measured instance may write: those naming the scratch folder, those of the last
/// window and Sparkle's.
fn touched(key: &str, scratch: &str) -> bool {
    key.contains(scratch) || key.ends_with(LAST) || key.starts_with("SU")
}

/// Whether a key the instance wrote belongs to the scratch folder and goes, instead of being put back.
fn scratch_only(key: &str, scratch: &str) -> bool {
    key.contains(scratch)
}

/// The values of the touched keys before the run, put back when dropped.
pub(crate) struct Guard {
    domain: Retained<NSUserDefaults>,
    scratch: String,
    saved: Vec<(String, Option<Retained<AnyObject>>)>,
}

fn keys(domain_name: &str) -> Vec<String> {
    NSUserDefaults::standardUserDefaults()
        .persistentDomainForName(&NSString::from_str(domain_name))
        .map(|dict| dict.allKeys().iter().map(|k| k.to_string()).collect())
        .unwrap_or_default()
}

impl Guard {
    /// Notes the values of the domain `bundle_id` that an instance working in `scratch` may change.
    pub(crate) fn new(bundle_id: &str, scratch: &str) -> Result<Guard, String> {
        let domain = NSUserDefaults::initWithSuiteName(NSUserDefaults::alloc(), Some(&NSString::from_str(bundle_id)))
            .ok_or_else(|| format!("no defaults for {bundle_id}"))?;
        let saved = keys(bundle_id)
            .into_iter()
            .filter(|key| touched(key, scratch))
            .map(|key| {
                let value = domain.objectForKey(&NSString::from_str(&key));
                (key, value)
            })
            .collect();
        Ok(Guard {
            domain,
            scratch: scratch.to_string(),
            saved,
        })
    }

    /// Puts the noted values back and removes what the instance added.
    pub(crate) fn restore(&self, bundle_id: &str) {
        for key in keys(bundle_id).into_iter().filter(|key| touched(key, &self.scratch)) {
            let saved = self.saved.iter().find(|(k, _)| *k == key);
            if scratch_only(&key, &self.scratch) || saved.is_none() {
                self.domain.removeObjectForKey(&NSString::from_str(&key));
            }
        }
        for (key, value) in &self.saved {
            // SAFETY: a property-list value read from this very domain.
            unsafe { self.domain.setObject_forKey(value.as_deref(), &NSString::from_str(key)) };
        }
        self.domain.synchronize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guard_minds_the_scratch_folders_and_the_last_windows_keys_only() {
        let scratch = "/tmp/compare/project";
        assert!(touched("NSWindow Frame ProjectWindow /tmp/compare/project", scratch));
        assert!(touched("Sidebar ProjectWindow Last", scratch));
        assert!(touched("NSWindow Frame ProjectWindow Last", scratch));
        assert!(touched("SULastCheckTime", scratch));
        assert!(!touched("Sidebar ProjectWindow /Users/someone/work", scratch));
        assert!(scratch_only("Panel ProjectWindow /tmp/compare/project", scratch));
        assert!(!scratch_only("Sidebar ProjectWindow Last", scratch));
    }
}
