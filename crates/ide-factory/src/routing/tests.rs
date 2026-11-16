use std::collections::BTreeMap;

use super::*;
use crate::metrics::{MIN_ORDERS, Rate};
use crate::quota::{QuotaKey, QuotaObservation, QuotaStatus};

#[test]
fn vendors_come_from_the_program_or_the_model_prefix() {
    let cases = [
        (Agent::Claude, None, Vendor::Anthropic),
        (Agent::Codex, Some("gpt-5-codex"), Vendor::OpenAi),
        (Agent::Antigravity, None, Vendor::Google),
        (Agent::OpenCode, Some("anthropic/claude-sonnet-4-5"), Vendor::Anthropic),
        (Agent::OpenCode, Some("openrouter/qwen/qwen3-coder"), Vendor::Alibaba),
        (Agent::Own, Some("gemini-2.5-pro"), Vendor::Google),
        (Agent::Own, Some("llama3.3:70b"), Vendor::Meta),
        (Agent::Own, Some("metallic-7b"), Vendor::Unknown),
        (Agent::Own, Some("my-finetune"), Vendor::Unknown),
        (Agent::Own, None, Vendor::Unknown),
        (Agent::Acp("kimi-cli".into()), Some("kimi-k2"), Vendor::Moonshot),
    ];
    for (agent, model, vendor) in cases {
        assert_eq!(vendor_of(&agent, model), vendor, "{agent} {model:?}");
    }
}

#[test]
fn an_unknown_vendor_is_never_a_change_of_vendor() {
    assert!(vendors_differ(Vendor::Anthropic, Vendor::OpenAi));
    assert!(!vendors_differ(Vendor::Anthropic, Vendor::Anthropic));
    assert!(!vendors_differ(Vendor::Unknown, Vendor::OpenAi));
    assert!(!vendors_differ(Vendor::Google, Vendor::Unknown));
}

#[test]
fn opencode_antigravity_and_unknown_acp_agents_take_no_role() {
    for agent in [Agent::OpenCode, Agent::Antigravity, Agent::Acp("x".into())] {
        for role in Role::ALL {
            assert!(!Capabilities::of(&agent).takes(role), "{agent} {role:?}");
        }
    }
    assert!(Capabilities::of(&Agent::Claude).takes(Role::Builder));
    assert!(Capabilities::of(&Agent::Codex).takes(Role::Reviewer));
    assert!(!Capabilities::of(&Agent::Codex).reports_subagents);
}

struct World {
    agents: BTreeMap<Agent, AgentStatus>,
    quotas: QuotaBook,
    reached: Vec<&'static str>,
    excluded: Vec<usize>,
    builders: Vec<Vendor>,
    rule: ReviewerRule,
    /// First-pass rate and orders per agent.
    rates: Vec<(Agent, (f64, u32))>,
}

impl World {
    fn new() -> World {
        let mut agents = BTreeMap::new();
        for (agent, service) in [
            (Agent::Claude, "claude-subscription"),
            (Agent::Codex, "chatgpt-subscription"),
            (Agent::Own, "google-api"),
        ] {
            let mut status = AgentStatus::ready(&agent);
            status.account = Some("me".into());
            status.service = Some(service.into());
            agents.insert(agent, status);
        }
        World {
            agents,
            quotas: QuotaBook::default(),
            reached: Vec::new(),
            excluded: Vec::new(),
            builders: Vec::new(),
            rule: ReviewerRule::Required,
            rates: Vec::new(),
        }
    }

    fn quota(&mut self, service: &str, utilization: f64) {
        self.quotas.observe(QuotaObservation {
            key: QuotaKey {
                account: "me".into(),
                service: service.into(),
                window: "five_hour".into(),
            },
            utilization: Some(utilization),
            status: QuotaStatus::Allowed,
            window_seconds: 18_000,
            resets_at: None,
            observed_at: 0,
        });
    }

    fn route(&self, role: Role, list: &[RankEntry]) -> Route {
        let reached = |service: Option<&str>| service.is_some_and(|s| self.reached.contains(&s));
        let first_pass = |entry: &RankEntry| {
            self.rates
                .iter()
                .find(|(agent, _)| *agent == entry.agent)
                .map(|&(_, (rate, orders))| Rate { rate, orders })
        };
        let cx = RouteContext {
            agents: &self.agents,
            quotas: &self.quotas,
            now_wall: 1000,
            start_limit: 0.7,
            budget_reached: &reached,
            excluded: &self.excluded,
            builders: &self.builders,
            rule: self.rule,
            first_pass: &first_pass,
        };
        route(role, list, &cx)
    }
}

fn list() -> Vec<RankEntry> {
    vec![
        RankEntry::new(Agent::Claude).model("claude-opus-4-5"),
        RankEntry::new(Agent::Codex).model("gpt-5-codex"),
        RankEntry::new(Agent::Own).model("gemini-2.5-pro"),
    ]
}

