//! The sync of the user's settings between their Macs (E114), without the file system: each Mac writes only a file of
//! its own and reads everybody's. A file holds, per setting, the value or a deletion and a logical clock; the setting
//! everybody ends up with is the one with the greatest clock, the device id breaking ties, so every Mac comes to the
//! same result without asking the user.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// When a value was set: a counter above every one seen before, then the wall clock in milliseconds, so that a clock
/// set back cannot put a change before an older one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Clock {
    /// One more than the greatest counter this Mac had seen.
    pub counter: u64,
    /// Milliseconds since 1970 on the Mac that made the change.
    pub wall: u64,
}

/// One setting in a Mac's file: its value, or `None` for a setting taken back to its default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// The value as the settings file stores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// When it was set.
    pub clock: Clock,
}

/// What one Mac knows of the synced settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceFile {
    /// The settings file's schema version of the app that wrote it.
    pub schema: u64,
    /// The Mac's id, which is also the file's name.
    pub device: String,
    /// The synced settings by name.
    #[serde(default)]
    pub entries: BTreeMap<String, Entry>,
}

impl DeviceFile {
    /// The file of `device` before it took part.
    pub fn empty(device: &str, schema: u64) -> DeviceFile {
        DeviceFile {
            schema,
            device: device.to_string(),
            entries: BTreeMap::new(),
        }
    }
}

/// The settings' field that stays on each Mac although its setting is synced: the microphone of dictation (E103).
const LOCAL_FIELDS: &[(&str, &str)] = &[("dictation", "microphone")];

/// `value` of the setting `name` without its fields that stay on each Mac.
fn shared_part(name: &str, value: &Value) -> Value {
    let mut value = value.clone();
    if let Value::Object(fields) = &mut value {
        for (setting, field) in LOCAL_FIELDS {
            if *setting == name {
                fields.remove(*field);
            }
        }
    }
    value
}

/// `imported` for the setting `name` with the fields that stay on this Mac taken from `local`.
fn with_local_part(name: &str, imported: &Value, local: Option<&Value>) -> Value {
    let mut value = imported.clone();
    if let (Value::Object(fields), Some(Value::Object(kept))) = (&mut value, local) {
        for (setting, field) in LOCAL_FIELDS {
            if *setting == name {
                match kept.get(*field) {
                    Some(mine) => fields.insert((*field).to_string(), mine.clone()),
                    None => fields.remove(*field),
                };
            }
        }
    }
    value
}

/// What a sync round decided.
#[derive(Debug, Clone, PartialEq)]
pub struct Round {
    /// This Mac's file as it is to be written.
    pub own: DeviceFile,
    /// The settings to change on this Mac: a value to set, or `None` to take the setting back to its default.
    pub apply: Vec<(String, Option<Value>)>,
    /// Macs whose file a newer version of the app wrote, which this one leaves out until it is updated.
    pub newer: Vec<String>,
}

/// What a round of the sync on this Mac works with.
pub struct Inputs<'a> {
    /// This Mac's id.
    pub device: &'a str,
    /// The files of the other Macs.
    pub others: &'a [DeviceFile],
    /// The values of this Mac's settings file.
    pub local: &'a Map<String, Value>,
    /// Which settings sync.
    pub shared: &'a dyn Fn(&str) -> bool,
    /// This app's version of the settings file.
    pub schema: u64,
    /// Brings values of an older version of the settings file up to `schema`.
    pub migrate: &'a dyn Fn(&mut Map<String, Value>, u64),
    /// The wall clock in milliseconds.
    pub now: u64,
}

