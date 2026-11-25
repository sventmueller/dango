//! The evidence package (E163): bound by its fingerprint to the tree of a commit, the rule set, the base commit and a
//! manifest of the other inputs (C69), with the deviations first (R70). Any change to one of these makes it invalid,
//! and an order is taken over only with valid evidence.

use serde::{Deserialize, Serialize};

use crate::budget::{Metering, Spend};
use crate::gates::{CommandOutcome, GateFailure, GateStep, SecretScan};
use crate::hash::hash_json;
use crate::routing::{Agent, RankEntry, Vendor};
use crate::station::{Role, Station};
use crate::verdict::Round;

/// An input of the gates outside the repository's tree (C69).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "input")]
pub enum ManifestEntry {
    /// A file copied into the worktree (E188), with the hash of its content.
    Copied {
        /// The path relative to the repository.
        path: String,
        /// The hash of its content.
        hash: String,
    },
    /// A path dependency outside the repository, such as `../splat`, with its state: commit and a hash of
    /// uncommitted changes.
    Dependency {
        /// The path as the build names it.
        path: String,
        /// Its state.
        state: String,
    },
}

/// The manifest of the inputs outside the tree, in a fixed order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    entries: Vec<ManifestEntry>,
}

impl Manifest {
    /// A manifest of `entries`, sorted so the order the app found them in does not matter.
    pub fn new(mut entries: Vec<ManifestEntry>) -> Manifest {
        entries.sort();
        entries.dedup();
        Manifest { entries }
    }

    /// The entries.
    pub fn entries(&self) -> &[ManifestEntry] {
        &self.entries
    }
}

/// What the fingerprint binds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bound {
    /// The tree of the evidence's commit.
    pub tree: String,
    /// The hash of the rule set (E190).
    pub rules: String,
    /// The base commit.
    pub base: String,
    /// The other inputs.
    pub manifest: Manifest,
}

impl Bound {
    /// The fingerprint: SHA-256 over all four.
    pub fn fingerprint(&self) -> String {
        hash_json(self)
    }

    /// What differs from `now`, in the order tree, rules, base, manifest.
    pub fn changed(&self, now: &Bound) -> Option<Invalidation> {
        if self.tree != now.tree {
            Some(Invalidation::Tree)
        } else if self.rules != now.rules {
            Some(Invalidation::Rules)
        } else if self.base != now.base {
            Some(Invalidation::Base)
        } else if self.manifest != now.manifest {
            Some(Invalidation::Manifest)
        } else {
            None
        }
    }
}

/// Why evidence is no longer valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Invalidation {
    /// The tree changed.
    Tree,
    /// The rule set changed.
    Rules,
    /// The base changed (C56).
    Base,
    /// An input of the manifest changed.
    Manifest,
    /// The criteria changed (E184).
    Criteria,
    /// The user worked by hand (E184).
    ByHand,
    /// The user rejected hunks at G2 (C55).
    PartialAcceptance,
}

/// A deviation, shown before everything else in the evidence; [`Deviation::rank`] is the order E163 lists them in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "deviation")]
pub enum Deviation {
    /// gitleaks is missing, so secrets were not checked (F75).
    SecretsNotChecked,
    /// The diff changes agent or scanner configuration (E172).
    AgentConfigChanged {
        /// The paths.
        paths: Vec<String>,
    },
    /// The diff changes existing tests or lint, CI or build configuration (E172).
    ChecksChanged {
        /// The paths.
        paths: Vec<String>,
    },
    /// The reviewer is of a builder's vendor (E162).
    SameVendorReview,
    /// Another entry of the role's list took over the station (E170).
    AgentSwitched {
        /// The role.
        role: Role,
        /// The agent before.
        from: Agent,
        /// The agent after.
        to: Agent,
    },
    /// The user switched the model in the session (E184).
    ModelSwitched {
        /// The role.
        role: Role,
        /// The new model.
        model: String,
    },
    /// The user wrote into an order's session (E184).
    Interventions {
        /// How often.
        count: u32,
    },
    /// The user worked on a station by hand (E184).
    ByHand,
    /// A path without a reliable figure, limited only by runtime and turns (E168).
    Unmetered,
    /// A late usage report pushed a budget past its ceiling by one turn (C64).
    BudgetOvershoot,
    /// Routing chose an entry whose quota was unknown (E170).
    QuotaUnknown,
    /// Files with credentials were copied into the worktree (C50).
    CredentialsCopied,
    /// The base moved and the diff against it stayed byte for byte the same, so G2 stands (C56).
    BaseUpdated,
    /// Routing chose a later entry of the role's list over an equally ready one by its first-pass rate (E170, E180).
    TieBreak {
        /// The role.
        role: Role,
        /// The entry chosen.
        chosen: RankEntry,
        /// The entry passed over.
        over: RankEntry,
        /// The chosen entry's first-pass rate in percent.
        percent: u32,
        /// The first-pass rate of the entry passed over, in percent.
        over_percent: u32,
    },
}