fn chosen(route: &Route) -> Option<usize> {
    route.choice.as_ref().map(|c| c.index)
}

#[test]
fn routing_table() {
    type Setup = fn(&mut World);
    type Case = (
        &'static str,
        Role,
        Setup,
        Option<usize>,
        &'static [(usize, &'static str)],
    );
    let cases: [Case; 9] = [
        ("first ready entry", Role::Builder, |_| {}, Some(0), &[]),
        (
            "a sign-in failure passes to the next entry with a note",
            Role::Builder,
            |w| {
                w.agents.get_mut(&Agent::Claude).unwrap().readiness = Readiness::SignInFailed("expired".into());
            },
            Some(1),
            &[(0, "not_ready")],
        ),
        (
            "a path the app does not list is not installed",
            Role::Builder,
            |w| {
                w.agents.remove(&Agent::Claude);
            },
            Some(1),
            &[(0, "not_ready")],
        ),
        (
            "quota at seventy percent passes",
            Role::Builder,
            |w| w.quota("claude-subscription", 0.7),
            Some(1),
            &[(0, "quota")],
        ),
        (
            "quota below seventy percent starts",
            Role::Builder,
            |w| w.quota("claude-subscription", 0.69),
            Some(0),
            &[],
        ),
        (
            "a reached budget of the service passes",
            Role::Builder,
            |w| w.reached.push("claude-subscription"),
            Some(1),
            &[(0, "budget")],
        ),
        (
            "an entry that failed in the station is excluded",
            Role::Builder,
            |w| w.excluded.push(0),
            Some(1),
            &[(0, "excluded")],
        ),
        (
            "the reviewer differs from every builder",
            Role::Reviewer,
            |w| w.builders = vec![Vendor::Anthropic, Vendor::OpenAi],
            Some(2),
            &[(0, "same_vendor"), (1, "same_vendor")],
        ),
        (
            "every path down: nothing is chosen",
            Role::Builder,
            |w| {
                for status in w.agents.values_mut() {
                    status.readiness = Readiness::StartFailed("crash".into());
                }
            },
            None,
            &[(0, "not_ready"), (1, "not_ready"), (2, "not_ready")],
        ),
    ];
    for (name, role, setup, expected, skips) in cases {
        let mut world = World::new();
        setup(&mut world);
        let route = world.route(role, &list());
        assert_eq!(chosen(&route), expected, "{name}");
        let got: Vec<(usize, String)> = route
            .skipped
            .iter()
            .map(|(i, skip)| {
                (
                    *i,
                    serde_json::to_value(skip).unwrap()["why"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        let want: Vec<(usize, String)> = skips.iter().map(|(i, s)| (*i, s.to_string())).collect();
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn without_another_vendor_the_rule_decides() {
    let mut world = World::new();
    world.builders = vec![Vendor::Anthropic];
    let only_claude = [RankEntry::new(Agent::Claude)];
    let required = world.route(Role::Reviewer, &only_claude);
    assert_eq!((chosen(&required), required.same_vendor), (None, false));
    world.rule = ReviewerRule::Preferred;
    let preferred = world.route(Role::Reviewer, &only_claude);
    assert_eq!((chosen(&preferred), preferred.same_vendor), (Some(0), true));
}

#[test]
fn an_unknown_model_does_not_satisfy_the_reviewer_rule() {
    let mut world = World::new();
    world.builders = vec![Vendor::Anthropic];
    let unknown = [RankEntry::new(Agent::Own).model("local-thing")];
    assert_eq!(chosen(&world.route(Role::Reviewer, &unknown)), None);
}

#[test]
fn a_stale_quota_is_unknown_and_lets_the_station_start() {
    let mut world = World::new();
    world.quota("claude-subscription", 0.95);
    let route = {
        let reached = |_: Option<&str>| false;
        let cx = RouteContext {
            agents: &world.agents,
            quotas: &world.quotas,
            now_wall: 18_000_001,
            start_limit: 0.7,
            budget_reached: &reached,
            excluded: &[],
            builders: &[],
            rule: ReviewerRule::Required,
            first_pass: &|_| None,
        };
        route(Role::Builder, &list(), &cx)
    };
    let choice = route.choice.unwrap();
    assert_eq!((choice.index, choice.quota), (0, QuotaState::Unknown));
    world.excluded.clear();
}

#[test]
fn the_own_agent_without_a_model_counts_as_its_endpoint_vendor() {
    let mut world = World::new();
    world.builders = vec![Vendor::Anthropic];
    let own = RankEntry::new(Agent::Own);
    let unknown = world.route(Role::Reviewer, std::slice::from_ref(&own));
    assert_eq!(chosen(&unknown), None, "an unknown vendor is no change of vendor");
    world.agents.get_mut(&Agent::Own).unwrap().vendor = Some(Vendor::Google);
    let routed = world.route(Role::Reviewer, std::slice::from_ref(&own));
    assert_eq!(routed.choice.map(|c| c.vendor), Some(Vendor::Google));
    let named = RankEntry::new(Agent::Own).model("gpt-5");
    let routed = world.route(Role::Reviewer, std::slice::from_ref(&named));
    assert_eq!(
        routed.choice.map(|c| c.vendor),
        Some(Vendor::OpenAi),
        "a model the entry names decides"
    );
}

#[test]
fn stage_two_needs_a_ready_builder_and_a_ready_reviewer_of_another_vendor() {
    let mut world = World::new();
    let builders = [RankEntry::new(Agent::Claude), RankEntry::new(Agent::Codex)];
    let reviewers = [RankEntry::new(Agent::Codex), RankEntry::new(Agent::Claude)];
    let stage = |world: &World, reviewers: &[RankEntry]| stage_for(&world.agents, &builders, reviewers, Stage::Two);
    assert_eq!(stage(&world, &reviewers), Stage::Two);
    assert_eq!(stage_for(&world.agents, &builders, &reviewers, Stage::One), Stage::One);
    world.agents.get_mut(&Agent::Codex).unwrap().readiness = Readiness::NotInstalled;
    assert_eq!(stage(&world, &reviewers), Stage::One, "Claude alone");
    let with_own = [RankEntry::new(Agent::Codex), RankEntry::new(Agent::Own)];
    assert_eq!(stage(&world, &with_own), Stage::One, "an own agent of unknown vendor");
    world.agents.get_mut(&Agent::Own).unwrap().vendor = Some(Vendor::Google);
    assert_eq!(
        stage(&world, &with_own),
        Stage::Two,
        "Claude builds, the own agent on Gemini reviews"
    );
    assert_eq!(
        stage(&world, &reviewers),
        Stage::One,
        "a ready own agent the reviewer's list does not name"
    );
}

#[test]
fn the_better_first_pass_rate_breaks_a_tie_only_on_enough_orders() {
    let enough = MIN_ORDERS;
    type Case = (&'static str, [(f64, u32); 3], Option<usize>, usize);
    let cases: [Case; 6] = [
        ("no rates: the first entry", [(0.0, 0); 3], None, 0),
        (
            "the second is better on enough orders",
            [(0.4, enough), (0.9, enough), (0.0, 0)],
            Some(0),
            1,
        ),
        (
            "the second rests on too few",
            [(0.4, enough), (0.9, enough - 1), (0.0, 0)],
            None,
            0,
        ),
        (
            "the first rests on too few",
            [(0.4, enough - 1), (0.9, enough), (0.0, 0)],
            None,
            0,
        ),
        (
            "an equal rate keeps the rank",
            [(0.8, enough), (0.8, enough), (0.0, 0)],
            None,
            0,
        ),
        (
            "the best of three wins",
            [(0.4, enough), (0.6, enough), (0.9, enough)],
            Some(0),
            2,
        ),
    ];
    for (name, rates, over, index) in cases {
        let mut world = World::new();
        world.rates = [Agent::Claude, Agent::Codex, Agent::Own]
            .into_iter()
            .zip(rates)
            .filter(|(_, (_, orders))| *orders > 0)
            .collect();
        let choice = world.route(Role::Builder, &list()).choice.expect("an entry");
        assert_eq!(choice.index, index, "{name}");
        assert_eq!(choice.tie_break.as_ref().map(|t| t.over_index), over, "{name}");
    }
}

#[test]
fn the_tie_break_note_names_both_rates_and_the_entry_passed_over() {
    let mut world = World::new();
    world.rates = vec![
        (Agent::Claude, (0.5, MIN_ORDERS)),
        (Agent::Codex, (0.75, MIN_ORDERS + 3)),
    ];
    let choice = world.route(Role::Builder, &list()).choice.expect("an entry");
    let note = choice.tie_break.expect("the tie-break decided");
    assert_eq!(note.over, list()[0]);
    assert_eq!((note.rate, note.over_rate), (0.75, 0.5));
}

#[test]
fn an_entry_that_is_not_as_ready_does_not_win_the_tie_break() {
    let mut world = World::new();
    world.rates = vec![(Agent::Claude, (0.2, MIN_ORDERS)), (Agent::Codex, (1.0, MIN_ORDERS))];
    world.quota("chatgpt-subscription", 0.2);
    let route = world.route(Role::Builder, &list());
    assert_eq!(
        chosen(&route),
        Some(0),
        "a known quota against an unknown one is no tie"
    );
    world.quota("claude-subscription", 0.5);
    let route = world.route(Role::Builder, &list());
    assert_eq!(chosen(&route), Some(1), "both known: the rate decides");
    world.quota("chatgpt-subscription", 0.9);
    let route = world.route(Role::Builder, &list());
    assert_eq!(chosen(&route), Some(0), "an entry over the start limit takes no part");
    assert!(route.choice.is_some_and(|c| c.tie_break.is_none()));
}