/// One round of the sync on this Mac. `own` is this Mac's file as last written, `None` when the sync was just turned
/// on: the local values then get the clock 0, so that what other Macs have wins.
pub fn round(own: Option<DeviceFile>, inputs: &Inputs<'_>) -> Round {
    let Inputs {
        device,
        others,
        local,
        shared,
        schema,
        migrate,
        now,
    } = *inputs;
    let newer: Vec<String> = others
        .iter()
        .filter(|f| f.schema > schema)
        .map(|f| f.device.clone())
        .collect();
    let others: Vec<DeviceFile> = others
        .iter()
        .filter(|f| f.schema <= schema && f.device != device)
        .map(|f| brought_up(f, schema, migrate))
        .collect();
    let first = own.is_none();
    let mut own = own.unwrap_or_else(|| DeviceFile::empty(device, schema));
    own.schema = schema;
    let seen = others
        .iter()
        .chain(std::iter::once(&own))
        .flat_map(|f| f.entries.values())
        .map(|e| e.clock.counter)
        .max()
        .unwrap_or(0);
    let names: BTreeSet<String> = local
        .keys()
        .chain(own.entries.keys())
        .filter(|name| shared(name))
        .cloned()
        .collect();
    for name in &names {
        let mine = local.get(name).map(|v| shared_part(name, v));
        let recorded = own.entries.get(name).map(|e| e.value.clone());
        if recorded.as_ref() == Some(&mine) {
            continue;
        }
        let clock = if first {
            Clock::default()
        } else {
            Clock {
                counter: seen + 1,
                wall: now,
            }
        };
        own.entries.insert(name.clone(), Entry { value: mine, clock });
    }
    let mut apply = Vec::new();
    for (name, (entry, from)) in winners(&own, &others) {
        if from == device || !shared(&name) {
            continue;
        }
        own.entries.insert(name.clone(), entry.clone());
        let mine = local.get(&name).map(|v| shared_part(&name, v));
        if mine != entry.value {
            let value = entry
                .value
                .as_ref()
                .map(|v| with_local_part(&name, v, local.get(&name)));
            apply.push((name, value));
        }
    }
    Round { own, apply, newer }
}

/// `file` with its values brought to `schema` by the settings' migrations.
fn brought_up(file: &DeviceFile, schema: u64, migrate: &dyn Fn(&mut Map<String, Value>, u64)) -> DeviceFile {
    if file.schema == schema {
        return file.clone();
    }
    let mut values: Map<String, Value> = file
        .entries
        .iter()
        .filter_map(|(name, e)| Some((name.clone(), e.value.clone()?)))
        .collect();
    migrate(&mut values, file.schema);
    let mut entries = BTreeMap::new();
    for (name, entry) in &file.entries {
        let value = entry.value.as_ref().and(values.get(name).cloned());
        entries.insert(
            name.clone(),
            Entry {
                value,
                clock: entry.clock,
            },
        );
    }
    for (name, value) in values {
        if !file.entries.contains_key(&name) {
            let clock = file.entries.values().map(|e| e.clock).max().unwrap_or_default();
            entries.insert(
                name,
                Entry {
                    value: Some(value),
                    clock,
                },
            );
        }
    }
    DeviceFile {
        schema,
        device: file.device.clone(),
        entries,
    }
}

