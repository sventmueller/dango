# splat-render: the preview's render path, shared by splat and vibeedit

What both apps use to turn a Markdown layout into native text with formulas, diagrams and pictures (E1, Phase 9, R3,
E14). Until splat 1.0.14 (`ef97dde`) all of it lived in splat's app crate (`src/editor/`); splat's branch
`vibeedit/render` moves it into two crates of splat's workspace without changing what splat does. Line numbers and
limits below are those of 1.0.14.

## Where the parts came from

| Part | splat file | Lines | Window? |
|---|---|---|---|
| Queue of render jobs: turns, retries, eviction | `crates/splat-editor-state/src/render_queue.rs` | 580 | no |
| Formulas and diagrams found in a document, shared TeX macros | `crates/splat-markdown/src/doc/figures.rs` | – | no |
| Layout, `Resolver` (`math_ready`, `diagram_ready`), source ↔ preview map | `crates/splat-preview` | – | no |
| Hidden WebKit renderer, render keys, listeners | `src/editor/js.rs` | 748 | yes |
| WebKit calls: offline rules, bundled page, scripts, PDF of a rect | `src/editor/webkit.rs` | 300 | yes |
| `Rendered`, `Sizing`, memory budget of all decoded pixels | `src/editor/picture.rs` | 113 | yes |
| Client of the sandboxed decoding service (XPC) and the service itself | `src/editor/decoder.rs`, `decoder/{protocol,serve}.rs`, `safe_image.rs`, `util/xpc.rs`, `src/decoder_main.rs` | 1 790 | service |
| Layout → `NSAttributedString` (fonts, text blocks, tables, attachments) | `src/editor/render.rs` (+ `attributes.rs`, parts of `style.rs`) | 1 650 | yes |
| Local image files: store, loading per volume, re-reading changed files, memory pressure | `src/editor/images.rs` (+ `util/volumes.rs`) | 720 + 266 | yes |
| Pictures from https | `src/editor/remote.rs` | 500 | yes |
| Document context: typographic quotes, repository, bibliography, embedded notes | `src/editor/env.rs` | 596 | partly |
| Component order of the editor controller | `src/editor/controller/layers.rs` | 444 | yes |

The plan's list (E1, Phase 9) misses one dependency: every picture the render path shows passes splat's **decoding
service** (`SplatDecoder.xpc`, sandboxed, no files, no network). MathJax's SVG becomes a PDF there (usvg + krilla),
WebKit's diagram PDF and every image file become pixels there (ImageIO), so the app itself never parses untrusted bytes.
Sharing the render path means sharing the service.

## Crates

Two new crates in splat's workspace, both macOS-only, both outside `check-library` and the `LIBRARY` coverage list:

- **`splat-decoder`** — protocol, service side (`serve`, `safe_image`), client side (`decode`, `decode_native`,
  `svg_to_pixels`, `pdf_to_pixels`, `image`, `release_idle`), the libxpc calls. Depends on objc2-app-kit, usvg, krilla,
  image; **not** on WebKit, so the sandboxed service binary does not load it. Each app's service binary is a
  three-line `main` calling `splat_decoder::service::main()`.
- **`splat-render`** — everything else above that needs a window: renderer, WebKit calls, pictures, text building,
  images, remote pictures, volumes. Depends on splat-decoder, splat-editor-state (render queue, lanes, links,
  repository), splat-preview, splat-markdown, splat-i18n, objc2-web-kit.

What stays in splat's app: `env.rs` (its user-defaults key, bibliographies and embedded notes are splat's document
context; the crate needs only `may_carry_along`, which `RenderCtx::leaves_the_mac` takes), `controller/layers.rs` and the
image grant banner of `images.rs` (sandbox grants; vibeedit runs without a sandbox, E2). Both adapt to the crate's
`Viewer`; neither moves. Splat's `util` re-exports what moved (`volumes`, attribute helpers), so callers elsewhere
in splat keep their paths.

## Setup

```rust
splat_render::start(Host::of_main_bundle(report));   // or Host { resources, report, first_retry, web_session }
splat_render::images::watch_memory(mtm);
```

