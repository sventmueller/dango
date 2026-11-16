use super::*;

#[test]
fn a_limit_in_the_file_shows_its_origin_and_resets_to_the_default() {
    let mut file = ProjectFile::new(
        Some("[limits]\nturn_minutes = 30\n".into()),
        false,
        UserDefaults::default(),
    );
    assert_eq!(file.limit(Limit::TurnMinutes), (30, Origin::File));
    file.reset_limit(Limit::TurnMinutes).unwrap();
    assert_eq!(file.limit(Limit::TurnMinutes), (60, Origin::Default));
    file.set_limit(Limit::Attempts, 2).unwrap();
    assert_eq!(file.limit(Limit::Attempts), (2, Origin::File));
}

const FILE: &str = "\
[[roles.reviewer]]
agent = \"claude\"

# Our rules.
[review]
rule = \"preferred\" # the team agreed

[risk]
# Payment code.
high = [\"src/pay/**\"]
";

#[test]
fn every_value_shows_its_origin_and_a_reset_keeps_the_rest_of_the_file() {
    let mut file = ProjectFile::new(Some(FILE.into()), false, UserDefaults::default());
    assert_eq!(file.origin(ProjectValue::Role(Role::Reviewer)), Origin::File);
    assert_eq!(file.origin(ProjectValue::Role(Role::Builder)), Origin::Default);
    assert_eq!(file.reviewer_preferred(), (true, Origin::File));
    assert_eq!(
        file.list(ProjectValue::RiskHigh),
        (vec!["src/pay/**".to_string()], Origin::File)
    );
    assert_eq!(file.list(ProjectValue::Tests), (Vec::new(), Origin::Default));
    file.reset(ProjectValue::Role(Role::Reviewer)).unwrap();
    file.reset(ProjectValue::ReviewerRule).unwrap();
    assert_eq!(file.origin(ProjectValue::Role(Role::Reviewer)), Origin::Default);
    assert_eq!(file.reviewer_preferred(), (false, Origin::Default));
    assert!(file.text().contains("# Payment code.") && file.text().contains("# Our rules."));
    file.set_list(ProjectValue::Tests, "spec/**, e2e/**").unwrap();
    assert_eq!(file.origin(ProjectValue::Tests), Origin::File);
    file.set_list(ProjectValue::RiskHigh, "").unwrap();
    assert_eq!(file.origin(ProjectValue::RiskHigh), Origin::Default);
    assert!(file.errors().is_empty(), "{:?}", file.errors());
}

/// Shows every field of `file`, lets `edit` change what the user types, and takes every field back as Save does.
fn save(file: &mut ProjectFile, edit: impl Fn(ProjectValue, &str) -> String) {
    let shown: Vec<(ProjectValue, String)> = ProjectValue::all()
        .into_iter()
        .map(|v| (v, file.field_text(v)))
        .collect();
    for (value, text) in shown {
        file.take(value, &text, &edit(value, &text)).unwrap();
    }
}

#[test]
fn saving_a_file_with_an_error_keeps_the_values_the_user_did_not_touch() {
    let text = "[limits]\nbogus = 1\nattempts = 2\n\n[risk]\n# Payment code.\nhigh = [\"src/pay/**\"]\n\n[worktree]\nsetup = \"npm ci\"\n";
    let mut file = ProjectFile::new(Some(text.into()), false, UserDefaults::default());
    assert!(!file.errors().is_empty(), "the unknown key is an error of the file");
    assert_eq!(file.field_text(ProjectValue::RiskHigh), "src/pay/**");
    assert_eq!(file.field_text(ProjectValue::Setup), "npm ci");
    assert_eq!(file.limit(Limit::Attempts), (2, Origin::File));
    save(&mut file, |value, text| match value {
        ProjectValue::Limit(Limit::TurnMinutes) => "30".into(),
        _ => text.to_string(),
    });
    assert!(file.text().contains("high = [\"src/pay/**\"]"), "{}", file.text());
    assert!(file.text().contains("# Payment code.") && file.text().contains("setup = \"npm ci\""));
    assert!(file.text().contains("bogus = 1") && file.text().contains("attempts = 2"));
    assert_eq!(file.limit(Limit::TurnMinutes), (30, Origin::File));
}

#[test]
fn an_untouched_budget_writes_nothing_and_an_emptied_one_leaves_the_default() {
    let mut form = DefaultsForm::default();
    form.set_budgets(Some(2.345), None);
    let defaults = form.defaults().clone();
    let mut file = ProjectFile::new(Some("[budget.day]\ndollars = 7.125\n".into()), false, defaults);
    assert_eq!(file.field_text(ProjectValue::OrderBudget), "");
    assert_eq!(file.order_budget(), (Some(2.345), Origin::Default));
    assert!(file.placeholder(ProjectValue::OrderBudget).is_some());
    assert_eq!(file.field_text(ProjectValue::DayBudget), "7.125");
    let before = file.text().to_string();
    save(&mut file, |_, text| text.to_string());
    assert_eq!(file.text(), before, "nothing the user left alone is written");
    save(&mut file, |value, text| match value {
        ProjectValue::DayBudget => String::new(),
        _ => text.to_string(),
    });
    assert_eq!(file.day_budget(), (None, Origin::Default));
    assert!(!file.text().contains("dollars"), "{}", file.text());
}

#[test]
fn a_limit_the_file_does_not_set_shows_empty_with_the_default_and_emptying_one_resets_it() {
    let mut file = ProjectFile::new(Some("[limits]\nattempts = 2\n".into()), false, UserDefaults::default());
    assert_eq!(file.field_text(ProjectValue::Limit(Limit::TurnMinutes)), "");
    assert!(
        file.placeholder(ProjectValue::Limit(Limit::TurnMinutes))
            .is_some_and(|p| p.contains("60"))
    );
    assert_eq!(file.field_text(ProjectValue::Limit(Limit::Attempts)), "2");
    file.take(ProjectValue::Limit(Limit::Attempts), "2", " ").unwrap();
    assert_eq!(file.limit(Limit::Attempts).1, Origin::Default);
}

#[test]
fn a_bad_pattern_shows_as_an_error_of_the_file() {
    let mut file = ProjectFile::new(None, false, UserDefaults::default());
    file.set_list(ProjectValue::Config, "ci/[").unwrap();
    assert_eq!(file.errors().len(), 1);
}

#[test]
fn values_come_back_from_their_tags() {
    for value in ProjectValue::all() {
        assert_eq!(ProjectValue::of_tag(value.tag()), Some(value));
    }
}

#[test]
fn effort_reviewer_rule_and_provider_budgets_change_the_defaults() {
    let mut form = DefaultsForm::default();
    form.set_effort(Role::Reviewer, 0, " high ");
    assert_eq!(form.list(Role::Reviewer)[0].effort.as_deref(), Some("high"));
    form.set_reviewer_preferred(true);
    assert!(form.reviewer_preferred());
    let service = api_service(&Agent::Claude);
    form.set_provider_budget(&service, Some(5.0));
    form.set_budgets(Some(2.0), None);
    assert_eq!(form.provider_budget(&service), Some(5.0));
    form.set_provider_budget(&service, None);
    assert_eq!(form.provider_budget(&service), None);
    assert_eq!(form.budgets(), (Some(2.0), None));
}

#[test]
fn the_ranked_list_moves_and_keeps_one_entry_per_agent() {
    let mut form = DefaultsForm::default();
    let first = form.list(Role::Builder)[0].agent.clone();
    assert_eq!(form.shift(Role::Builder, 0, false), 1);
    assert_eq!(form.list(Role::Builder)[1].agent, first);
    let len = form.list(Role::Builder).len();
    form.add(Role::Builder, first);
    assert_eq!(form.list(Role::Builder).len(), len);
}

#[test]
fn a_ranked_list_edited_in_the_dialog_goes_into_the_file_and_an_empty_one_leaves_the_default() {
    let mut file = ProjectFile::new(Some(FILE.into()), false, UserDefaults::default());
    let (mut list, origin) = file.ranks(Role::Planner);
    assert_eq!(origin, Origin::Default);
    ranks::add(&mut list, Agent::Codex);
    let codex = list.iter().position(|e| e.agent == Agent::Codex).unwrap();
    ranks::set_model(&mut list, codex, "gpt-a");
    ranks::set_effort(&mut list, codex, "high");
    file.set_ranks(Role::Planner, &list).unwrap();
    assert_eq!(file.ranks(Role::Planner), (list, Origin::File));
    assert!(file.text().contains("# Payment code."), "{}", file.text());
    assert!(file.errors().is_empty(), "{:?}", file.errors());
    file.set_ranks(Role::Planner, &[]).unwrap();
    assert_eq!(file.origin(ProjectValue::Role(Role::Planner)), Origin::Default);
}

#[test]
fn dollars_read_with_a_comma_or_a_point() {
    assert_eq!(read_dollars("2,5"), Ok(Some(2.5)));
    assert_eq!(read_dollars(" "), Ok(None));
    assert_eq!(read_dollars("x"), Err(NotAnAmount));
}