/// For every setting, the entry with the greatest clock among all files and the Mac it came from; on equal clocks the
/// greater device id.
fn winners(own: &DeviceFile, others: &[DeviceFile]) -> BTreeMap<String, (Entry, String)> {
    let mut best: BTreeMap<String, (Entry, String)> = BTreeMap::new();
    for file in std::iter::once(own).chain(others) {
        for (name, entry) in &file.entries {
            let better = best
                .get(name)
                .is_none_or(|(held, from)| (entry.clock, file.device.as_str()) > (held.clock, from.as_str()));
            if better {
                best.insert(name.clone(), (entry.clone(), file.device.clone()));
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SCHEMA: u64 = 1;

    fn shared(name: &str) -> bool {
        name != "agentEndpoints"
    }

    fn no_migration(_: &mut Map<String, Value>, _: u64) {}

    fn values(pairs: &[(&str, Value)]) -> Map<String, Value> {
        pairs.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect()
    }

    /// One round on `device`, with the round before as its own file.
    fn on(device: &str, own: Option<DeviceFile>, others: &[DeviceFile], local: &Map<String, Value>, now: u64) -> Round {
        let inputs = Inputs {
            device,
            others,
            local,
            shared: &shared,
            schema: SCHEMA,
            migrate: &no_migration,
            now,
        };
        round(own, &inputs)
    }

    /// Applies what a round decided to `local`.
    fn applied(mut local: Map<String, Value>, round: &Round) -> Map<String, Value> {
        for (name, value) in &round.apply {
            match value {
                Some(v) => local.insert(name.clone(), v.clone()),
                None => local.remove(name),
            };
        }
        local
    }

    #[test]
    fn turning_it_on_lets_what_other_macs_have_win_and_keeps_per_mac_settings() {
        let a_local = values(&[("fontSize", json!(14)), ("agentEndpoints", json!(["a"]))]);
        let a = on("a", None, &[], &a_local, 1000);
        assert!(a.apply.is_empty());
        let a = on("a", Some(a.own), &[], &values(&[("fontSize", json!(15))]), 2000);
        let b_local = values(&[("fontSize", json!(12)), ("agentEndpoints", json!(["b"]))]);
        let b = on("b", None, std::slice::from_ref(&a.own), &b_local, 3000);
        assert_eq!(b.apply, vec![("fontSize".to_string(), Some(json!(15)))]);
        assert!(!b.own.entries.contains_key("agentEndpoints"));
    }

    #[test]
    fn changes_to_different_settings_on_two_macs_both_arrive() {
        let start = values(&[("fontSize", json!(12)), ("minimap", json!(false))]);
        let a = on("a", None, &[], &start, 0);
        let b = on("b", None, std::slice::from_ref(&a.own), &start, 0);
        let a2 = on(
            "a",
            Some(a.own.clone()),
            std::slice::from_ref(&b.own),
            &values(&[("fontSize", json!(14)), ("minimap", json!(false))]),
            10,
        );
        let b2 = on(
            "b",
            Some(b.own),
            std::slice::from_ref(&a.own),
            &values(&[("fontSize", json!(12)), ("minimap", json!(true))]),
            10,
        );
        let a3 = on(
            "a",
            Some(a2.own.clone()),
            std::slice::from_ref(&b2.own),
            &values(&[("fontSize", json!(14)), ("minimap", json!(false))]),
            20,
        );
        let b3 = on(
            "b",
            Some(b2.own.clone()),
            std::slice::from_ref(&a2.own),
            &values(&[("fontSize", json!(12)), ("minimap", json!(true))]),
            20,
        );
        assert_eq!(a3.apply, vec![("minimap".to_string(), Some(json!(true)))]);
        assert_eq!(b3.apply, vec![("fontSize".to_string(), Some(json!(14)))]);
    }

    #[test]
    fn the_same_setting_changed_on_two_macs_ends_the_same_everywhere() {
        let start = values(&[("fontSize", json!(12))]);
        let a = on("a", None, &[], &start, 0);
        let b = on("b", None, &[], &start, 0);
        let a2 = on("a", Some(a.own), &[], &values(&[("fontSize", json!(14))]), 100);
        let b2 = on("b", Some(b.own), &[], &values(&[("fontSize", json!(16))]), 200);
        let a3 = on(
            "a",
            Some(a2.own.clone()),
            std::slice::from_ref(&b2.own),
            &values(&[("fontSize", json!(14))]),
            300,
        );
        let b3 = on(
            "b",
            Some(b2.own.clone()),
            std::slice::from_ref(&a2.own),
            &values(&[("fontSize", json!(16))]),
            300,
        );
        let a_ends = applied(values(&[("fontSize", json!(14))]), &a3);
        let b_ends = applied(values(&[("fontSize", json!(16))]), &b3);
        assert_eq!(a_ends, b_ends);
        assert_eq!(a_ends.get("fontSize"), Some(&json!(16)));
    }

    #[test]
    fn taking_a_setting_back_to_its_default_syncs_as_a_deletion() {
        let a = on("a", None, &[], &values(&[("minimap", json!(true))]), 0);
        let b = on(
            "b",
            None,
            std::slice::from_ref(&a.own),
            &values(&[("minimap", json!(true))]),
            0,
        );
        let a2 = on("a", Some(a.own), std::slice::from_ref(&b.own), &Map::new(), 50);
        assert_eq!(a2.own.entries.get("minimap").map(|e| e.value.clone()), Some(None));
        let b2 = on(
            "b",
            Some(b.own),
            std::slice::from_ref(&a2.own),
            &values(&[("minimap", json!(true))]),
            60,
        );
        assert_eq!(b2.apply, vec![("minimap".to_string(), None)]);
    }

    #[test]
    fn a_clock_set_back_does_not_put_a_later_change_first() {
        let a = on("a", None, &[], &values(&[("fontSize", json!(12))]), 5000);
        let a2 = on("a", Some(a.own), &[], &values(&[("fontSize", json!(13))]), 5000);
        let b = on(
            "b",
            None,
            std::slice::from_ref(&a2.own),
            &values(&[("fontSize", json!(13))]),
            10,
        );
        let b2 = on(
            "b",
            Some(b.own),
            std::slice::from_ref(&a2.own),
            &values(&[("fontSize", json!(20))]),
            10,
        );
        assert!(b2.own.entries["fontSize"].clock > a2.own.entries["fontSize"].clock);
        let a3 = on(
            "a",
            Some(a2.own.clone()),
            std::slice::from_ref(&b2.own),
            &values(&[("fontSize", json!(13))]),
            6000,
        );
        assert_eq!(a3.apply, vec![("fontSize".to_string(), Some(json!(20)))]);
    }

    #[test]
    fn an_old_file_that_arrives_late_changes_nothing() {
        let a = on("a", None, &[], &values(&[("fontSize", json!(12))]), 0);
        let old_a = on("a", Some(a.own.clone()), &[], &values(&[("fontSize", json!(13))]), 10);
        let new_a = on(
            "a",
            Some(old_a.own.clone()),
            &[],
            &values(&[("fontSize", json!(18))]),
            20,
        );
        let b = on(
            "b",
            None,
            std::slice::from_ref(&new_a.own),
            &values(&[("fontSize", json!(12))]),
            30,
        );
        let local = applied(values(&[("fontSize", json!(12))]), &b);
        let late = on("b", Some(b.own.clone()), std::slice::from_ref(&old_a.own), &local, 40);
        assert!(late.apply.is_empty());
        assert_eq!(local.get("fontSize"), Some(&json!(18)));
    }

    #[test]
    fn a_change_made_by_hand_in_the_settings_file_is_a_change_like_any() {
        let a = on("a", None, &[], &values(&[("showWhitespace", json!(false))]), 0);
        let edited = on("a", Some(a.own), &[], &values(&[("showWhitespace", json!(true))]), 100);
        assert_eq!(edited.own.entries["showWhitespace"].value, Some(json!(true)));
        assert!(edited.own.entries["showWhitespace"].clock.counter > 0);
    }

    #[test]
    fn a_file_of_a_newer_version_pauses_and_is_left_out() {
        let mut future = DeviceFile::empty("c", SCHEMA + 1);
        future.entries.insert(
            "fontSize".into(),
            Entry {
                value: Some(json!(30)),
                clock: Clock { counter: 99, wall: 0 },
            },
        );
        let a = on("a", None, &[future], &values(&[("fontSize", json!(12))]), 0);
        assert_eq!(a.newer, vec!["c".to_string()]);
        assert!(a.apply.is_empty());
    }

    #[test]
    fn the_microphone_of_dictation_stays_on_each_mac() {
        let a_dictation = json!({"mode": "local", "microphone": {"uid": "a-mic"}});
        let a = on("a", None, &[], &values(&[("dictation", a_dictation.clone())]), 0);
        assert_eq!(a.own.entries["dictation"].value, Some(json!({"mode": "local"})));
        let a2 = on(
            "a",
            Some(a.own),
            &[],
            &values(&[("dictation", json!({"mode": "online", "microphone": {"uid": "a-mic"}}))]),
            10,
        );
        let b_dictation = json!({"mode": "local", "microphone": {"uid": "b-mic"}});
        let b = on(
            "b",
            None,
            std::slice::from_ref(&a2.own),
            &values(&[("dictation", b_dictation)]),
            20,
        );
        assert_eq!(
            b.apply,
            vec![(
                "dictation".to_string(),
                Some(json!({"mode": "online", "microphone": {"uid": "b-mic"}}))
            )]
        );
        let mic_only = on(
            "b",
            Some(b.own.clone()),
            std::slice::from_ref(&a2.own),
            &values(&[("dictation", json!({"mode": "online", "microphone": {"uid": "other"}}))]),
            30,
        );
        assert_eq!(mic_only.own.entries["dictation"], b.own.entries["dictation"]);
    }

    #[test]
    fn an_older_file_is_brought_up_by_the_migrations() {
        let mut old = DeviceFile::empty("b", 1);
        old.entries.insert(
            "size".into(),
            Entry {
                value: Some(json!(14)),
                clock: Clock { counter: 5, wall: 0 },
            },
        );
        let rename = |values: &mut Map<String, Value>, _: u64| {
            if let Some(size) = values.remove("size") {
                values.insert("fontSize".into(), size);
            }
        };
        let local = values(&[("fontSize", json!(12))]);
        let others = [old];
        let inputs = Inputs {
            device: "a",
            others: &others,
            local: &local,
            shared: &shared,
            schema: 2,
            migrate: &rename,
            now: 0,
        };
        let a = round(None, &inputs);
        assert_eq!(a.apply, vec![("fontSize".to_string(), Some(json!(14)))]);
    }
}