- **`Host::resources`** — the folder holding `render.html` and `js/`; the page loads from there with read access to
  that folder only, and the navigation policy allows nothing but that one file. Both apps keep splat's layout
  (`Contents/Resources/render.html`, `Contents/Resources/js/`).
- **`Host::report`** — where failures go (`what`, `detail`), never with document text; splat keeps its `Splat: …` log
  lines, vibeedit sends them to `diag::report`.
- **`Host::first_retry`, `Host::web_session`** — the first pause before a failed picture from the web is tried again
  (30 s), and a hook on the session that loads them; splat's self-test serves the web from inside the app through it.
- **Texts** — the crate uses `splat_i18n::t`/`tf` with German keys, looked up in the main bundle's
  `Localizable.strings` ("Formel: {}", "Diagramm: {}", "Darstellung fehlgeschlagen", link and spoiler tooltips).
  splat's translation test already scans every crate root; vibeedit's `tests/translations.rs` adds
  `../splat/crates/splat-render/src` to what it scans.
- **Decoding service** — found as `<main bundle id>.decoder`, unchanged. A service binary that measures nothing is
  `fn main() { splat_decoder::service_main() }`; splat keeps its own, which writes coverage.

## Render jobs

```rust
pub enum Key { Math { tex, display, dark, macros: Arc<str> }, Diagram { source, kind: DiagramKind, dark } }
pub struct Wanted { pub keys: Vec<Key>, pub macros: Arc<str> }
pub fn wanted(doc: &Doc, dark: bool) -> Wanted;          // splat's former update.rs rule
pub struct Ready { pub dark: bool, pub macros: Arc<str> } // impl splat_preview::layout::Resolver (math/diagram only)

pub trait Viewer {                                       // main thread; one per document, held weakly
    fn renders_arrived(&self);
    fn image_arrived(&self, url: String);                // a picture from the web arrived or failed
    fn image_loaded(&self, path: PathBuf, file: ImageFile);
    fn images_changed(&self, changed: Vec<(PathBuf, Stamp)>);
    fn web_images_shown(&self) -> Vec<String>;           // what failed downloads are tried again for
    fn has_closed(&self) -> bool;
}

pub fn js::renderer(mtm) -> Rc<Renderer>;                // created on first use, lives as long as the app
impl Renderer {
    pub fn listen(&self, viewer: &Rc<dyn Viewer>) -> Owner;
    pub fn request(self: &Rc<Self>, owner: Owner, keys: Vec<Key>);
}
pub fn splat_render::stops_showing(viewer: &Rc<dyn Viewer>); // js::forget + remote::stops_showing
pub fn js::asks_for_nothing(viewer: &Rc<dyn Viewer>);  pub fn js::trim();
pub fn js::cached(key: &Key) -> Option<Rendered>;       // main thread
pub fn js::ready(key: &Key) -> bool;                    // any thread, for a layout off the main thread
pub fn js::outcome(key: &Key) -> Option<Result<(), String>>; // None while queued; Err with MathJax's/Mermaid's message
```

A key is the whole input of a picture: the source, display or inline, the appearance and — for formulas — every
`\newcommand` the document defines (`macros_of`). `wanted` also asks for the other appearance when a document has at most
200 figures, so switching to dark mode shows pictures at once. `Owner` stays in `splat-editor-state::render_queue`;
`js` re-exports it. The interface draft had a `Listener` trait wrapped in a `Viewer` struct; one trait object, held
weakly by the crate, does the same with one type less.

## Invalidation

1. The text changes: the host lays out again (`splat_preview::layout` with `Ready`), so a formula whose key is ready
   is an attachment and one that is not shows as its source in code style.
2. After a pause in typing (0.35 s in splat) the host calls `request(owner, wanted(doc, dark).keys)`.
3. When the queue has run empty, every listener gets **one** `renders_arrived`; the host lays out again on a later
   turn of the run loop (it may be inside its own rendering when the call comes) and the formulas appear.
4. Appearance or macros change → other keys → the same path; nothing needs to be dropped by hand.
5. Local images: `ImageStore::file` answers `None` and reads the file on a lane of its volume; `image_loaded` hands the
   result back, the host calls `store.finish(path, file)` and re-renders the blocks showing it. The host's 2-second
   timer calls `look_at_images(open)` with each viewer and its store, which compares stamps and reports `images_changed`; a missing image
   is searched for again every 15th look.
