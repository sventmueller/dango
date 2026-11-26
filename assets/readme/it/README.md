<p><img src="../../readme-icon.png" alt="Icona di Dango" width="147" height="147"></p>

# Dango

**L’editor nativo per macOS per lavorare con gli agenti di programmazione.**

Dango riunisce il codice, gli agenti che lo modificano e tutto ciò che li circonda in una finestra per cartella: un editor con server di linguaggio e previsioni durante la digitazione, terminali, un browser, Git, CI, un debugger e thread AI con Claude, Codex, Antigravity, OpenCode o un agente integrato nell’app. Ogni agente lavora nella stessa cronologia, chiede prima di modificare file o eseguire comandi e lascia un punto di ripristino prima di ogni prompt, così puoi rivedere e riavvolgere ciò che ha fatto, messaggio per messaggio. Nulla di ciò che una cartella configura viene eseguito prima che tu ti fidi di essa, e il testo esterno arriva a un agente contrassegnato come non attendibile. Detta nell’editor e nel campo del messaggio; il riconoscimento vocale può restare sul tuo Mac. Dango è scritto in Rust su veri controlli AppKit, senza Electron, parla 35 lingue e non ha telemetria.

[![Scarica Dango.dmg](https://img.shields.io/badge/Scarica_Dango-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Scarica Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: firmato e autenticato da Apple, per macOS 14 o versioni successive su Apple Silicon e Intel. Il link sarà disponibile dalla prima pubblicazione.

**Hai trovato un bug?** [Apri una issue](https://github.com/sventmueller/dango/issues/new) e indica con quale agente e in che tipo di progetto si è verificato.

[![Rust, edizione 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org) [![macOS 14 o successivo](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/) ![Universale: Apple Silicon e Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white) [![AppKit tramite objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2) [![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com) [![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/) [![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/) [![Terminale: Tapa](https://img.shields.io/badge/terminal-Tapa-F46D01)](https://github.com/sventmueller/tapa) ![35 lingue dell’interfaccia](https://img.shields.io/badge/languages-35-4C9A2A) ![Nessuna telemetria](https://img.shields.io/badge/telemetry-none-4C9A2A) ![Firmato con Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A) [![Licenza: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licenza)

[Bahasa Indonesia](../id/README.md) · [Bahasa Melayu](../ms/README.md) · [Català](../ca/README.md) · [Čeština](../cs/README.md) · [Dansk](../da/README.md) · [Deutsch](../de/README.md) · [English](../../../README.md) · [Español](../es/README.md) · [Français](../fr/README.md) · [Hrvatski](../hr/README.md) · **Italiano** · [Magyar](../hu/README.md) · [Nederlands](../nl/README.md) · [Norsk](../no/README.md) · [Polski](../pl/README.md) · [Português (Brasil)](../pt_BR/README.md) · [Português (Portugal)](../pt_PT/README.md) · [Română](../ro/README.md) · [Slovenčina](../sk/README.md) · [Slovenščina](../sl/README.md) · [Suomi](../fi/README.md) · [Svenska](../sv/README.md) · [Tiếng Việt](../vi/README.md) · [Türkçe](../tr/README.md) · [Ελληνικά](../el/README.md) · [Русский](../ru/README.md) · [Українська](../uk/README.md) · [עברית](../he/README.md)‎ · [العربية](../ar/README.md)‎ · [हिन्दी](../hi/README.md) · [ไทย](../th/README.md) · [한국어](../ko/README.md) · [日本語](../ja/README.md) · [简体中文](../zh-Hans/README.md) · [繁體中文](../zh-Hant/README.md)

## Indice

- [Funzioni in breve](#funzioni-in-breve)
- [Requisiti](#requisiti)
- [Installazione](#installazione)
- [Primi passi](#primi-passi)
- [Agenti AI](#agenti-ai)
  - [La cronologia](#la-cronologia)
  - [Permessi e modalità](#permessi-e-modalità)
  - [Punti di ripristino e worktree](#punti-di-ripristino-e-worktree)
- [Editor](#editor)
- [Server di linguaggio](#server-di-linguaggio)
- [Test](#test)
- [Debugger](#debugger)
- [Terminale](#terminale)
- [Browser](#browser)
- [Git](#git)
- [CI](#ci)
- [Container](#container)
- [Markdown](#markdown)
- [Dettatura](#dettatura)
- [Fiducia](#fiducia)
- [Sandbox](#sandbox)
- [Aspetto e accessibilità](#aspetto-e-accessibilità)
- [Lingue](#lingue)
- [Abbreviazioni da tastiera](#abbreviazioni-da-tastiera)
- [Privacy e sicurezza](#privacy-e-sicurezza)
- [Dove si trovano i dati](#dove-si-trovano-i-dati)
- [Limiti noti](#limiti-noti)
- [Compilare dal codice sorgente](#compilare-dal-codice-sorgente)
- [Licenza](#licenza)

## Funzioni in breve

- **Tutti gli agenti in una cronologia.** Claude tramite il suo comando `claude`, Codex, Antigravity e OpenCode tramite l’Agent Client Protocol, altri dal registro ACP e un agente integrato nell’app per Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama e LM Studio. Risposte, chiamate agli strumenti, richieste di permesso, piani, cose da fare, subagenti e task in background hanno lo stesso aspetto, qualunque agente sia in esecuzione.
- **Rivedere e riavvolgere.** Prima di ogni prompt, Dango conserva un punto di ripristino. Ciò che un agente ha modificato si può rivedere e riavvolgere per messaggio, e un thread può lavorare in un proprio worktree Git e consegnare il lavoro con un’unione o un cherry-pick.
- **Una finestra per cartella.** File, terminali, pagine web, thread AI, diff e anteprime sono pannelli di macOS di quella finestra. La barra laterale contiene File, Ricerca, Agenti, Git, Test, Debug, CI e Container.
- **Un vero editor di codice.** Colori della sintassi, più punti di inserimento, compressione del codice, `.editorconfig`, server di linguaggio per 17 linguaggi con completamento, vai alla definizione, riferimenti, rinomina, correzioni rapide e gerarchie, e previsioni durante la digitazione da GitHub Copilot o da un modello locale.
- **Debugger, Git e CI integrati.** Punti di interruzione ed esecuzione passo passo per Rust, C, C++, Swift, Go, JavaScript, TypeScript e Python; mettere in stage singoli blocchi, branch, stash, conflitti e un grafo dei commit; esecuzioni, job e log da GitHub Actions e GitLab CI.
- **Il terminale di Tapa.** Blocchi di comandi di cui copi, salvi o passi a un agente l’output, un campo di input con proposte dalla tua cronologia e dal progetto, la ricerca in tutta la cronologia, e notifiche e avanzamento dei programmi che vi girano.
- **Prudente con le tue cartelle.** Una cartella di cui non ti fidi non avvia nulla: né processi Git, né server di linguaggio, né hook, né agenti. I comandi Git distruttivi non vengono mai consentiti per sempre, e i commit vengono controllati alla ricerca di segreti.
- **Dettatura.** Parla nell’editor e nel campo del messaggio, online tramite OpenAI o Groq oppure in locale con Whisper e Parakeet.
- **Nativo fino in fondo.** Finestre, pannelli, menu e barre degli strumenti AppKit, TextKit 2 per l’editor, Metal per il terminale, WebKit solo nel pannello del browser. Modalità scura, Aumenta contrasto e VoiceOver funzionano come in ogni app per Mac.
- **Nella tua lingua.** L’interfaccia è disponibile in 35 lingue: arabo, catalano, cinese (semplificato), cinese (tradizionale), croato, ceco, danese, olandese, inglese, finlandese, francese, tedesco, greco, ebraico, hindi, ungherese, indonesiano, italiano, giapponese, coreano, malese, norvegese, polacco, portoghese (Brasile), portoghese (Portogallo), rumeno, russo, slovacco, sloveno, spagnolo, svedese, tailandese, turco, ucraino e vietnamita.

## Requisiti

- macOS 14 Sonoma o versioni successive, su Apple Silicon o Intel. Dango viene sviluppato e testato su macOS 27.
- Per i thread AI, l’agente che vuoi usare; vedi [Agenti AI](#agenti-ai).
- Per i server di linguaggio e gli adattatori di debug, che Dango installa quando lo chiedi: Go per gopls e `dlv`; Node.js per i server di TypeScript, Python, JSON, YAML, CSS, HTML e shell e per il debugger JavaScript; i Xcode Command Line Tools per clangd, sourcekit-lsp e `lldb-dap`. rust-analyzer viene scaricato dalle sue release su GitHub.
- Per le previsioni durante la digitazione, un piano GitHub Copilot (basta quello gratuito) o un modello fill-in-the-middle in Ollama o LM Studio.

## Installazione

**Dall’immagine disco.** Ogni versione è un’immagine disco, `Dango-<version>.dmg`, con un’app universale per Apple Silicon e Intel, firmata con un Developer ID e autenticata da Apple. Aprila e trascina Dango nella cartella Applicazioni. Un Cask di Homebrew installa la stessa immagine. Non è ancora stata pubblicata alcuna versione.

**Aggiornamenti.** Dango installato dall’immagine disco si mantiene aggiornato con [Sparkle](https://sparkle-project.org). Al secondo avvio chiede se può cercare aggiornamenti da solo; finché non acconsenti, non effettua alcuna richiesta. **Dango › Verifica aggiornamenti…** controlla subito. Gli aggiornamenti sono firmati con la chiave propria dell’app, che Sparkle verifica prima di installare qualsiasi cosa. Una versione compilata da te non ha l’aggiornamento automatico.

**Dal codice sorgente.** Vedi [Compilare dal codice sorgente](#compilare-dal-codice-sorgente), poi sposta `target/release/Dango.app` in `/Applications`.

**Il comando da terminale.** **Dango › Installa il comando “dango”…** scrive un piccolo script in `~/.local/bin/dango`, senza diritti di amministratore; `~/.local/bin` deve essere nel tuo `PATH`. Poi `dango .` apre la cartella corrente e `dango file.rs` un file. Dango non sostituisce mai lì un file che non ha scritto lui stesso.

## Primi passi

1. **Apri una cartella** con ⌘O o `dango <folder>`. La prima volta Dango chiede se ti fidi di essa (vedi [Fiducia](#fiducia)).
2. **La barra laterale** passa tra File (⌘1), Ricerca (⌘2), Agenti (⌘3), Git (⌘4), Test (⌘5), Debug (⌘6), CI (⌘7) e Container (⌘8); un clic destro nasconde quelli che non usi, e ⌃⌘S chiude la barra laterale.
3. **Ogni documento è un pannello** della finestra della cartella, i pannelli propri di macOS. Il “+” alla fine della barra dei pannelli e ⌘T offrono un nuovo file, un terminale, il browser e ogni agente installato, anche in un proprio worktree Git; tenendo premuto il “+” compare lo stesso come menu.
4. **Apertura rapida** con ⌘P trova i file, e con `#` i simboli del progetto; ⇧⌘F cerca e sostituisce nel progetto.
5. **Parla con un agente** in un pannello AI. ⌘Esc porta il focus della tastiera nel suo campo del messaggio e indietro.
6. **Impostazioni** con ⌘, contiene i pannelli Generali, Terminale, CI, Agenti, Software Factory, Agente integrato, Server di linguaggio, Dettatura e Previsione.

La barra degli strumenti contiene indietro e avanti tra i punti che hai modificato (⌃- e ⌃⇧-), il branch, Apertura rapida e la panoramica Attività, che elenca tutto ciò che è in esecuzione. **Apri a lato** nel menu contestuale di un file mostra un secondo documento accanto al primo. Dopo un riavvio o un arresto anomalo ogni finestra torna con i suoi pannelli, e gli editor con testo non salvato tornano con quel testo. Immagini, audio, video, font e documenti Office si aprono in un pannello tramite Quick Look, i PDF tramite PDFKit.

## Agenti AI

| Agente | Come viene eseguito | Di cosa ha bisogno |
|---|---|---|
| Claude | il comando `claude`, con cui Dango parla direttamente tramite il suo protocollo stream-json | `claude` installato e con accesso effettuato; Dango cerca nel `PATH` della shell di login, in `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` e `/usr/local/bin`. L’accesso avviene solo in `claude` stesso |
| Codex | `codex-acp` tramite l’Agent Client Protocol (ACP) | Node.js e npm; Dango installa il pacchetto a versione fissata quando lo chiedi |
| Antigravity | `agy acp` | il tuo `agy` nel `PATH`, oppure il download a versione fissata, installato quando lo chiedi |
| OpenCode | `opencode acp` | il tuo `opencode` nel `PATH`, oppure il download a versione fissata, installato quando lo chiedi |
| Altri agenti ACP | dal registro ACP, scaricato solo quando lo chiedi | ciò che indica la voce del registro; le voci senza checksum si installano solo dopo la tua conferma |
| Agente integrato | integrato nell’app, con strumenti per file, ricerca, comandi, diagnostica e richieste web | una chiave API per Anthropic, OpenAI, Google Gemini o OpenRouter, conservata nel portachiavi; Ollama o LM Studio su questo Mac senza chiave; qualsiasi server che parli l’API di OpenAI o di Anthropic |

**Impostazioni › Agenti** mostra quali agenti ha questo Mac, installa gli altri e i loro aggiornamenti e apre il registro ACP. Gli agenti che Dango installa sono fissati a versioni testate e verificati con i checksum pubblicati (npm integrity o SHA-256); Dango non esegue pacchetti `npx` senza versione.

### La cronologia

Ogni agente lavora in un pannello AI con la stessa cronologia: risposte, chiamate agli strumenti con il loro output, richieste di permesso, domande, piani da approvare e commentare (⌥⌘K), cose da fare, subagenti e task in background. Due pulsanti nell’intestazione del pannello elencano i subagenti e i processi in background in esecuzione. Menziona un file, `@terminal` o `@browser` per passarne il contenuto all’agente. Un punto verde sul pannello indica una sessione al lavoro, uno rosso una sessione che ti aspetta, e una notifica avvisa quando termina o attende mentre guardi altrove. Per Claude, la riga di stato mostra il contesto e l’utilizzo.

### Permessi e modalità

I nuovi thread iniziano nella modalità “Manuale”: l’agente chiede prima di modificare file o eseguire comandi. Per i comandi Git distruttivi (`push --force`, `reset --hard`, `clean -fd`) Dango non offre “Consenti sempre”. L’agente integrato conserva le sue regole “Consenti sempre” per progetto, le elenca in **Impostazioni › Agente integrato** e chiede ogni volta per i comandi Git distruttivi, qualunque cosa sia salvata; Claude e gli agenti ACP conservano tali regole nella propria configurazione. Con “Consenti tutto” e senza sandbox, il thread mostra un avviso permanente.

### Punti di ripristino e worktree

Prima di ogni prompt, Dango conserva un punto di ripristino, come commit sotto `refs/ide-checkpoints/` nel repository del progetto, o in una propria cartella per un progetto senza Git; un normale push o fetch non li porta mai con sé. Ciò che un agente ha modificato si può rivedere e riavvolgere per messaggio. Un thread può lavorare in un proprio worktree Git: il riquadro Git mostra cosa ha modificato e ne riprende il lavoro con un’unione o un cherry-pick.

## Editor

- **Colori della sintassi, numeri di riga e indicatori delle modifiche** rispetto all’ultimo commit, trova e sostituisci, Apertura rapida (⌘P) e ricerca nel progetto (⇧⌘F).
- **Il rientro** segue il file e `.editorconfig`: tabulazioni o spazi, rientro automatico, coppie di parentesi.
- **Più punti di inserimento:** ⌥-clic, ⌥-trascina per una colonna, ⌃⇧↑ e ⌃⇧↓, ⌘D per l’occorrenza successiva.
- **Compressione** (⌥⌘← e ⌥⌘→, con ⇧ per tutto), attiva o disattiva commento (⌘/), vai alla riga (⌃G), spazi visibili e a capo automatico (⌥Z).
- **I file mantengono la codifica**, il byte order mark e i fine riga. Un file modificato sul disco viene ricaricato se l’editor non ha testo non salvato; altrimenti una barra offre le differenze e un’unione.
- **Previsioni durante la digitazione.** Mentre digiti compare una proposta in grigio, e Tab la accetta. Viene da GitHub Copilot o da un modello locale in Ollama o LM Studio, secondo la scelta in **Impostazioni › Previsione**; un modello locale mantiene il tuo codice su questo Mac.
- **Minimappa e Scorrimento fisso** (menu Editor): un’immagine stretta dell’intero file accanto al testo, e le righe di intestazione dei blocchi in cui si trova la riga in alto, fissate sopra di essa.
- **Diagnostica a fine riga** (menu Vista): il messaggio più grave di ogni riga dopo un secondo di pausa, con un simbolo per la sua gravità.
- **Anteprima dal vivo delle pagine statiche:** Editor › Apri in un pannello del browser serve le pagine di una cartella attendibile da Dango stesso e ricarica il pannello quando un file viene salvato.
- **Prettier** formatta JavaScript, TypeScript, CSS, HTML, Markdown e altro dove il progetto lo configura, con il `node_modules/.bin/prettier` del progetto stesso; l’impostazione `formatter` sceglie il formattatore per ogni linguaggio.
- **Modalità Vim** (Editor › Modalità Vim): modalità Normale, Inserisci, Visuale e Sostituisci, conteggi, operatori con movimenti e oggetti di testo, `.`, registri, marcatori e macro. `u` e ⌃R usano l’annullamento dell’editor, e ogni inserimento conta come un solo passo. `"+` e `"*` sono gli appunti; l’impostazione `vimClipboard` collega agli appunti anche il registro senza nome. `/`, `?`, `n`, `N`, `*`, `:s` e `:%s` cercano con le espressioni regolari di Rust, non con i pattern di Vim: niente `\v`, e `\b` al posto di `\<` e `\>`. I comandi Ex sono `:w`, `:q`, `:wq`, `:x`, `:noh` e `:<line>`. In modalità Normale ⌃G passa a Vim invece che a vai alla riga.
- **Sincronizzazione delle impostazioni** tramite una cartella di iCloud Drive (**Impostazioni › Generali**): le tue impostazioni e abbreviazioni da tastiera ti seguono sugli altri tuoi Mac; percorsi, indirizzi locali e consensi restano su ciascuno.

## Server di linguaggio

Go, Rust, TypeScript e JavaScript, Python, C, C++ e Objective-C, Swift, script di shell, JSON, YAML, CSS, HTML, TOML, Markdown e Lua, più server tuoi da **Impostazioni › Server di linguaggio**. Dango installa un server quando lo chiedi, nella propria cartella degli strumenti, e usa il `tsc` di un progetto TypeScript quando può fare da server di linguaggio.

Cosa offrono: diagnostica (⌘' e ⇧⌘' passano da una all’altra), informazioni sul simbolo (⌃⌘I), completamento (⌥Esc), aiuto sulle firme, vai alla definizione (⌃⌘J), alla definizione del tipo e all’implementazione, riferimenti (⌃⇧⌘F), gerarchie di chiamate e di tipi (⌃⇧⌘H), rinomina (⌃⌘E), formattazione (⌥⇧F), colori semantici, compressione e la struttura (⇧⌘O); tipi e nomi dei parametri nella riga; correzioni rapide e refactoring dietro una lampadina (⌥⌘A); i simboli del progetto con ⌥⌘O.

Accanto al server proprio di un file, ESLint controlla il JavaScript e il TypeScript dei progetti che lo usano, con le sue correzioni rapide, e le abbreviazioni Emmet compaiono nel completamento per HTML, CSS e JSX. Dango li installa entrambi quando lo chiedi; **Impostazioni › Generali** corregge i problemi di ESLint al salvataggio, se vuoi.

## Test

Il riquadro Test elenca i test di una cartella per pacchetto e file con il loro stato: `cargo test` o cargo-nextest, Go, pytest, Vitest e Jest. Un simbolo accanto a ogni test nel margine lo esegue, ⌥-clic ne esegue il debug, e un’asserzione non riuscita ne segna la riga. Il menu Debug esegue il test nel punto di inserimento (⌃⌥⌘U), di nuovo gli ultimi test (⌃⌥⌘G) o tutti, ed esegue il debug del test nel punto di inserimento. I test vengono eseguiti solo nelle cartelle attendibili, perché eseguono il codice del progetto; `cargo test` elenca i suoi test quando lo chiedi, perché elencarli compila ogni programma di test.

## Debugger

Punti di interruzione, anche con condizioni e come logpoint, esecuzione passo passo, thread con i loro stack di chiamate, variabili ed espressioni di controllo nell’Inspector (⌥⌘I), valori accanto al codice mentre il programma è in pausa, e una console di debug che valuta nel frame in pausa.

| Linguaggio | Adattatore di debug |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` dai Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | il debugger JavaScript, su Node.js |
| Python | debugpy |

Il file `.vscode/launch.json` di un progetto viene letto. Il menu Debug usa le abbreviazioni di Xcode: ⌘\\ imposta un punto di interruzione, ⌘Y li attiva o disattiva tutti, ⌃⌘Y continua, F6, F7 e F8 sono Esegui istruzione/routine, Esegui istruzione ed Esci da istruzione/routine, ⌘. interrompe.

## Terminale

I terminali si aprono come pannelli (**File › Nuovo terminale**, ⌃\`) con la shell di login e il suo ambiente. Il terminale è quello di [Tapa](https://github.com/sventmueller/tapa), con il proprio core VT, disegnato con Metal: xterm e VT520 con il protocollo di tastiera di Kitty, i link OSC 8, l’output sincronizzato, il truecolor e la segnalazione del mouse, nella dimensione del testo dell’editor e con i colori chiari e scuri di Tapa. La sua integrazione con la shell per zsh, bash e fish si configura da sé senza toccare i tuoi file di configurazione e ti segue quando ti colleghi a un altro host con `ssh`. Dopo un riavvio un terminale torna come una nuova shell nella cartella in cui si trovava, solo nelle cartelle attendibili; ciò che mostrava non torna.

- **Blocchi di comandi.** Ogni comando e il suo output formano un blocco. ⌘↑ e ⌘↓ saltano da un prompt all’altro. **Seleziona blocco** (⇧⌘A), **Copia output** (⌥⌘C), **Salva output…** (⌥⌘S) e **Comprimi/espandi blocco** (⇧⌘↩) agiscono sul blocco corrente; **Aggiungi/rimuovi segnalibro** (⇧⌘M) contrassegna una riga, e ⇧⌘↑ e ⇧⌘↓ saltano da un segnalibro all’altro. Con **Contrassegna i comandi come blocchi** attivo, una barra nel margine mostra ogni blocco, i comandi non riusciti vengono segnalati, il comando in esecuzione resta in alto mentre il suo output scorre, e alcuni pulsanti sopra il blocco sotto il puntatore ne mostrano lo stato, la durata e le stesse azioni.
- **Invia blocco all’agente** (menu Modifica e menu contestuale del terminale) apre un nuovo pannello AI con la riga di comando e l’output del blocco corrente allegati come Contenuto esterno (vedi [Fiducia](#fiducia)), e una bozza che chiede la causa quando il comando non è riuscito, come fa il log di un job CI fallito. Non ha una scorciatoia finché non ne assegni una a `terminal.blockToAgent` in `keybindings.json`.
- **Il campo di input.** **Vista › Mostra/nascondi campo di input** (⌘I) trasforma la riga di comando in un campo di testo in fondo al terminale, con sopra la cartella e il branch Git; i nuovi terminali seguono la tua ultima scelta. A capo invia l’intero campo alla shell. Richiede l’integrazione con la shell e si fa da parte davanti ai programmi a schermo intero come vim, htop o claude.
- **Le proposte** appaiono in grigio subito dopo ciò che digiti, nel campo di input e senza di esso: dalla tua cronologia, con la cartella, il branch e il codice di uscita con cui è stato eseguito ogni comando, dalle specifiche di oltre 700 strumenti da riga di comando, e dal progetto, come i branch git, i container docker, gli script di `package.json` e i target di make, just e cargo. → accetta una proposta; Tab completa e, con più candidati, apre un menu che la digitazione filtra. Prima che tu digiti, il campo può proporre il comando che di solito viene dopo. I comandi sono colorati secondo le loro parti, e uno che la tua shell non può eseguire riceve una sottolineatura rossa tratteggiata. Le proposte non usano mai la rete e avviano programmi solo dal `PATH` della shell di login.
- **Trova** (⌘F, ⌘G e ⇧⌘G quando un terminale ha il focus) cerca in tutta la cronologia, con espressioni regolari, maiuscole/minuscole e parole intere, e conta i risultati; la barra di scorrimento segnala i risultati, i prompt e i comandi non riusciti.
- **Selezionare e copiare.** Un doppio clic seleziona per intero un URL, un percorso, un hash o un indirizzo IP, e il trascinamento con ⌥ seleziona un rettangolo. **Seleziona con la tastiera** (⇧⌘C) sposta una selezione con i tasti di Vim, **Copia con formattazione** (⇧⌥⌘C) mantiene colori e stili. Un programma può copiare con OSC 52 solo mentre il suo terminale ha il focus della tastiera, e il terminale lo segnala; nessun programma legge gli appunti.
- **Incollare.** Un testo che eseguirebbe un comando al suo a capo, o che contiene caratteri di controllo, viene mostrato prima in una finestra di dialogo, a meno che il programma non lo legga come incolla tra parentesi (bracketed paste). Incollare un’immagine la passa a un agente di programmazione in esecuzione nel terminale, che la legge da sé dagli appunti, oppure incolla il percorso di una copia salvata tra le cache; tramite `ssh` il terminale dice perché non lo fa. I file trascinati su un terminale, immagini comprese, arrivano come percorsi tra virgolette; un’immagine trascinata da altrove, come una pagina web, segue la strada di una incollata.
- **Link.** ⌘-clic apre un indirizzo web nel pannello del browser, un link `mailto:` in Mail e un nome di file, anche con la sua riga e colonna, nell’editor di Dango; i link vengono riconosciuti anche senza OSC 8. I link di altro tipo non si aprono.
- **Server di sviluppo.** Quando un programma nel terminale avvia un server di sviluppo, una barra sopra il terminale ne indica l’indirizzo, e **Apri in un pannello del browser** lo apre lì.
- **Notifiche e avanzamento.** Il campanello e una notifica chiesta da un programma (OSC 9 e OSC 777) mettono il punto rosso sul pannello del terminale finché non lo vedi; mentre un’altra app è in primo piano, la notifica va anche a macOS, e un clic su di essa riporta al terminale. L’avanzamento che un programma segnala (OSC 9;4) appare come un anello sul suo pannello e come una barra sull’icona nel Dock.
- **Input da tastiera sicuro** (**Dango › Input da tastiera sicuro**) impedisce alle altre app di leggere ciò che digiti in un terminale; si attiva anche da sé mentre un programma chiede una password.
- **Tasti.** ⌘K pulisce il terminale (**Modifica › Pulisci terminale**); ⌃C ed Esc vanno al programma che vi gira. I comandi del terminale si trovano nei menu Modifica e Vista, e le loro scorciatoie agiscono solo mentre un terminale ha il focus della tastiera, così ⌘I, ⌘↑ o ⌘F mantengono il loro significato nell’editor. Il menu contestuale apre anche un **Nuovo terminale qui** e chiude il terminale con **Chiudi area**.

**Impostazioni › Terminale** attiva o disattiva il campo di input per i nuovi terminali, i suoi tasti Vim, le proposte, i colori dei comandi, la sottolineatura di quelli sconosciuti, i blocchi di comandi, la copia immediata della selezione, la visualizzazione preventiva degli incolla su più righe e ⌥ come tasto Meta.

## Browser

Un pannello del browser con campo dell’indirizzo (⌘L), indietro e avanti (⌘[ e ⌘]), ricarica (⌘R), ricerca, download che chiedono prima e l’inspector web. `@browser` passa la pagina a un agente, contrassegnata come testo esterno.

## Git

Il riquadro Git mette in stage e scarta file o singoli blocchi, crea commit, fa commit e push o sincronizza, cambia e crea branch, conserva stash, risolve conflitti di unione e disegna un grafo dei commit recenti. Blame mostra chi ha modificato per ultimo la riga sotto il cursore, oppure una colonna per l’intero file. Un pulsante scrive il messaggio di commit con l’AI; il diff parte solo dopo che gitleaks non vi ha trovato alcun segreto, e ogni commit viene controllato anch’esso alla ricerca di segreti.

## CI

GitHub Actions e GitLab CI, anche GitHub Enterprise Server e GitLab autogestito:

- esecuzioni, job e log del branch corrente;
- rieseguire job falliti, un singolo job o, su GitHub, un’intera esecuzione; avviare job manuali o approvare deployment in attesa; avviare una nuova pipeline o un workflow avviabile a mano; annullare;
- le pull request o merge request aperte e le issue, che si aprono nel pannello del browser;
- il log di un job fallito passato a un agente, contrassegnato come testo esterno.

Il token proviene dal portachiavi, da `gh` o `glab`, dall’ambiente della shell di login, oppure è uno che inserisci in **Impostazioni › CI**.

## Container

Il riquadro Container mostra i progetti Compose di Docker con i loro servizi, i singoli container, le immagini e i volumi, e segue gli eventi di Docker mentre accadono. Un file Compose della cartella appare come progetto anche prima che esista uno dei suoi container. I container si avviano, si arrestano e si riavviano; i loro log e una shell si aprono in pannelli del terminale, e una porta pubblicata si apre nel pannello del browser. Eliminare un container, un’immagine o un volume, e pulire i dati inutilizzati, chiedono prima conferma. I comandi up, down e pull di Compose vengono eseguiti solo nelle cartelle attendibili, perché Compose compila ed esegue ciò che il progetto descrive. Il menu del riquadro sceglie il contesto Docker della finestra senza cambiare quello usato dalle tue shell; gli stessi comandi si trovano nel menu Container. Serve la riga di comando `docker`, Docker 24.0.6 o versione successiva.

## Markdown

- **Anteprima.** Un file Markdown si apre con la sua anteprima a lato; **Vista › Mostra/nascondi anteprima a lato** (⇧⌘V) la chiude e la apre, e **Impostazioni › Generali** disattiva l’anteprima automatica. L’anteprima disegna formule, diagrammi Mermaid, Graphviz e PlantUML e le immagini della cartella; le immagini dal web si caricano solo in una cartella attendibile o quando le richiedi nell’anteprima.
- **Menu Formato:** grassetto (⌘B), corsivo (⌘I), barrato (⇧⌘X), codice (⌥⇧⌘C), link (⇧⌘K), intestazioni 1–3 (⌥⌘1–⌥⌘3), elenchi puntati e numerati, attività, citazioni e blocchi di codice. Ogni comando attiva o disattiva, sulla selezione o sulle righe, ed è un solo passo di annullamento.
- **Incolla.** Il testo da Word o da una pagina web arriva come Markdown, le sue immagini come file accanto al documento; ⌥⇧⌘V incolla testo semplice.
- **Esporta.** **File › Esporta** scrive un PDF, un singolo file HTML con le immagini incorporate o un documento Word. Le immagini dal web vengono incluse solo da una cartella attendibile.
- **Apri in Splat.** Se [Splat](https://github.com/sventmueller/splat) è installato, **File › Apri in Splat** e i menu contestuali dell’elenco dei file e dei pannelli aprono lì il documento, dopo averlo salvato.

## Dettatura

Detta nel campo del messaggio di un pannello AI o nell’editor: con il pulsante del microfono lì presente, **Modifica › Avvia dettatura** (⌃⌥D) o il tasto fn (tienilo premuto per registrare, premilo due volte per continuare a registrare). Il riconoscimento vocale è quello di [Lowspeak](https://github.com/sventmueller/lowspeak); non serve avere installato Lowspeak.

- **Impostazioni › Dettatura** imposta modalità e modello (online con OpenAI o Groq, in locale con Whisper o Parakeet), la lingua, la post-elaborazione, il microfono e il tuo vocabolario.
- **I modelli locali** vengono scaricati da Hugging Face quando ne scegli uno, con l’avanzamento e la possibilità di annullare. Finiscono nelle cartelle che usa Lowspeak, così ciascuna app usa ciò che l’altra ha scaricato. Un modello locale mantiene la registrazione su questo Mac.
- **La trascrizione online** richiede una chiave per OpenAI o Groq, che Dango conserva in un proprio elemento del portachiavi.
- **Da Lowspeak.** Se Lowspeak è configurato diversamente dalle impostazioni predefinite, il pannello Dettatura propone una volta di adottarne le impostazioni, e le chiavi su richiesta; macOS chiede prima che Dango ne legga una (scegli “Consenti”, non “Consenti sempre”).
- **Permessi.** macOS chiede l’accesso al microfono alla prima dettatura. Il tasto fn funziona mentre Dango è l’app attiva e Lowspeak non è in esecuzione; richiede il permesso di accessibilità, che Dango chiede solo dopo la prima dettatura, mai all’avvio.

## Fiducia

Una cartella può far eseguire programmi non appena vi si avvia qualcosa: hook Git e impostazioni del repository come `core.fsmonitor`, server di linguaggio da `node_modules`, hook degli agenti, `.mcp.json`. La prima volta che apri una cartella, Dango chiede se ti fidi di essa.

- **Attendibile:** Git, server di linguaggio, debugger, agenti e CI vengono eseguiti per questa cartella e tutto ciò che contiene.
- **Limitata:** puoi leggere e modificare; gli indicatori delle modifiche nell’editor derivano dalla lettura diretta di `.git`. Nulla di ciò che la cartella configura viene avviato. I riquadri Git, CI e AI lo segnalano e propongono di fidarsi della cartella.

Un testo che non proviene da te può contenere istruzioni pensate per guidare un agente (prompt injection). L’output di un terminale (`@terminal`, o un blocco di comandi passato a un agente), una pagina web (`@browser`), un log CI passato a un agente, un file fuori dal progetto e un testo più lungo incollato da una pagina web vengono quindi inviati contrassegnati: l’etichetta dice “Contenuto esterno”, e l’agente riceve il testo con l’avviso che si tratta di dati non attendibili di cui non deve seguire le istruzioni. Le pagine web dell’agente integrato arrivano con lo stesso avviso. Un testo incollato conta come proveniente da una pagina web quando il browser lo indica negli appunti, come fanno Safari, Chrome e Firefox; il testo da altre fonti, e una frase breve di poche parole, resta tuo.

## Sandbox

Dango stesso viene eseguito senza App Sandbox, perché ogni programma che avvia la erediterebbe: terminali, Git, server di linguaggio e agenti non potrebbero funzionare. Invece, ogni thread AI ha un interruttore per la sandbox, disattivato per impostazione predefinita. Ciò che limita dipende dall’agente, e il suggerimento dell’interruttore lo spiega:

| Agente | Con la sandbox attiva |
|---|---|
| Claude | i comandi di shell vengono eseguiti nella sandbox propria di Claude; i suoi strumenti per file e web no |
| Codex | comandi e modifiche scrivono solo nel progetto, senza rete |
| Agente integrato | file, richieste web e comandi restano entro il progetto e le cartelle consentite, senza rete (`sandbox-exec`) |
| Antigravity, OpenCode | nessuna sandbox che Dango possa attivare |

## Aspetto e accessibilità

- **La modalità scura** segue il sistema; **Aumenta contrasto** e **Riduci movimento** vengono rispettati.
- **VoiceOver** dà un nome alla barra laterale, ai pannelli, alle schede della cronologia AI, alle righe di Git e CI e al debugger, legge le righe del terminale e annuncia il nuovo output. I punti di interruzione non si distinguono mai solo per il colore.
- **Tastiera.** Ogni comando ha un’abbreviazione, e tutte si possono modificare in `keybindings.json`; ⌥⌘\` passa al riquadro successivo.
- **Da destra a sinistra.** In arabo ed ebraico la finestra viene rispecchiata, mentre le viste del codice e la griglia del terminale restano da sinistra a destra.

Cosa aggiunge Dango ad AppKit, area per area, è descritto in [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Lingue

L’interfaccia è tradotta in 35 lingue: arabo, catalano, cinese (semplificato), cinese (tradizionale), croato, ceco, danese, olandese, inglese, finlandese, francese, tedesco, greco, ebraico, hindi, ungherese, indonesiano, italiano, giapponese, coreano, malese, norvegese, polacco, portoghese (Brasile), portoghese (Portogallo), rumeno, russo, slovacco, sloveno, spagnolo, svedese, tailandese, turco, ucraino e vietnamita.

Dango segue l’ordine delle lingue in **Impostazioni di Sistema › Generali › Lingua e zona**; lì, sotto **Applicazioni**, puoi anche scegliere una lingua solo per Dango. Se non c’è una traduzione per la tua lingua, usa l’inglese. I termini seguono un glossario, [docs/glossary.md](../../../docs/glossary.md), e le traduzioni di Apple, così Dango si legge come il resto del tuo Mac.

## Abbreviazioni da tastiera

| Azione | Abbreviazione |
|---|---|
| Apri cartella, nuova finestra, chiudi finestra | ⌘O, ⇧⌘N, ⇧⌘W |
| Nuovo file, salva, salva con nome | ⌘N, ⌘S, ⇧⌘S |
| Nuovo pannello, chiudi pannello, pannello successivo e precedente | ⌘T, ⌘W, ⇧⌘] e ⇧⌘[ |
| Apertura rapida, cerca nel progetto | ⌘P, ⇧⌘F |
| File, Ricerca, Agenti, Git, Test, Debug, CI, Container | da ⌘1 a ⌘8 |
| Mostra o nascondi la barra laterale, riquadro successivo | ⌃⌘S, ⌥⌘\` |
| Indietro e avanti tra i punti modificati | ⌃- e ⌃⇧- |
| Attiva o disattiva commento, vai alla riga | ⌘/, ⌃G |
| Occorrenza successiva, punto di inserimento sopra e sotto | ⌘D, ⌃⇧↑ e ⌃⇧↓ |
| Comprimi, espandi, comprimi tutto, espandi tutto | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| A capo automatico, Inspector | ⌥Z, ⌥⌘I |
| Completamento, informazioni sul simbolo, vai alla definizione | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Riferimenti, gerarchia delle chiamate, rinomina | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Simboli nel file, nel progetto | ⇧⌘O, ⌥⌘O |
| Correzione rapida, formatta | ⌥⌘A, ⌥⇧F |
| Problema successivo e precedente | ⌘' e ⇧⌘' |
| Anteprima Markdown, incolla come testo semplice | ⇧⌘V, ⌥⇧⌘V |
| Grassetto, corsivo, barrato, codice, link | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Intestazione da 1 a 3 | da ⌥⌘1 a ⌥⌘3 |
| Nuovo terminale, pulisci il terminale | ⌃\`, ⌘K |
| In un terminale: trova, trova successivo, trova precedente | ⌘F, ⌘G, ⇧⌘G |
| In un terminale: prompt precedente e successivo, segnalibro precedente e successivo, aggiungi o rimuovi un segnalibro | ⌘↑ e ⌘↓, ⇧⌘↑ e ⇧⌘↓, ⇧⌘M |
| In un terminale: seleziona blocco, copia output, salva output, comprimi blocco | ⇧⌘A, ⌥⌘C, ⌥⌘S, ⇧⌘↩ |
| In un terminale: campo di input, seleziona con la tastiera, copia con formattazione | ⌘I, ⇧⌘C, ⇧⌥⌘C |
| Focus sul campo del messaggio dell’agente, commenta un piano | ⌘Esc, ⌥⌘K |
| Campo dell’indirizzo, ricarica, indietro, avanti (browser) | ⌘L, ⌘R, ⌘[, ⌘] |
| Avvia o interrompi la dettatura | ⌃⌥D |
| Punto di interruzione, attiva o disattiva tutti i punti di interruzione | ⌘\\, ⌘Y |
| Esegui il test nel punto di inserimento, riesegui gli ultimi test | ⌃⌥⌘U, ⌃⌥⌘G |
| Continua, esegui istruzione/routine, esegui istruzione, esci da istruzione/routine, interrompi | ⌃⌘Y, F6, F7, F8, ⌘. |
| Impostazioni | ⌘, |

`keybindings.json` nella cartella dei dati dell’app (vedi [Dove si trovano i dati](#dove-si-trovano-i-dati)) ne modifica qualsiasi.

## Privacy e sicurezza

- **Nessuna telemetria,** nessuna analisi, nessun rapporto di arresto anomalo inviato da nessuna parte. Gli errori finiscono nel log di sistema (Console, sotto il nome dell’app).
- **La rete solo per ciò che avvii tu:** gli agenti che usi, ciascuno con il proprio servizio; il registro ACP e i download di agenti, server di linguaggio, adattatori di debug e modelli di dettatura quando li chiedi; gli host CI a cui accedi; le previsioni di Copilot se lo hai scelto; i provider dell’agente integrato; la trascrizione online se l’hai scelta; le pagine che apri nel pannello del browser; e la verifica degli aggiornamenti una volta che l’hai consentita.
- **Il codice lascia il Mac** solo dove hai scelto un servizio per farlo: un agente, le previsioni di Copilot, un messaggio di commit scritto con l’AI. Il diff per un messaggio di commit parte solo dopo che gitleaks non vi ha trovato alcun segreto.
- **Chiavi e token** restano nel portachiavi di macOS, mai in un file di impostazioni. Le chiavi di altre app, come quelle di Lowspeak, vengono lette solo se acconsenti, e una sola volta.
- **Download fissati.** Agenti, server di linguaggio e adattatori di debug che Dango installa sono fissati a versioni testate e verificati con i loro checksum.
- **Firma del codice** con l’Hardened Runtime; gli aggiornamenti si installano solo con la firma propria dell’app.

## Dove si trovano i dati

`<id>` è l’identificatore del bundle, `local.agentide` per la versione pubblicata (`local.agentide.debug` per le build di debug). Non contiene il nome visualizzato, quindi un cambio di nome conserva tutti i dati.

| Posizione | Contenuto |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`, `terminal-history.sqlite` (la cronologia dei comandi dei terminali); `threads/` (l’elenco del riquadro AI), `agent-threads/` (conversazioni dell’agente integrato), `layouts/` (pannelli per progetto), `recovery/` (editor non salvati dopo un arresto anomalo), `checkpoints/` (per cartelle senza Git), `worktrees/`, `agents/` e `tools/` (agenti, server di linguaggio e adattatori di debug installati), `task-output/` |
| `~/Library/Caches/<id>/` | immagini incollate, script di integrazione con la shell, il registro ACP; si può eliminare |
| `~/Library/Logs/<id>/dictation/` | i file di log della dettatura |
| `~/Library/Application Support/Lowspeak` e `FluidAudio` | modelli di dettatura locali, condivisi con Lowspeak |
| `~/Library/WebKit/<id>/` | cookie e dati dei siti web del pannello del browser |
| `<project>/.dango/settings.json` | le impostazioni proprie del progetto |
| il repository Git del progetto | punti di ripristino, come commit sotto `refs/ide-checkpoints/`; un normale push o fetch non li porta mai con sé |
| portachiavi, servizio `<id>` | chiavi API dell’agente integrato e della dettatura, token CI |
| `~/.local/bin/dango` | il comando da terminale, se l’hai installato |

Claude conserva le proprie sessioni in `~/.claude/projects`; Dango le legge per riprendere una sessione, ma non le modifica.

## Limiti noti

- **Nessuna App Sandbox** per l’app stessa, perché terminali, Git, server di linguaggio e agenti la erediterebbero; una cartella deve essere attendibile prima che venga eseguito qualsiasi cosa configuri. Antigravity e OpenCode non hanno una sandbox che Dango possa attivare.
- **I dati di utilizzo e la suddivisione del contesto di Claude** si basano su parti del suo protocollo contrassegnate come sperimentali, e vengono nascosti quando mancano.
- **Riavvolgere la conversazione** funziona solo con Claude; con gli altri agenti viene riavvolto solo il codice.
- **L’accesso alla CI tramite il browser** è disattivato; usa `gh`, `glab`, un token o l’ambiente.
- **Nessuna immagine nel terminale:** la grafica di Kitty, Sixel e le immagini di iTerm2 non vengono mostrate.
- **Gemini CLI non viene offerto:** Google ha interrotto l’accesso con un account personale e rimanda ad Antigravity. I thread Gemini precedenti restano leggibili.

## Compilare dal codice sorgente

Ti servono:

- macOS 14 o versioni successive con i Xcode Command Line Tools (`xcode-select --install`); Xcode in sé non serve.
- Rust come indicato da `rust-toolchain.toml`. Il `rust` di Homebrew compila per il processore di questo Mac; la build universale di una versione richiede rustup con i target `aarch64-apple-darwin` e `x86_64-apple-darwin`.
- Copie di [splat](https://github.com/sventmueller/splat), [lowspeak](https://github.com/sventmueller/lowspeak) e [tapa](https://github.com/sventmueller/tapa) accanto a questa (`../splat`, `../lowspeak`, `../tapa`), ciascuna sul proprio branch `main`; il workspace ne prende i crate tramite percorso, il terminale da Tapa.
- cmake, per ogni build con la dettatura (`make run`, `make build`, `make release`), che compila whisper.cpp. `make check` compila senza.

| Comando | Cosa fa |
|---|---|
| `make run` | compila l’app di debug (`target/debug/Dango Debug.app`) e la apre |
| `make build` | compila l’app di release (`target/release/Dango.app`), firmata con un certificato locale che la prima build crea nel portachiavi di login; lascia `Cargo.toml` invariato |
| `make lint` | formattazione e clippy; gli avvisi lo fanno fallire |
| `make check` | `lint`, poi tutti i test; non richiede portachiavi, finestra né rete |
| `make test` | `check`, poi l’autotest `probe` e la verifica dell’accessibilità, da sinistra a destra e da destra a sinistra, nell’app reale |
| `make perf` | i budget di tempo, in una build di release; significativo solo su un Mac altrimenti inattivo |
| `make release` | una prova della pubblicazione in `target/release-dry-run/`, firmata con il certificato locale; non invia nulla da nessuna parte |
| `make release-notarized` | `check`, poi la pubblicazione vera e propria in `dist/`: firma con Developer ID, autenticazione, il feed degli aggiornamenti firmato con la chiave propria dell’app; lascia `Cargo.toml` invariato |
| `make deploy` | su un `main` pulito, con o senza sigillo di CI: autentica la versione successiva, crea il commit di `Cargo.toml`, carica i file della pubblicazione, invia il codice e il tag di versione, con il sigillo verde di CI se c’è, e poi pubblica da sé la versione |

Debug e release sono app separate con impostazioni e dati propri. La compilazione, i test, gli autotest, la pubblicazione e le regole che il codice segue sono descritti in [docs/development.md](../../../docs/development.md), le decisioni che ci stanno dietro in [docs/plan.md](../../../docs/plan.md). Le note di rilascio sono [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), tradotte accanto a ogni README in `assets/readme/<language>/`.

## Licenza

Dango è distribuito con una delle seguenti licenze

- Licenza Apache, versione 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Licenza MIT ([LICENSE-MIT](../../../LICENSE-MIT))

a tua scelta.

Salvo diversa indicazione esplicita, qualsiasi contributo inviato intenzionalmente da te per l’inclusione in Dango, come definito nella licenza Apache-2.0, sarà distribuito con la doppia licenza di cui sopra, senza termini o condizioni aggiuntivi.
