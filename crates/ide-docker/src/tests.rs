//! The readers against Docker's output (A10). The `*-24.0.6` files are recordings of the oldest supported Docker
//! (Compose 2.23.0), the `*-29.9.3` ones of the newest at the time (Compose 5.7.0). The `*-format` files hold cases
//! the recordings lack, a dangling image and a label value with commas, in the documented form; `events-health-29.9.3`
//! keeps the shape of a recorded health check with neutral names and values.

use std::path::Path;

use super::board::{Board, Look, Node, Section};
use super::*;

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn containers_carry_their_compose_project_service_and_ports() {
    let list = containers(&fixture("ps-format.jsonl"));
    assert_eq!(list.len(), 3);
    let db = &list[0];
    assert_eq!(
        (db.project.as_deref(), db.service.as_deref()),
        (Some("shop"), Some("db"))
    );
    assert_eq!(db.ports.len(), 1, "the IPv6 twin is left out");
    assert_eq!(db.url().as_deref(), Some("http://localhost:5432"));
    assert!(db.running() && !list[1].running());
    assert_eq!(
        list[2].project, None,
        "a label with commas in its value does not make a project"
    );
    assert_eq!(list[2].ports[0].container_port, 80);
}

#[test]
fn images_volumes_contexts_and_events_read_as_docker_writes_them() {
    let images = images(&fixture("images-format.jsonl"));
    assert_eq!(images[0].reference(), "postgres:16");
    assert_eq!(images[1].reference(), "sha256:22bb", "a dangling image goes by its id");
    let volumes = volumes(&fixture("volumes-format.jsonl"));
    assert_eq!(volumes[0].project.as_deref(), Some("shop"));
    let contexts = contexts(&fixture("contexts-29.9.3.jsonl"));
    assert_eq!(
        contexts.iter().find(|c| c.current).map(|c| c.name.as_str()),
        Some("desktop-linux")
    );
    let events: Vec<Event> = fixture("events-format.jsonl").lines().filter_map(event).collect();
    assert_eq!(
        events[0],
        Event {
            kind: ObjectKind::Container,
            action: EventAction::Listed("start".into()),
            id: "3f2a1c".into()
        }
    );
    assert_eq!(events[1].kind, ObjectKind::Volume);
}

/// The same Compose project, a container alone, a stop and start, recorded by the oldest and the newest Docker.
fn reads_recording(version: &str) {
    let recorded = |what: &str| fixture(&format!("{what}-{version}.jsonl"));
    let list = containers(&recorded("ps"));
    let named = |name: &str| {
        list.iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("{name} listed"))
    };
    let db = named("dangorec-db-1");
    assert_eq!(
        (db.project.as_deref(), db.service.as_deref(), db.running()),
        (Some("dangorec"), Some("db"), true)
    );
    assert_eq!(db.url().as_deref(), Some("http://localhost:18080"));
    let web = named("dangorec-web-1");
    assert!(!web.running() && web.status.starts_with("Exited (1)"));
    let proxy = named("dangorec-proxy");
    assert_eq!((proxy.project.as_deref(), proxy.ports.len()), (None, 1));
    assert_eq!(images(&recorded("images"))[0].reference(), "alpine:3.20");
    let volumes = volumes(&recorded("volumes"));
    assert_eq!(
        (volumes[0].name.as_str(), volumes[0].project.as_deref()),
        ("dangorec_data", Some("dangorec"))
    );
    let raw = recorded("events");
    let events: Vec<Event> = raw.lines().filter_map(event).collect();
    assert_eq!(events.len(), raw.lines().count(), "every line is a change");
    assert!(
        events
            .iter()
            .any(|e| e.kind == ObjectKind::Container && e.action.word() == "start" && e.id == proxy.id)
    );
    assert!(events.iter().any(|e| e.action.word() == "die" && e.id == web.id));
    assert!(events.iter().all(Event::changes_listing));
    assert!(contexts(&recorded("contexts")).iter().any(|c| c.current));
}

#[test]
fn docker_24_0_6_output_reads_as_recorded() {
    reads_recording("24.0.6");
}

#[test]
fn docker_29_output_reads_as_recorded() {
    reads_recording("29.9.3");
}

#[test]
fn a_health_check_changes_nothing_listed() {
    let checks: Vec<Event> = fixture("events-health-29.9.3.jsonl")
        .lines()
        .filter_map(event)
        .collect();
    assert_eq!(checks.len(), 4);
    assert!(checks.iter().all(|e| !e.changes_listing()), "{checks:?}");
}