6. Pictures from https: `remote::get(url, Some(viewer))` answers `None` and downloads; `image_arrived(url)`
   follows. Failures that may pass are retried with a doubling pause up to 15 minutes.

## Cancellation

| Event | Effect |
|---|---|
| `request` with a new list | Keys nobody wants any more leave the queue; the one WebKit is drawing finishes and is cached. |
| `asks_for_nothing` | The document has no figures any more; does not start the renderer. |
| `stops_showing` (document closed) | Its queue goes, keys another document wants pass to that one, its results count as unused; downloads only it waited for leave the queue or stop. |
| A render takes > 10 s | Counts as failed; a new web view takes over (a looping script keeps its web process, WebKit cannot end it). |
| The page does not load in 10 s | New web view, up to 3 tries; then formulas stay source text for this run. |
| WebKit's process dies | The page loads again; the key that was drawing gets one more try, a second loss marks it failed. |
| A decode takes > 10 s or too much memory | The service is ended from outside and the picture fails; a helper that dies otherwise gets one more try with a fresh one. |

Answers that arrive after their job was given up are dropped by job number, never applied to a newer key.

## Caching and memory

- In memory only, no disk cache. At most 1 500 results or 256 MB of pixels; beyond that the results no open document
  asked for in its last 8 requests go. What a document shows is never evicted.
- All decoded pixels (formulas, diagrams, images, downloads) share one budget of 1/8 of physical memory: past half of
  it new pictures decode at 1 280 px, past all of it at 640 px, past twice of it not at all.
- On memory pressure (`watch_memory`): smaller decodes, idle decoder helpers end, `trim`, unused downloads go.

## Threading and main-thread rules

- `Renderer`, `ImageStore`, `remote`, `RenderCtx` and every `Viewer` are main-thread only (thread-locals and `Rc`).
  `ready` is the only call meant for other threads.
- WebKit runs one job at a time; owners take turns, so one document full of slow diagrams does not hold up another's
  formulas.
- SVG → PDF → pixels runs on the crate's background lane, never on the main thread: decoder calls block on a socket.
  Up to 3 helpers per kind; image files beyond the start-up disk read on a lane of their volume, so a hanging network
  volume holds up only its own pictures.
- Every Objective-C callback and completion block runs inside `guarded`; results hop back with `main_soon`.

## The hidden WebKit renderer

- One `WKWebView` in a borderless `NSPanel` at (−30 000, −30 000), 1 600 × 1 200, alpha 0, ignoring the mouse,
  excluded from the Windows menu. WebKit measures text only inside a window; a panel keeps a click on the Dock icon
  working, which counts every other visible window as open.
- Non-persistent data store; content rules block every load but `file:`, `data:`, `blob:`, `about:`; `render.html`
  has its own CSP (`connect-src 'none'`). The page loads 16 ms after the view exists, so a key pressed at launch is
  not held up.
- `render.js` loads the libraries with the first formula or diagram: MathJax 4 (2.4 MB), Mermaid (5.3 MB),
  Graphviz/viz.js (1.4 MB), PlantUML (3.2 MB + 0.3 MB themes). `renderMath` answers an SVG with its size in ex,
  `renderDiagram` the rect it drew in, which WebKit turns into a transparent PDF.
- `mathlive.min.js`, `mathlive-fonts/` and `mathfield-page.js` belong to splat's formula field, not to this crate;
  vibeedit does not bundle them (1.1 MB less, about 12.9 MB in its bundle, R6).

## Text

