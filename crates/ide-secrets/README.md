# ide-secrets

The one place the app keeps secrets: CI tokens (`ide-ci`), the own agent's provider keys (`ide-agent`) and the dictation keys (`ide-ui-dictation`). The keychain items live under the app's service name, one account per secret; tests and the self-test keep them in memory and leave the keychain alone (R4).

```mermaid
flowchart LR
    ci["ide-ci: Auth"] -->|"SecretStore::get, set, delete"| store{{"dyn SecretStore"}}
    agent["ide-agent: key_for"] --> store
    dictation["ide-ui-dictation: Keys"] --> store
    store -->|"the app"| keychain["Keychain: security-framework generic passwords under the service"]
    store -->|"tests, self-test"| memory["Memory"]
    keychain -->|"errSecItemNotFound"| none["Ok(None)"]
    keychain -->|"any other status"| error["SecretError: status, message"]
```

The API is documented in [`src/lib.rs`](src/lib.rs).
