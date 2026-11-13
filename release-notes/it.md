<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Questa è la prima versione di Dango, un editor nativo per macOS pensato per lavorare con gli agenti di programmazione.
Editor, terminali, un browser, Git, CI, un debugger e i thread AI condividono un’unica finestra per cartella. L’app è
scritta in Rust su AppKit; WebKit compare solo nel pannello del browser. L’interfaccia è disponibile in 35 lingue.

## Requisiti
- macOS 14 o versioni successive, su Apple silicon o Intel.
- Per i thread AI, l’agente che scegli: Claude richiede il comando `claude` con accesso effettuato; Codex richiede
  Node.js, e l’app installa il suo pacchetto in una versione fissata quando lo chiedi; Antigravity e OpenCode usano il
  tuo `agy` o `opencode`, oppure un download in una versione fissata; l’agente integrato richiede una chiave API, oppure
  Ollama o LM Studio su questo Mac.
- I server di linguaggio e gli adattatori di debug vengono installati quando lo chiedi. gopls e `dlv` richiedono Go; i
  server per TypeScript, Python, JSON, YAML, CSS, HTML e shell e il debugger JavaScript richiedono Node.js; clangd,
  sourcekit-lsp e `lldb-dap` sono inclusi negli Xcode Command Line Tools.
- Le previsioni di modifica richiedono un abbonamento a GitHub Copilot (basta quello gratuito), oppure un modello
  fill-in-the-middle in Ollama o LM Studio.

## Finestra
- Ogni file, terminale, pagina del browser, thread AI, vista delle differenze e anteprima è un pannello macOS nella
  finestra della cartella. Il “+” alla fine della barra dei pannelli e ⌘T offrono un nuovo file, un terminale, il
  browser e ogni agente installato, anche in un worktree Git dedicato.
- La barra laterale passa tra File, Ricerca, Agenti, Git, CI e Debug (⌘1–⌘6); con un clic destro nascondi quelli che non
  usi. La barra strumenti contiene indietro e avanti tra i punti che hai modificato (⌃- e ⌃⇧-), il branch, Apertura
  rapida e la panoramica Attività.
- “Apri a lato” mostra un secondo documento accanto al primo. Dopo un riavvio, o un arresto anomalo, ogni finestra torna
  con i suoi pannelli, e gli editor con testo non salvato tornano con quel testo.
- Immagini, audio, video, font e documenti Office si aprono in un pannello tramite Quick Look, i PDF tramite PDFKit.

## Editor
- Colori della sintassi, numeri di riga, segni di modifica rispetto all’ultimo commit, trova e sostituisci, Apertura
  rapida (⌘P) e ricerca nel progetto (⇧⌘F).
- Rientro automatico, tabulazioni o spazi secondo `.editorconfig`, coppie di parentesi, attivazione e disattivazione dei
  commenti, raggruppamento del codice, vai alla riga, spazi visibili e a capo automatico.
- Più cursori: ⌥-clic, ⌥-trascinamento per una colonna, ⌘D per l’occorrenza successiva.
- I file mantengono la codifica, il byte order mark e i fine riga. Un file modificato su disco viene ricaricato se
  l’editor non contiene testo non salvato; altrimenti una barra offre le differenze e l’unione.
- **Previsioni di modifica:** mentre scrivi, una proposta appare in grigio; Tab la accetta. Le proposte arrivano da
  GitHub Copilot o da un modello locale in Ollama o LM Studio, a seconda di cosa scegli nelle impostazioni; un modello
  locale mantiene il tuo codice su questo Mac.

## Server di linguaggio
Go, Rust, TypeScript e JavaScript, Python, C, C++ e Objective-C, Swift, script di shell, JSON, YAML, CSS, HTML, TOML,
Markdown e Lua, più server personali dalle impostazioni. Diagnostica, informazioni al passaggio del puntatore,
completamento, aiuto sulle firme, vai alla definizione, alla definizione di tipo e all’implementazione, riferimenti,
gerarchie delle chiamate e dei tipi, rinomina, formattazione, colori semantici, raggruppamento e struttura; tipi e nomi
dei parametri all’interno della riga; correzioni rapide e refactoring dietro una lampadina (⌥⌘A); i simboli del progetto
con ⌥⌘O o `#` in Apertura rapida. Il `tsc` di un progetto TypeScript viene usato quando può fare da server di
linguaggio.

## Debugger
Breakpoint, anche con condizioni, esecuzione passo passo, thread con i rispettivi stack di chiamate, variabili ed
espressioni osservate nell’Inspector (⌥⌘I), valori accanto al codice mentre il programma è in pausa, e una console di
debug che valuta le espressioni nel frame in pausa. Rust, C, C++ e Swift tramite `lldb-dap`, Go tramite `dlv`,
JavaScript e TypeScript tramite il debugger JavaScript, Python tramite debugpy. Il file `.vscode/launch.json` del
progetto viene letto. Il menu Debug usa le abbreviazioni da tastiera di Xcode.

## Terminale
Terminali nei pannelli con l’ambiente della shell di login, disegnati con Metal. Incollando o trascinando un’immagine,
la si passa a un agente di programmazione in esecuzione nel terminale, oppure si incolla il percorso di una copia
salvata. ⌘-clic apre link e nomi di file. Un server di sviluppo avviato dal terminale può essere aperto nel pannello del
browser.

## Markdown
Un file Markdown si apre con l’anteprima accanto (⇧⌘V), con formule, diagrammi Mermaid, Graphviz e PlantUML e le
immagini della cartella. Un menu Formato per grassetto, corsivo, link, titoli, elenchi, citazioni e blocchi di codice;
il testo incollato da Word o da una pagina web arriva come Markdown. **File › Esporta** scrive un PDF, un singolo file
HTML o un documento Word. Se Splat è installato, **File › Apri in Splat** apre lì il documento.

