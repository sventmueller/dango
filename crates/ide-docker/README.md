# ide-docker

Managing containers, images, volumes and Compose projects through the `docker` command line (E136), without AppKit and without starting processes. It builds each command's arguments, every one with `--context`, reads what `--format '{{json .}}'` prints one object per line, and models the Containers panel's tree. Compose projects come from the containers' labels and the folder's Compose files, not from `docker compose ls`.

```mermaid
flowchart TB
    panel["Containers panel"] -->|"list_containers, list_images, list_volumes"| command["Command: docker --context …"]
    panel -->|"watch_events, restarted with backoff"| events["docker events, JSON lines"]
    command -->|"the app runs it through its supervisor"| output["JSON lines"]
    output -->|"containers(), images(), volumes()"| board["Board: Compose projects, containers, images, volumes"]
    events -->|"event()"| panel
    board -->|"rows()"| panel
```

The API is documented in [`src/lib.rs`](src/lib.rs).
