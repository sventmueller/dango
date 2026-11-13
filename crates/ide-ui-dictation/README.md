# ide-ui-dictation

Dictation into the app's text fields, the AI composer and the editor, through lowspeak (E13, E99): the microphone
button and line beside a field, the menu item, fn as in Lowspeak, the app's own API keys and the questions about them.
One recording runs at a time, bound to the field that had the focus when it started; its text goes in at that field's
cursor as one undo step. The app registers each field as a `DictationField` and holds the dictations in its context
through `Host`; the crate knows no editor and no AI tab. Without the feature `engine` only that trait and the
shortcut's spelling remain, so builds without dictation never compile whisper.cpp. See [docs/dictation.md](../../docs/dictation.md).

```mermaid
flowchart LR
    app["app: editor, composer"] -->|"register(DictationField, text, Mic, line)"| hub["Hub: places by Target"]
    press["mic button, menu item, fn"] -->|"press, toggle_focused, begin_with_fn"| begin{"microphone allowed, key there?"}
    begin -->|"no: ask"| ask["microphone question, ask: take Lowspeak's key over or enter one"]
    begin -->|"yes"| desk["Desk: phases per field"]
    desk -->|"Act::Start, Stop"| engine["engine: lowspeak Dictation"]
    engine -->|"Event, via main_soon"| desk
    desk -->|"Act::Insert"| insert{"field still open?"}
    insert -->|"yes"| field["insertText as one undo step"]
    insert -->|"no"| clipboard["clipboard + DictationField::notice"]
    desk -->|"Act::Show, Failed, NoSpeech"| controls["Mic and line: listening, transcribing, note"]
```

The API is documented in [`src/lib.rs`](src/lib.rs).
