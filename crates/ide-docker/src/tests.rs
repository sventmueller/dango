//! The readers against Docker's output. `contexts-29.9.3.jsonl` is a recording of Docker 29.9.3; the `*-format`
//! files follow the documented `--format '{{json .}}'` objects of Docker 24.0.6 and later, field by field, until
//! recordings of the oldest and the newest Docker replace them (A10).

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
            kind: "container".into(),
            action: "start".into(),
            id: "3f2a1c".into()
        }
    );
    assert_eq!(events[1].kind, "volume");
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
    let root = std::env::temp_dir().join(format!("ide-docker-{}", std::process::id()));
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