#[test]
fn states_protocols_kinds_and_actions_read_from_dockers_words() {
    let states = [
        ("created", ContainerState::Created),
        ("running", ContainerState::Running),
        ("paused", ContainerState::Paused),
        ("restarting", ContainerState::Restarting),
        ("removing", ContainerState::Removing),
        ("exited", ContainerState::Exited),
        ("dead", ContainerState::Dead),
        ("frozen", ContainerState::Other("frozen".into())),
    ];
    for (word, state) in states {
        assert_eq!(ContainerState::read(word), state);
    }
    assert_eq!(ports("0.0.0.0:53->53/udp")[0].protocol, Protocol::Udp);
    assert_eq!(ports("0.0.0.0:9->9/sctp")[0].protocol, Protocol::Sctp);
    assert_eq!(ports("0.0.0.0:9->9/quic")[0].protocol, Protocol::Other("quic".into()));
    assert_eq!(ObjectKind::read("network"), ObjectKind::Network);
    assert_eq!(ObjectKind::read("daemon"), ObjectKind::Other("daemon".into()));
    assert_eq!(
        EventAction::read("health_status: healthy"),
        EventAction::Quiet("health_status: healthy".into())
    );
    assert_eq!(EventAction::read("destroy").word(), "destroy");
    assert!(matches!(EventAction::read("exec_start: sh"), EventAction::Quiet(_)));
}

#[test]
fn every_command_names_its_context_and_never_switches_the_users_one() {
    let all = [
        list_containers("ctx"),
        list_images("ctx"),
        list_volumes("ctx"),
        watch_events("ctx"),
        container("ctx", Action::Remove, "id"),
        compose("ctx", Compose::Up, Path::new("/p/shop/compose.yaml")),
    ];
    for command in &all {
        assert_eq!(command.args[..2], ["--context", "ctx"], "{command:?}");
        assert!(!command.args.windows(2).any(|w| w[0] == "context" && w[1] == "use"));
    }
    assert_eq!(all[4].args[2..], ["rm", "--force", "id"]);
    let up = &all[5];
    assert_eq!(up.cwd.as_deref(), Some(Path::new("/p/shop")));
    assert!(up.args.ends_with(&["up".to_string(), "--detach".to_string()]));
    assert_eq!(
        logs_line("desktop-linux", "3f2a1c"),
        "docker --context desktop-linux logs --follow 3f2a1c"
    );
}

#[test]
fn the_tree_groups_compose_projects_and_shows_their_files_without_containers() {
    let root = ide_core::testing::scratch("docker");
    std::fs::create_dir_all(root.join("shop")).unwrap();
    std::fs::create_dir_all(root.join("tools")).unwrap();
    std::fs::write(root.join("shop/compose.yaml"), "services: {}\n").unwrap();
    std::fs::write(root.join("tools/docker-compose.yml"), "services: {}\n").unwrap();
    let board = Board {
        containers: containers(&fixture("ps-format.jsonl")),
        images: images(&fixture("images-format.jsonl")),
        volumes: volumes(&fixture("volumes-format.jsonl")),
        files: compose_files(&root),
    };
    let rows = board.rows();
    let shop = rows.iter().find(|r| r.label == "shop").unwrap();
    assert_eq!((shop.look, shop.detail.as_str()), (Look::Partly, "1/2"));
    assert!(matches!(&shop.node, Node::Project(_, Some(file)) if file.ends_with("shop/compose.yaml")));
    let tools = rows.iter().find(|r| r.label == "tools").unwrap();
    assert_eq!((tools.look, tools.detail.as_str()), (Look::Stopped, "0/0"));
    assert!(rows.iter().any(|r| r.node == Node::Section(Section::Containers)));
    assert!(rows.iter().any(|r| r.label == "web-proxy" && r.level == 1));
    assert!(rows.iter().any(|r| r.label == "db" && r.level == 2));
}

#[test]
fn the_version_floor_and_the_backoff() {
    assert!(supported("24.0.6") && supported("29.9.3") && !supported("24.0.5") && !supported("20.10.24"));
    assert_eq!(backoff(0).as_secs(), 1);
    assert_eq!(backoff(3).as_secs(), 8);
    assert_eq!(backoff(20).as_secs(), 60);
}

#[test]
fn every_image_row_has_its_own_node_even_when_docker_lists_an_untagged_image_twice() {
    let image = |id: &str, repository: &str, tag: &str| Image {
        id: id.into(),
        repository: repository.into(),
        tag: tag.into(),
        size: "1MB".into(),
    };
    let board = Board {
        containers: Vec::new(),
        images: vec![
            image("e29b", "postgres", "<none>"),
            image("e29b", "postgres", "<none>"),
            image("e29b", "postgres", "17.11-alpine"),
            image("952a", "postgres", "<none>"),
            image("22bb", "<none>", "<none>"),
        ],
        volumes: Vec::new(),
        files: Vec::new(),
    };
    let images: Vec<(String, Node)> = board
        .rows()
        .into_iter()
        .filter(|r| matches!(r.node, Node::Image(_)))
        .map(|r| (r.label, r.node))
        .collect();
    let nodes: std::collections::HashSet<&Node> = images.iter().map(|(_, node)| node).collect();
    assert_eq!(nodes.len(), images.len(), "{images:?}");
    assert_eq!(images.len(), 4, "the same untagged image listed twice is one row");
    assert_eq!(images[0], ("postgres:<none>".into(), Node::Image("e29b".into())));
    assert_eq!(
        images[1],
        (
            "postgres:17.11-alpine".into(),
            Node::Image("postgres:17.11-alpine".into())
        )
    );
    assert_eq!(images[3], ("22bb".into(), Node::Image("22bb".into())));
}