`RenderCtx { mtm, zoom, doc_dir, images, max_image_width, dark, requester: Option<&Rc<dyn Viewer>>, leaves_the_mac:
Option<CarriesAlong>, macros, drawn: &dyn Drawn }` with `render_block(&TopBlock) -> NSMutableAttributedString` as
before: splat's fonts, text blocks for code, quotes and rules, `NSTextTable` for tables, alerts, task markers,
attachments sized by `Sizing` (formulas in ex of the line's font). `Drawn` hands over what others make: pictures from
the web and what WebKit drew; `render::Live` is splat's, which downloads every picture. `leaves_the_mac` is the export's
rule which local pictures may go along (`env::may_carry_along`, which stays in splat). Needs a TextKit 1 text view, as
text blocks and tables lay out nowhere else; E7's TextKit 2 rule concerns the editor. `attachment(&Rendered, &NSFont,
max_width)` is public for text built elsewhere.

## What changes in splat

Code moves, behaviour does not: the same constants, the same order of calls, the same log lines. `EditorController`
gets a small `Viewer` (`Shown`) that forwards to what it called before (`renders_arrived`, `image_arrived`,
`Message::ImageLoaded`, `Message::ImagesChanged`). Unit tests move with their code; `ImageFile` moved to `picture.rs`,
so the crate's modules still depend downwards only. The guards that scan `src/` — "only the service hands bytes to the
system's decoders" (now in `tests/structure.rs`), "the app calls no private method", "everything the system calls is
guarded" — also scan the two crates. The service binary stays `splat-decoder`, the bundle layout stays. Gates on branch
`vibeedit/render`: `make check` (fmt, clippy both feature sets, check-library, all tests, vendor check),
`cargo xtask selftest probe`, `make test-gui` (gui, resize, typing).

## How vibeedit uses it

- **Preview tab** (`src/preview/`): a Markdown file opens with its preview in a panel to the right of its editor
  (E59), which "Vorschau daneben ein-/ausblenden" (⇧⌘V) closes and opens. Typing lays the text out again 0.12 s after a key on a background lane, with `Ready`; the
  formulas and diagrams are asked for after a 0.35 s pause; arrivals lay out or render again on a later turn. Every
  block goes through `RenderCtx::render_block` into a TextKit 1 view, `doc_dir` = the file's folder, a timer looks at
  the picture files every 2 s while a preview is open. Editor and preview scroll together by paragraph through
  `splat_preview::map`. The whole document renders per update; splat's progressive rendering stays in splat, so very
  large files (several MB) update more slowly than in splat.
- **Pictures from the web (S4):** in a trusted folder they load as in splat. In an untrusted one the preview's `Drawn`
  never asks `remote`: the picture shows as a placeholder and a bar offers "Bilder aus dem Netz laden", which loads
  them for that document.
- **AI answers** keep their own text builder (`src/agent/markdown.rs`) and turn `Atom::Math`/`Atom::Diagram` into
  attachments once `cached(key)` has them, else code as before. All answers ask as one viewer
  (`src/agent/figures.rs`, at most 400 keys); when more of them are drawn, the open timelines build their cards anew.
  A plan for approval keeps them as code, as its comments point into its text.
- **PDF** files open in a tab of their own with PDFKit's `PDFView`, **HTML** files beside their editor in the browser
  tab, which shows files of the window's folder only; neither uses this crate.
- **Bundle:** xtask copies `render.html` and `js/` from `../splat/resources` without `mathlive.min.js`,
  `mathlive-fonts/`, `mathfield-page.js` and `LICENSE-mathlive`, and adds the libraries' licenses to the
  acknowledgements. The service `Decoder` in `SERVICES` runs the program `ide-decoder` (`src/decoder_main.rs`,
  splat-decoder alone, so no WebKit) from `Contents/XPCServices/Decoder.xpc` (identifier: the build's with `.decoder`,
  `local.agentide.decoder` for the release), signed with the App Sandbox and nothing else before the app. The JS stays
  out of vibeedit's repository.

## Decisions

- **S1 Two crates** — `splat-decoder` and `splat-render`, so the sandboxed service links no WebKit.
- **S2 Decoding service in vibeedit** — launchd starts it from the bundle, so it is no child process of the
  `Supervisor` (E10, E23) and needs no login-shell environment; an accepted deviation from E23. Decoding in the app
  would hand images from any cloned repository to ImageIO in an unsandboxed process.
- **S3 `env.rs` and `layers.rs` stay in splat**, adapted to the crate, instead of moving as the plan's list says.
- **S4 Pictures from the web in untrusted folders** — placeholders until the user asks for them, per document;
  trusted folders load them as splat does.