impl Deviation {
    /// Its place in the list.
    pub fn rank(&self) -> u8 {
        match self {
            Deviation::SecretsNotChecked => 0,
            Deviation::AgentConfigChanged { .. } => 1,
            Deviation::ChecksChanged { .. } => 2,
            Deviation::SameVendorReview => 3,
            Deviation::AgentSwitched { .. } => 4,
            Deviation::ModelSwitched { .. } => 5,
            Deviation::Interventions { .. } => 6,
            Deviation::ByHand => 7,
            Deviation::Unmetered => 8,
            Deviation::BudgetOvershoot => 9,
            Deviation::QuotaUnknown => 10,
            Deviation::CredentialsCopied => 11,
            Deviation::BaseUpdated => 12,
            Deviation::TieBreak { .. } => 13,
        }
    }
}

/// Sorts `deviations` into E163's order and drops repeats.
pub fn order_deviations(deviations: &mut Vec<Deviation>) {
    deviations.sort_by_key(Deviation::rank);
    deviations.dedup();
}

/// One gate's entry in the evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateRecord {
    /// Which step.
    pub step: GateStep,
    /// Its name, such as the gate command's.
    pub name: String,
    /// The command that ran, for gate commands; with the name it tells whether the record still belongs to the gate
    /// set after the rules changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// The failure, if it failed.
    pub failure: Option<GateFailure>,
    /// What the command did, for gate commands.
    pub command: Option<CommandOutcome>,
    /// What the scan found, for the secrets scan.
    pub scan: Option<SecretScan>,
    /// Set when the step did not run, such as the path scope without a scope (F54).
    pub skipped: bool,
}

/// Where one turn's work came from (E163).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// The role.
    pub role: Role,
    /// The agent.
    pub agent: Agent,
    /// The model, when known.
    pub model: Option<String>,
    /// The vendor.
    pub vendor: Vendor,
    /// The commit the factory made after the turn, when it changed anything.
    pub commit: Option<String>,
}

/// Cost and runtime of a station (E163).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StationCost {
    /// The station.
    pub station: Option<Station>,
    /// What it spent.
    pub spend: Spend,
    /// How long it ran, in milliseconds.
    pub runtime_ms: u64,
}

/// The evidence package of one state of an order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    bound: Bound,
    fingerprint: String,
    commit: String,
    deviations: Vec<Deviation>,
    gates: Vec<GateRecord>,
    rounds: Vec<Round>,
    costs: Vec<StationCost>,
    provenance: Vec<Provenance>,
    images: Vec<String>,
    metering: Metering,
    invalid: Option<Invalidation>,
}

impl Evidence {
    /// The evidence for `commit`, bound to `bound`.
    pub fn new(bound: Bound, commit: &str, mut deviations: Vec<Deviation>) -> Evidence {
        order_deviations(&mut deviations);
        Evidence {
            fingerprint: bound.fingerprint(),
            bound,
            commit: commit.to_string(),
            deviations,
            gates: Vec::new(),
            rounds: Vec::new(),
            costs: Vec::new(),
            provenance: Vec::new(),
            images: Vec::new(),
            metering: Metering::default(),
            invalid: None,
        }
    }

    /// Adds the gates, review rounds, costs, provenance, pictures and metering.
    pub fn with_details(
        mut self,
        gates: Vec<GateRecord>,
        rounds: Vec<Round>,
        costs: Vec<StationCost>,
        provenance: Vec<Provenance>,
        metering: Metering,
    ) -> Evidence {
        self.images = gates
            .iter()
            .filter_map(|g| g.command.as_ref())
            .flat_map(|c| c.images.iter().cloned())
            .collect();
        self.gates = gates;
        self.rounds = rounds;
        self.costs = costs;
        self.provenance = provenance;
        self.metering = metering;
        self
    }

