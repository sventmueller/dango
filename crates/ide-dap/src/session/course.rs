//! How far a session has got in starting and in ending, apart from what its program does; both only move forward.

use crate::types::EndReason;

/// The session's way from its start to its end.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Course {
    start: Startup,
    end: Ending,
}

/// The two answers the program waits for before it runs, which come in either order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Startup {
    #[default]
    Waiting,
    /// `launch` or `attach` was answered.
    Launched,
    /// The breakpoints are set and `configurationDone` was answered.
    Configured,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum Ending {
    /// Nobody asked the session to end, and the adapter has not said it is over.
    #[default]
    Open,
    /// The session ends as `reason` once the connection or the adapter goes.
    Asked {
        reason: EndReason,
        goodbye: Goodbye,
        adapter: AdapterSaid,
    },
}

/// Whether `terminate` and `disconnect` went out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Goodbye {
    Due,
    Said,
}

/// Whether the adapter sent `terminated`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdapterSaid {
    Nothing,
    Terminated,
}

impl Course {
    /// Notes that `launch` or `attach` was answered.
    pub(crate) fn launched(&mut self) {
        self.start = match self.start {
            Startup::Waiting => Startup::Launched,
            Startup::Configured | Startup::Ready => Startup::Ready,
            Startup::Launched => Startup::Launched,
        };
    }

    /// Notes that the configuration is done.
    pub(crate) fn configured(&mut self) {
        self.start = match self.start {
            Startup::Waiting => Startup::Configured,
            Startup::Launched | Startup::Ready => Startup::Ready,
            Startup::Configured => Startup::Configured,
        };
    }

    /// Whether the breakpoints went to the adapter, so later changes go there too.
    pub(crate) fn is_configured(&self) -> bool {
        matches!(self.start, Startup::Configured | Startup::Ready)
    }

    /// Whether the program may run: launched or attached, and configured.
    pub(crate) fn is_ready(&self) -> bool {
        self.start == Startup::Ready
    }

    /// Asks the session to end as `reason`; false when ending was asked before, whose reason stays.
    pub(crate) fn ask_end(&mut self, reason: EndReason) -> bool {
        if self.end != Ending::Open {
            return false;
        }
        self.end = Ending::Asked {
            reason,
            goodbye: Goodbye::Due,
            adapter: AdapterSaid::Nothing,
        };
        true
    }

    /// Notes the adapter's `terminated`; true when that is what starts the end, as a finished session.
    pub(crate) fn adapter_terminated(&mut self) -> bool {
        match &mut self.end {
            Ending::Open => {
                self.end = Ending::Asked {
                    reason: EndReason::Finished,
                    goodbye: Goodbye::Due,
                    adapter: AdapterSaid::Terminated,
                };
                true
            }
            Ending::Asked { adapter, .. } => {
                *adapter = AdapterSaid::Terminated;
                false
            }
        }
    }

    /// Whether the adapter sent `terminated`.
    pub(crate) fn is_terminated(&self) -> bool {
        matches!(
            self.end,
            Ending::Asked {
                adapter: AdapterSaid::Terminated,
                ..
            }
        )
    }

    /// Takes the one goodbye of a session asked to end; `Some` with whether the adapter already sent `terminated`.
    pub(crate) fn take_goodbye(&mut self) -> Option<bool> {
        match &mut self.end {
            Ending::Asked {
                goodbye: goodbye @ Goodbye::Due,
                adapter,
                ..
            } => {
                *goodbye = Goodbye::Said;
                Some(*adapter == AdapterSaid::Terminated)
            }
            Ending::Open | Ending::Asked { .. } => None,
        }
    }

    /// How the session ends, once that was decided.
    pub(crate) fn intended(&self) -> Option<&EndReason> {
        match &self.end {
            Ending::Open => None,
            Ending::Asked { reason, .. } => Some(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_program_runs_after_both_answers_in_either_order() {
        let mut course = Course::default();
        assert!(!course.is_ready() && !course.is_configured());
        course.launched();
        assert!(!course.is_ready() && !course.is_configured());
        course.configured();
        assert!(course.is_ready() && course.is_configured());

        let mut course = Course::default();
        course.configured();
        assert!(course.is_configured() && !course.is_ready());
        course.configured();
        assert!(!course.is_ready());
        course.launched();
        assert!(course.is_ready());
        course.launched();
        assert!(course.is_ready(), "a second answer takes nothing back");
    }

    #[test]
    fn the_first_reason_to_end_stays() {
        let mut course = Course::default();
        assert_eq!(course.intended(), None);
        assert!(course.ask_end(EndReason::Stopped));
        assert!(!course.ask_end(EndReason::Finished));
        assert!(!course.adapter_terminated(), "ending was under way already");
        assert_eq!(course.intended(), Some(&EndReason::Stopped));
        assert!(course.is_terminated());
    }

    #[test]
    fn the_adapters_terminated_ends_an_open_session_as_finished() {
        let mut course = Course::default();
        assert!(!course.is_terminated());
        assert!(course.adapter_terminated());
        assert_eq!(course.intended(), Some(&EndReason::Finished));
        assert!(!course.ask_end(EndReason::Stopped));
        assert_eq!(course.take_goodbye(), Some(true));
    }

    #[test]
    fn goodbye_is_said_once_and_only_when_ending() {
        let mut course = Course::default();
        assert_eq!(course.take_goodbye(), None, "nobody asked the session to end");
        course.ask_end(EndReason::Stopped);
        assert_eq!(course.take_goodbye(), Some(false));
        assert_eq!(course.take_goodbye(), None);
        course.adapter_terminated();
        assert_eq!(course.take_goodbye(), None);
        assert!(course.is_terminated());
    }
}
