//! The self-test's hooks of `intake.rs`: a round of reads now instead of after the interval, and what the self-test
//! reads of a folder's intake.

use super::*;

impl Leitstand {
    /// Reads the issues of every folder whose intake applies, as the next round would.
    pub(crate) fn read_issues_now(&self) {
        self.poll_intake();
    }

    /// Whether intake from issues applies to `root`: its workflow file asks for it and the user confirmed it.
    pub(crate) fn takes_issues(&self, root: &Path) -> bool {
        self.intake_folders().iter().any(|(folder, _, _)| folder == root)
    }

    /// Whether a read of `root`'s issues, or of the issue of one of its orders, is on its way.
    pub(crate) fn reading_issues(&self, root: &Path) -> bool {
        self.intake
            .borrow()
            .folders
            .get(root)
            .is_some_and(|state| state.reading || !state.checking.is_empty())
    }

    /// Whether a rate limit holds the reads of `root` back now.
    pub(crate) fn issues_held(&self, root: &Path) -> bool {
        self.intake
            .borrow()
            .folders
            .get(root)
            .is_some_and(|state| ide_ci::now() < state.not_before)
    }

    /// The pages of the issues `root` took in as orders, as kept on this Mac.
    pub(crate) fn issues_taken(&self, root: &Path) -> BTreeSet<String> {
        self.folders
            .borrow()
            .get(root)
            .map(|folder| load_taken(&folder.project))
            .unwrap_or_default()
    }
}