## Browser
Un pannello del browser con campo indirizzo, indietro e avanti, ricerca, download con conferma preventiva e Web
Inspector.

## Git
Un riquadro Git per mettere in stage e annullare le modifiche di file o singoli blocchi, commit, commit e push o
sincronizzazione, branch, stash, conflitti di unione e un grafico dei commit recenti. Blame mostra chi ha modificato per
ultimo la riga sotto il cursore, oppure una colonna per l’intero file. Un pulsante scrive il messaggio del commit con
l’AI; le differenze vengono inviate solo dopo che gitleaks non vi ha trovato alcun segreto, e anche ogni commit viene
controllato alla ricerca di segreti. Un thread AI può lavorare in un worktree Git dedicato; il riquadro mostra cosa ha
modificato e riprende il lavoro tramite unione o cherry-pick.

## CI
GitHub Actions e GitLab CI, anche GitHub Enterprise Server e GitLab self-managed: esecuzioni, job e log del branch
corrente; riesecuzione dei job non riusciti, di un singolo job o (su GitHub) di un’intera esecuzione, avvio di job
manuali o approvazione dei deployment in attesa, avvio di una nuova pipeline o di un workflow avviabile manualmente, e
annullamento. Le merge request o pull request aperte e le issue sono elencate nello stesso riquadro e si aprono nel
pannello del browser. Il log di un job non riuscito può essere passato a un agente, contrassegnato come testo esterno.
Il token proviene dal Portachiavi, da `gh` o `glab`, dall’ambiente della shell di login, oppure lo inserisci tu.

## AI
Ogni agente lavora in un pannello AI con la stessa timeline: risposte, chiamate agli strumenti con il loro output,
richieste di permesso, domande, piani da approvare e commentare, cose da fare, subagenti e task in background; due
pulsanti nell’intestazione del pannello elencano i subagenti e i processi in background in esecuzione. Prima di ogni
prompt l’app salva un punto di ripristino; le modifiche si possono rivedere e riavvolgere messaggio per messaggio. I
nuovi thread partono in modalità “Manuale”, e i comandi Git distruttivi non vengono mai consentiti in modo permanente.
La panoramica Attività elenca tutto ciò che è in esecuzione. Un punto verde sul pannello indica una sessione al lavoro,
uno rosso una sessione che ti aspetta, e una notifica ti avvisa quando una sessione finisce o resta in attesa mentre
guardi altrove.
- **Claude** tramite il comando `claude`, senza modifiche, con sessioni, subagenti, task in background, contesto e
  utilizzo.
- **Agenti ACP:** Codex, Antigravity e OpenCode, e altri dal registro ACP quando lo chiedi; gli agenti installati sono
  fissati a versioni testate e verificati con i rispettivi checksum.
- **Agente integrato:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio e i server che parlano l’API di
  OpenAI o di Anthropic, con strumenti per file, ricerca, comandi, diagnostica e richieste web; le chiavi restano nel
  Portachiavi.
- Un interruttore sandbox per thread limita ciò che i comandi di un agente possono fare, nella misura in cui l’agente lo
  supporta.
- Il testo esterno, come l’output del terminale, una pagina web o un log CI, arriva all’agente contrassegnato come non
  attendibile.

## Dettatura
Dettatura nel campo dei messaggi AI e nell’editor tramite il riconoscimento vocale di Lowspeak: il pulsante del
microfono, ⌃⌥D o il tasto fn mentre Dango è l’app attiva. Usa le impostazioni e i modelli di Lowspeak; la trascrizione
online richiede una chiave OpenAI o Groq.

## Aggiornamenti
L’app si aggiorna tramite Sparkle. Al secondo avvio chiede se può verificare gli aggiornamenti da sola; finché non
acconsenti, non invia alcuna richiesta. **Dango › Verifica aggiornamenti…** esegue subito la verifica.

## Accessibilità
Nomi VoiceOver per la barra laterale, i pannelli, le schede della timeline AI, le righe Git e CI, il debugger e il testo
del terminale; controllo completo da tastiera con abbreviazioni modificabili in `keybindings.json`; Aumenta contrasto e
Riduci movimento; le viste del codice restano da sinistra a destra anche nelle interfacce da destra a sinistra. I
dettagli si trovano in ACCESSIBILITY.md.

## Privacy
Nessuna telemetria, nessuna analisi e nessun rapporto sugli arresti anomali. L’app si connette solo per ciò che avvii
tu: gli agenti che usi, i download che richiedi, gli host CI a cui accedi, le previsioni di Copilot se le hai scelte, la
trascrizione online se l’hai scelta, le pagine che apri e la verifica degli aggiornamenti dopo che l’hai consentita.
Chiavi e token restano nel Portachiavi di macOS.

## Limiti noti
- L’accesso alla CI tramite il browser è disattivato; usa `gh`, `glab`, un token o l’ambiente.
- L’app funziona senza App Sandbox, perché terminali, Git, server di linguaggio e agenti la erediterebbero; devi fidarti
  di una cartella prima che venga eseguito qualcosa che essa configura. Antigravity e OpenCode non hanno una sandbox che
  l’app possa attivare.
- I dati di utilizzo di Claude e la suddivisione del contesto si basano su parti del suo protocollo contrassegnate come
  sperimentali e vengono nascosti quando mancano. Il riavvolgimento della conversazione funziona solo con Claude; per
  gli altri agenti viene riavvolto solo il codice.
- Gemini CLI non è disponibile: Google ha terminato l’accesso con un account personale e rimanda ad Antigravity. I
  thread Gemini precedenti restano leggibili.