    /// The fingerprint.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// What it is bound to.
    pub fn bound(&self) -> &Bound {
        &self.bound
    }

    /// The commit it is evidence for.
    pub fn commit(&self) -> &str {
        &self.commit
    }

    /// The deviations, in E163's order.
    pub fn deviations(&self) -> &[Deviation] {
        &self.deviations
    }

    /// Adds a deviation in its place.
    pub fn deviate(&mut self, deviation: Deviation) {
        self.deviations.push(deviation);
        order_deviations(&mut self.deviations);
    }

    /// Every gate with its result.
    pub fn gates(&self) -> &[GateRecord] {
        &self.gates
    }

    /// The review rounds.
    pub fn rounds(&self) -> &[Round] {
        &self.rounds
    }

    /// Cost and runtime per station.
    pub fn costs(&self) -> &[StationCost] {
        &self.costs
    }

    /// Where each turn's work came from.
    pub fn provenance(&self) -> &[Provenance] {
        &self.provenance
    }

    /// Pictures the gates wrote (C69).
    pub fn images(&self) -> &[String] {
        &self.images
    }

    /// Which units could be counted.
    pub fn metering(&self) -> Metering {
        self.metering
    }

    /// Why it is invalid, if it is.
    pub fn invalid(&self) -> Option<Invalidation> {
        self.invalid
    }

    /// Whether it is valid for `now`.
    pub fn valid_for(&self, now: &Bound) -> bool {
        self.invalid.is_none() && self.bound.changed(now).is_none()
    }

    /// Marks it invalid; the first reason stays.
    pub fn invalidate(&mut self, why: Invalidation) {
        self.invalid.get_or_insert(why);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bound() -> Bound {
        Bound {
            tree: "t1".into(),
            rules: "r1".into(),
            base: "b1".into(),
            manifest: Manifest::new(vec![
                ManifestEntry::Dependency {
                    path: "../splat".into(),
                    state: "abc".into(),
                },
                ManifestEntry::Copied {
                    path: "CLAUDE.md".into(),
                    hash: "h".into(),
                },
            ]),
        }
    }

    #[test]
    fn every_bound_input_changes_the_fingerprint_and_invalidates() {
        let evidence = Evidence::new(bound(), "c1", Vec::new());
        assert!(evidence.valid_for(&bound()));
        type Change = (fn(&mut Bound), Invalidation);
        let changes: [Change; 4] = [
            (|b| b.tree = "t2".into(), Invalidation::Tree),
            (|b| b.rules = "r2".into(), Invalidation::Rules),
            (|b| b.base = "b2".into(), Invalidation::Base),
            (
                |b| {
                    b.manifest = Manifest::new(vec![ManifestEntry::Dependency {
                        path: "../splat".into(),
                        state: "def".into(),
                    }])
                },
                Invalidation::Manifest,
            ),
        ];
        for (change, why) in changes {
            let mut now = bound();
            change(&mut now);
            assert_ne!(now.fingerprint(), evidence.fingerprint());
            assert!(!evidence.valid_for(&now));
            assert_eq!(evidence.bound().changed(&now), Some(why));
        }
    }

    #[test]
    fn the_manifest_order_does_not_matter() {
        let mut reversed = bound();
        let mut entries = reversed.manifest.entries().to_vec();
        entries.reverse();
        reversed.manifest = Manifest::new(entries);
        assert_eq!(reversed.fingerprint(), bound().fingerprint());
    }

    #[test]
    fn deviations_come_in_the_listed_order() {
        let mut evidence = Evidence::new(
            bound(),
            "c1",
            vec![Deviation::Unmetered, Deviation::SameVendorReview, Deviation::Unmetered],
        );
        evidence.deviate(Deviation::SecretsNotChecked);
        assert_eq!(
            evidence.deviations(),
            [
                Deviation::SecretsNotChecked,
                Deviation::SameVendorReview,
                Deviation::Unmetered
            ]
        );
        evidence.invalidate(Invalidation::Criteria);
        evidence.invalidate(Invalidation::Tree);
        assert_eq!(evidence.invalid(), Some(Invalidation::Criteria));
        assert!(!evidence.valid_for(&bound()));
    }
}
