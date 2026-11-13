<p><img src="../../readme-icon.png" alt="Pictograma Dango" width="168" height="168"></p>

# Dango

**Editorul nativ pentru macOS pentru lucrul cu agenți de programare.**

Dango aduce codul, agenții care îl modifică și tot ce îi înconjoară într-o singură fereastră pentru fiecare dosar: un editor cu servere de limbaj și predicții în timp ce tastați, terminale, un browser, Git, CI, un depanator și conversații IA cu Claude, Codex, Antigravity, OpenCode sau un agent integrat în aplicație. Fiecare agent lucrează în aceeași cronologie, întreabă înainte să modifice fișiere sau să ruleze comenzi și lasă un punct de control înaintea fiecărui prompt, astfel încât să puteți examina și derula înapoi ce a făcut, mesaj cu mesaj. Nimic din ce configurează un dosar nu rulează înainte să aveți încredere în el, iar textul din afară ajunge la un agent marcat ca nesigur. Dictați în editor și în câmpul de mesaj; recunoașterea vocală poate rămâne pe Mac-ul dvs. Dango este scris în Rust pe controale AppKit reale, fără Electron, vorbește 35 de limbi și nu are telemetrie.

**Încă nepublicat.** Prima versiune va fi o imagine de disc semnată cu un Developer ID și notarizată de Apple, pentru macOS 14 sau o versiune ulterioară pe Apple Silicon și Intel. Până atunci, [compilați Dango din codul sursă](#compilare-din-codul-sursă).

**Ați găsit o eroare?** [Deschideți un issue](https://github.com/sventmueller/dango/issues/new) și spuneți cu ce agent și în ce tip de proiect a apărut.

[![Rust, ediția 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 sau ulterior](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Universal: Apple Silicon și Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit prin objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 de limbi ale interfeței](https://img.shields.io/badge/languages-35-4C9A2A)
![Fără telemetrie](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Semnat cu Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Licență: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licență)

[Bahasa Indonesia](../id/README.md) ·
[Bahasa Melayu](../ms/README.md) ·
[Català](../ca/README.md) ·
[Čeština](../cs/README.md) ·
[Dansk](../da/README.md) ·
[Deutsch](../de/README.md) ·
[English](../../../README.md) ·
[Español](../es/README.md) ·
[Français](../fr/README.md) ·
[Hrvatski](../hr/README.md) ·
[Italiano](../it/README.md) ·
[Magyar](../hu/README.md) ·
[Nederlands](../nl/README.md) ·
[Norsk](../no/README.md) ·
[Polski](../pl/README.md) ·
[Português (Brasil)](../pt_BR/README.md) ·
[Português (Portugal)](../pt_PT/README.md) ·
**Română** ·
[Slovenčina](../sk/README.md) ·
[Slovenščina](../sl/README.md) ·
[Suomi](../fi/README.md) ·
[Svenska](../sv/README.md) ·
[Tiếng Việt](../vi/README.md) ·
[Türkçe](../tr/README.md) ·
[Ελληνικά](../el/README.md) ·
[Русский](../ru/README.md) ·
[Українська](../uk/README.md) ·
[עברית](../he/README.md)‎ ·
[العربية](../ar/README.md)‎ ·
[हिन्दी](../hi/README.md) ·
[ไทย](../th/README.md) ·
[한국어](../ko/README.md) ·
[日本語](../ja/README.md) ·
[简体中文](../zh-Hans/README.md) ·
[繁體中文](../zh-Hant/README.md)

## Cuprins

- [Funcții pe scurt](#funcții-pe-scurt)
- [Cerințe](#cerințe)
- [Instalare](#instalare)
- [Primii pași](#primii-pași)
- [Agenți IA](#agenți-ia)
  - [Cronologia](#cronologia)
  - [Permisiuni și moduri](#permisiuni-și-moduri)
  - [Puncte de control și worktree-uri](#puncte-de-control-și-worktree-uri)
- [Editor](#editor)
- [Servere de limbaj](#servere-de-limbaj)
- [Depanator](#depanator)
- [Terminal](#terminal)
- [Browser](#browser)
- [Git](#git)
- [CI](#ci)
- [Markdown](#markdown)
- [Dictare](#dictare)
- [Încredere](#încredere)
- [Sandbox](#sandbox)
- [Aspect și accesibilitate](#aspect-și-accesibilitate)
- [Limbi](#limbi)
- [Scurtături de tastatură](#scurtături-de-tastatură)
- [Confidențialitate și securitate](#confidențialitate-și-securitate)
- [Unde se află datele](#unde-se-află-datele)
- [Limitări cunoscute](#limitări-cunoscute)
- [Compilare din codul sursă](#compilare-din-codul-sursă)
- [Licență](#licență)

## Funcții pe scurt

- **Toți agenții într-o singură cronologie.** Claude prin propria comandă `claude`, Codex, Antigravity și OpenCode prin Agent Client Protocol, alții din registrul ACP și un agent integrat în aplicație pentru Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama și LM Studio. Răspunsurile, apelurile de instrumente, cererile de permisiune, planurile, listele De făcut, subagenții și sarcinile din fundal arată la fel, indiferent ce agent rulează.
- **Examinare și derulare înapoi.** Înaintea fiecărui prompt, Dango păstrează un punct de control. Ce a modificat un agent poate fi examinat și derulat înapoi pentru fiecare mesaj, iar o conversație poate lucra într-un worktree Git propriu și își poate preda munca prin comasare sau cherry-pick.
- **O fereastră pentru fiecare dosar.** Fișierele, terminalele, paginile web, conversațiile IA, diferențele și previzualizările sunt file macOS ale acelei ferestre. Bara laterală conține Fișiere, Căutare, Agenți, Git, Depanare și CI.
- **Un editor de cod adevărat.** Culori de sintaxă, mai multe puncte de inserare, pliere, `.editorconfig`, servere de limbaj pentru 17 limbaje cu completare, salt la definiție, referințe, redenumire, corecturi rapide și ierarhii, plus predicții în timp ce tastați de la GitHub Copilot sau de la un model local.
- **Depanator, Git și CI integrate.** Puncte de întrerupere și execuție pas cu pas pentru Rust, C, C++, Swift, Go, JavaScript, TypeScript și Python; pregătirea pentru commit a unor fragmente separate, ramuri, stash-uri, conflicte și un graf al commiturilor; rulări, joburi și jurnale din GitHub Actions și GitLab CI.
- **Atent cu dosarele dvs.** Un dosar în care nu aveți încredere nu pornește nimic: niciun proces Git, server de limbaj, hook sau agent. Comenzile Git distructive nu sunt niciodată permise definitiv, iar commiturile sunt verificate pentru secrete.
- **Dictare.** Vorbiți în editor și în câmpul de mesaj, online prin OpenAI sau Groq ori local cu Whisper și Parakeet.
- **Nativ până în măduva oaselor.** Ferestre, file, meniuri și bare de instrumente AppKit, TextKit 2 pentru editor, Metal pentru terminal, WebKit doar în fila browserului. Modul întunecat, Măriți contrastul și VoiceOver funcționează ca în orice aplicație Mac.
- **În limba dvs.** Interfața este disponibilă în 35 de limbi: arabă, catalană, chineză (simplificată), chineză (tradițională), croată, cehă, daneză, olandeză, engleză, finlandeză, franceză, germană, greacă, ebraică, hindi, maghiară, indoneziană, italiană, japoneză, coreeană, malaeză, norvegiană, poloneză, portugheză (Brazilia), portugheză (Portugalia), română, rusă, slovacă, slovenă, spaniolă, suedeză, thailandeză, turcă, ucraineană și vietnameză.

## Cerințe

- macOS 14 Sonoma sau o versiune ulterioară, pe Apple Silicon sau Intel. Dango este dezvoltat și testat pe macOS 27.
- Pentru conversațiile IA, agentul pe care doriți să îl folosiți; consultați [Agenți IA](#agenți-ia).
- Pentru serverele de limbaj și adaptoarele de depanare, pe care Dango le instalează la cerere: Go pentru gopls și `dlv`; Node.js pentru serverele TypeScript, Python, JSON, YAML, CSS, HTML și shell și pentru depanatorul JavaScript; Xcode Command Line Tools pentru clangd, sourcekit-lsp și `lldb-dap`. rust-analyzer este descărcat din versiunile sale de pe GitHub.
- Pentru predicțiile în timp ce tastați, un abonament GitHub Copilot (cel gratuit este suficient) sau un model fill-in-the-middle în Ollama sau LM Studio.

## Instalare

**Din imaginea de disc.** Fiecare versiune este o imagine de disc, `Dango-<version>.dmg`, cu o aplicație universală pentru Apple Silicon și Intel, semnată cu un Developer ID și notarizată de Apple. Deschideți-o și trageți Dango în dosarul Aplicații. Un Cask Homebrew instalează aceeași imagine. Nicio versiune nu a fost publicată încă.

**Actualizări.** Dango instalat din imaginea de disc se menține la zi cu [Sparkle](https://sparkle-project.org). La a doua pornire întreabă dacă poate căuta singur actualizări; până nu sunteți de acord, nu face nicio solicitare. **Dango › Căutați actualizări…** caută imediat. Actualizările sunt semnate cu cheia proprie a aplicației, pe care Sparkle o verifică înainte de a instala ceva. O versiune compilată de dvs. nu are actualizare automată.

**Din codul sursă.** Consultați [Compilare din codul sursă](#compilare-din-codul-sursă), apoi mutați `target/release/Dango.app` în `/Applications`.

**Comanda de terminal.** **Dango › Instalați comanda „dango”…** scrie un mic script în `~/.local/bin/dango`, fără drepturi de administrator; `~/.local/bin` trebuie să fie în `PATH`. Apoi `dango .` deschide dosarul curent și `dango file.rs` un fișier. Dango nu înlocuiește niciodată acolo un fișier pe care nu l-a scris el însuși.

## Primii pași

1. **Deschideți un dosar** cu ⌘O sau `dango <folder>`. Prima dată, Dango vă întreabă dacă aveți încredere în el (consultați [Încredere](#încredere)).
2. **Bara laterală** comută între Fișiere (⌘1), Căutare (⌘2), Agenți (⌘3), Git (⌘4), Depanare (⌘5) și CI (⌘6); un clic dreapta le ascunde pe cele pe care nu le folosiți, iar ⌃⌘S pliază bara laterală.
3. **Fiecare document este o filă** a ferestrei dosarului, filele proprii ale macOS. „+” de la capătul barei de file și ⌘T oferă un fișier nou, un terminal, browserul și fiecare agent instalat, și într-un worktree Git propriu; dacă țineți apăsat „+”, apare același lucru sub formă de meniu.
4. **Deschidere rapidă** cu ⌘P găsește fișiere, iar cu `#` simbolurile proiectului; ⇧⌘F caută și înlocuiește în proiect.
5. **Vorbiți cu un agent** într-o filă IA. ⌘Esc mută focalizarea tastaturii în câmpul său de mesaj și înapoi.
6. **Configurări** cu ⌘, conține filele General, CI, Agenți, Agent propriu, Server de limbaj, Dictare și Predicție.

Bara de instrumente conține înapoi și înainte prin locurile pe care le-ați editat (⌃- și ⌃⇧-), ramura, Deschidere rapidă și prezentarea Activitate, care listează tot ce rulează. **Deschideți alături** din meniul contextual al unui fișier afișează un al doilea document lângă primul. După o repornire sau o blocare, fiecare fereastră revine cu filele sale, iar editoarele cu text nesalvat revin cu acel text. Imaginile, sunetul, videoclipurile, fonturile și documentele Office se deschid într-o filă prin Quick Look, fișierele PDF prin PDFKit.

## Agenți IA

| Agent | Cum rulează | De ce are nevoie |
|---|---|---|
| Claude | comanda `claude`, cu care Dango comunică direct prin protocolul ei stream-json | `claude` instalat și autentificat; Dango caută în `PATH`-ul shellului de autentificare, în `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` și `/usr/local/bin`. Autentificarea are loc doar în `claude` însuși |
| Codex | `codex-acp` prin Agent Client Protocol (ACP) | Node.js și npm; Dango instalează pachetul cu versiune fixată la cerere |
| Antigravity | `agy acp` | propriul `agy` în `PATH` sau descărcarea cu versiune fixată, instalată la cerere |
| OpenCode | `opencode acp` | propriul `opencode` în `PATH` sau descărcarea cu versiune fixată, instalată la cerere |
| Alți agenți ACP | din registrul ACP, preluat doar la cerere | ce indică intrarea din registru; intrările fără sumă de control se instalează doar după ce confirmați |
| Agent propriu | integrat în aplicație, cu instrumente pentru fișiere, căutare, comenzi, diagnosticări și cereri web | o cheie API pentru Anthropic, OpenAI, Google Gemini sau OpenRouter, păstrată în portchei; Ollama sau LM Studio pe acest Mac fără cheie; orice server care vorbește API-ul OpenAI sau Anthropic |

**Configurări › Agenți** arată ce agenți are acest Mac, îi instalează pe ceilalți și actualizările lor și deschide registrul ACP. Agenții instalați de Dango sunt fixați la versiuni testate și verificați după sumele de control publicate (npm integrity sau SHA-256); Dango nu rulează pachete `npx` fără versiune.

### Cronologia

Fiecare agent lucrează într-o filă IA cu aceeași cronologie: răspunsuri, apeluri de instrumente cu rezultatul lor, cereri de permisiune, întrebări, planuri de aprobat și comentat (⌥⌘K), liste De făcut, subagenți și sarcini din fundal. Două butoane din antetul filei listează subagenții și procesele din fundal care rulează. Menționați un fișier, `@terminal` sau `@browser` pentru a-i transmite agentului conținutul. Un punct verde pe filă indică o sesiune care lucrează, unul roșu o sesiune care vă așteaptă, iar o notificare vă anunță când se termină sau așteaptă în timp ce vă uitați în altă parte. Pentru Claude, linia de stare arată contextul și consumul.

### Permisiuni și moduri

Conversațiile noi încep în modul „Manual”: agentul întreabă înainte să modifice fișiere sau să ruleze comenzi. Pentru comenzile Git distructive (`push --force`, `reset --hard`, `clean -fd`), Dango nu oferă „Permiteți întotdeauna”. Agentul propriu își păstrează regulile „Permiteți întotdeauna” pentru fiecare proiect, le listează în **Configurări › Agent propriu** și întreabă de fiecare dată la comenzile Git distructive, indiferent ce este salvat; Claude și agenții ACP păstrează astfel de reguli în propria configurație. Cu „Permiteți tot” și fără sandbox, conversația afișează un avertisment permanent.

### Puncte de control și worktree-uri

Înaintea fiecărui prompt, Dango păstrează un punct de control, ca commit sub `refs/ide-checkpoints/` în depozitul proiectului, sau într-un dosar propriu pentru un proiect fără Git; un push sau fetch obișnuit nu le ia niciodată cu el. Ce a modificat un agent poate fi examinat și derulat înapoi pentru fiecare mesaj. O conversație poate lucra într-un worktree Git propriu: panoul Git arată ce a modificat și preia munca prin comasare sau cherry-pick.

## Editor

- **Culori de sintaxă, numere de linie și marcaje de modificare** față de ultimul commit, căutare și înlocuire, Deschidere rapidă (⌘P) și căutare în proiect (⇧⌘F).
- **Indentarea** urmează fișierul și `.editorconfig`: taburi sau spații, indentare automată, perechi de paranteze.
- **Mai multe puncte de inserare:** ⌥-clic, ⌥-tragere pentru o coloană, ⌃⇧↑ și ⌃⇧↓, ⌘D pentru următoarea apariție.
- **Pliere** (⌥⌘← și ⌥⌘→, cu ⇧ pentru tot), activarea sau dezactivarea comentariului (⌘/), salt la linie (⌃G), spații vizibile și încadrarea liniilor (⌥Z).
- **Fișierele își păstrează codificarea**, marcajul de ordine a octeților și terminațiile de linie. Un fișier modificat pe disc este reîncărcat dacă editorul nu are text nesalvat; altfel, o bară oferă diferențele și o comasare.
- **Predicții în timp ce tastați.** În timp ce tastați apare o sugestie cu gri, iar Tab o acceptă. Provine de la GitHub Copilot sau de la un model local din Ollama sau LM Studio, după alegerea din **Configurări › Predicție**; un model local vă păstrează codul pe acest Mac.

## Servere de limbaj

Go, Rust, TypeScript și JavaScript, Python, C, C++ și Objective-C, Swift, scripturi shell, JSON, YAML, CSS, HTML, TOML, Markdown și Lua, plus servere proprii din **Configurări › Server de limbaj**. Dango instalează un server la cerere, în propriul dosar de instrumente, și folosește `tsc`-ul unui proiect TypeScript atunci când poate servi drept server de limbaj.

Ce oferă: diagnosticări (⌘' și ⇧⌘' trec de la una la alta), informații la trecerea cursorului (⌃⌘I), completare (⌥Esc), ajutor pentru semnături, salt la definiție (⌃⌘J), la definiția tipului și la implementare, referințe (⌃⇧⌘F), ierarhii de apeluri și de tipuri (⌃⇧⌘H), redenumire (⌃⌘E), formatare (⌥⇧F), culori semantice, pliere și schița (⇧⌘O); tipuri și nume de parametri în interiorul liniei; corecturi rapide și refactorizări în spatele unui bec (⌥⌘A); simbolurile proiectului cu ⌥⌘O.

## Depanator

Puncte de întrerupere, și cu condiții sau ca puncte de jurnal, execuție pas cu pas, fire de execuție cu stivele lor de apeluri, variabile și expresii urmărite în inspector (⌥⌘I), valori lângă cod cât timp programul este în pauză și o consolă de depanare care evaluează în cadrul aflat în pauză.

| Limbaj | Adaptor de depanare |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` din Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | depanatorul JavaScript, pe Node.js |
| Python | debugpy |

Fișierul `.vscode/launch.json` al unui proiect este citit. Meniul Depanare folosește scurtăturile din Xcode: ⌘\\ setează un punct de întrerupere, ⌘Y le activează sau dezactivează pe toate, ⌃⌘Y continuă, F6, F7 și F8 trec peste, intră și ies, ⌘. oprește.

## Terminal

Terminale în file (⌃\`) cu mediul shellului de autentificare, desenate cu Metal. ⌘K golește ecranul. O imagine lipită sau trasă ajunge la un agent de programare care rulează în terminal ori lipește calea unei copii salvate. ⌘-clic deschide linkuri și nume de fișiere. Un server de dezvoltare pornit din terminal poate fi deschis în fila browserului.

## Browser

O filă de browser cu câmp de adresă (⌘L), înapoi și înainte (⌘[ și ⌘]), reîncărcare (⌘R), căutare, descărcări care întreabă mai întâi și inspectorul web. `@browser` transmite pagina unui agent, marcată ca text din afară.

## Git

Panoul Git pregătește pentru commit și renunță la fișiere sau fragmente separate, face commituri, face commit și push sau sincronizează, comută și creează ramuri, păstrează stash-uri, rezolvă conflictele de comasare și desenează un graf al commiturilor recente. Blame arată cine a modificat ultima dată linia de sub cursor sau o coloană pentru întregul fișier. Un buton scrie mesajul de commit cu IA; diferențele pleacă doar după ce gitleaks nu a găsit niciun secret în ele, iar fiecare commit este verificat și el pentru secrete.

## CI

GitHub Actions și GitLab CI, inclusiv GitHub Enterprise Server și GitLab autogestionat:

- rulări, joburi și jurnale ale ramurii curente;
- rularea din nou a joburilor eșuate, a unui singur job sau, pe GitHub, a unei rulări întregi; pornirea joburilor manuale sau aprobarea implementărilor în așteptare; pornirea unui pipeline nou sau a unui workflow care poate fi pornit manual; anularea;
- pull requesturile sau merge requesturile deschise și issue-urile, care se deschid în fila browserului;
- jurnalul unui job eșuat transmis unui agent, marcat ca text din afară.

Tokenul provine din portchei, din `gh` sau `glab`, din mediul shellului de autentificare sau este unul pe care îl introduceți în **Configurări › CI**.

## Markdown

- **Previzualizare.** Un fișier Markdown se deschide cu previzualizarea alături; **Vizualizare › Afișați/ascundeți previzualizarea alături** (⇧⌘V) o închide și o deschide, iar **Configurări › General** dezactivează previzualizarea automată. Previzualizarea desenează formule, diagrame Mermaid, Graphviz și PlantUML și imaginile dosarului; imaginile de pe web se încarcă doar într-un dosar de încredere sau când le cereți în previzualizare.
- **Meniul Format:** aldin (⌘B), cursiv (⌘I), tăiat (⇧⌘X), cod (⌥⇧⌘C), link (⇧⌘K), titluri 1–3 (⌥⌘1–⌥⌘3), liste cu marcatori și numerotate, sarcini, citate și blocuri de cod. Fiecare comandă comută, pe selecție sau pe linii, și este un singur pas de anulare.
- **Lipire.** Textul din Word sau dintr-o pagină web sosește ca Markdown, iar imaginile ca fișiere lângă document; ⌥⇧⌘V lipește text simplu.
- **Export.** **Fișier › Exportați** scrie un PDF, un singur fișier HTML cu imaginile incluse sau un document Word. Imaginile de pe web sunt incluse doar dintr-un dosar de încredere.
- **Deschideți în Splat.** Dacă [Splat](https://github.com/sventmueller/splat) este instalat, **Fișier › Deschideți în Splat** și meniurile contextuale ale listei de fișiere și ale filelor deschid documentul acolo, după ce l-au salvat.

## Dictare

Dictați în câmpul de mesaj al unei file IA sau în editor: cu butonul de microfon de acolo, **Editare › Porniți dictarea** (⌃⌥D) sau tasta fn (țineți-o apăsată pentru a înregistra, apăsați-o de două ori pentru a continua înregistrarea). Recunoașterea vocală este cea din [Lowspeak](https://github.com/sventmueller/lowspeak); Lowspeak în sine nu trebuie să fie instalat.

- **Configurări › Dictare** setează modul și modelul (online cu OpenAI sau Groq, local cu Whisper sau Parakeet), limba, post-procesarea, microfonul și propriul vocabular.
- **Modelele locale** sunt descărcate de pe Hugging Face când alegeți unul, cu progres și posibilitatea de a anula. Ajung în dosarele pe care le folosește Lowspeak, astfel încât fiecare aplicație folosește ce a descărcat cealaltă. Un model local păstrează înregistrarea pe acest Mac.
- **Transcrierea online** necesită o cheie pentru OpenAI sau Groq, pe care Dango o păstrează într-un element propriu din portchei.
- **Din Lowspeak.** Dacă Lowspeak este configurat altfel decât implicit, fila Dictare oferă o singură dată preluarea configurărilor lui și, la cerere, a cheilor; macOS întreabă înainte ca Dango să citească una (alegeți „Permiteți”, nu „Permiteți întotdeauna”).
- **Permisiuni.** macOS cere acces la microfon la prima dictare. Tasta fn funcționează cât timp Dango este aplicația activă și Lowspeak nu rulează; necesită permisiunea de accesibilitate, pe care Dango o cere doar după prima dictare, niciodată la pornire.

## Încredere

Un dosar poate face să ruleze programe imediat ce ceva este pornit în el: hookuri Git și configurări ale depozitului precum `core.fsmonitor`, servere de limbaj din `node_modules`, hookuri ale agenților, `.mcp.json`. Prima dată când deschideți un dosar, Dango vă întreabă dacă aveți încredere în el.

- **De încredere:** Git, serverele de limbaj, depanatoarele, agenții și CI rulează pentru acest dosar și tot ce conține.
- **Restricționat:** puteți citi și edita; marcajele de modificare din editor provin din citirea directă a `.git`. Nimic din ce configurează dosarul nu este pornit. Panourile Git, CI și IA spun acest lucru și oferă să aveți încredere în dosar.

Un text care nu provine de la dvs. poate conține instrucțiuni menite să conducă un agent (injectare de prompt). Ieșirea unui terminal (`@terminal`), o pagină web (`@browser`), un jurnal CI transmis unui agent, un fișier din afara proiectului și un text mai lung lipit dintr-o pagină web sunt, prin urmare, trimise marcate: eticheta spune „Conținut extern”, iar agentul primește textul împreună cu avertismentul că sunt date nesigure ale căror instrucțiuni nu trebuie să le urmeze. Paginile web ale agentului propriu vin cu același avertisment. Un text lipit este considerat provenit dintr-o pagină web atunci când browserul indică acest lucru în clipboard, așa cum fac Safari, Chrome și Firefox; textul din alte surse și o frază scurtă de câteva cuvinte rămân ale dvs.

## Sandbox

Dango însuși rulează fără App Sandbox, deoarece fiecare program pe care îl pornește l-ar moșteni: terminalele, Git, serverele de limbaj și agenții nu ar putea funcționa. În schimb, fiecare conversație IA are un comutator de sandbox, dezactivat implicit. Ce limitează depinde de agent, iar sugestia comutatorului o spune:

| Agent | Cu sandboxul activat |
|---|---|
| Claude | comenzile shell rulează în sandboxul propriu al lui Claude; instrumentele sale pentru fișiere și web, nu |
| Codex | comenzile și modificările scriu doar în proiect, fără rețea |
| Agent propriu | fișierele, cererile web și comenzile rămân în proiect și în dosarele permise, fără rețea (`sandbox-exec`) |
| Antigravity, OpenCode | niciun sandbox pe care Dango să-l poată activa |

## Aspect și accesibilitate

- **Modul întunecat** urmează sistemul; **Măriți contrastul** și **Reduceți mișcarea** sunt respectate.
- **VoiceOver** denumește bara laterală, filele, cardurile din cronologia IA, rândurile din Git și CI, depanatorul și textul terminalului. Punctele de întrerupere nu se deosebesc niciodată doar prin culoare.
- **Tastatură.** Fiecare comandă are o scurtătură și toate pot fi modificate în `keybindings.json`; ⌥⌘\` trece la panoul următor.
- **De la dreapta la stânga.** În arabă și ebraică, fereastra este oglindită, în timp ce vizualizările de cod rămân de la stânga la dreapta.

Ce adaugă Dango peste AppKit, zonă cu zonă, se află în [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Limbi

Interfața este tradusă în 35 de limbi: arabă, catalană, chineză (simplificată), chineză (tradițională), croată, cehă, daneză, olandeză, engleză, finlandeză, franceză, germană, greacă, ebraică, hindi, maghiară, indoneziană, italiană, japoneză, coreeană, malaeză, norvegiană, poloneză, portugheză (Brazilia), portugheză (Portugalia), română, rusă, slovacă, slovenă, spaniolă, suedeză, thailandeză, turcă, ucraineană și vietnameză.

Dango urmează ordinea limbilor din **Configurări sistem › General › Limbă și regiune**; tot acolo puteți alege o limbă doar pentru Dango, la **Aplicații**. Fără o traducere în limba dvs., folosește engleza. Termenii urmează un glosar, [docs/glossary.md](../../../docs/glossary.md), și traducerile Apple, astfel încât Dango să se citească precum restul Mac-ului dvs.

## Scurtături de tastatură

| Acțiune | Scurtătură |
|---|---|
| Deschidere dosar, fereastră nouă, închidere fereastră | ⌘O, ⇧⌘N, ⇧⌘W |
| Fișier nou, salvare, salvare ca | ⌘N, ⌘S, ⇧⌘S |
| Filă nouă, închidere filă, fila următoare și anterioară | ⌘T, ⌘W, ⇧⌘] și ⇧⌘[ |
| Deschidere rapidă, căutare în proiect | ⌘P, ⇧⌘F |
| Fișiere, Căutare, Agenți, Git, Depanare, CI | ⌘1 până la ⌘6 |
| Afișarea sau ascunderea barei laterale, panoul următor | ⌃⌘S, ⌥⌘\` |
| Înapoi și înainte prin locurile editate | ⌃- și ⌃⇧- |
| Activare/dezactivare comentariu, salt la linie | ⌘/, ⌃G |
| Următoarea apariție, punct de inserare deasupra și dedesubt | ⌘D, ⌃⇧↑ și ⌃⇧↓ |
| Pliere, depliere, pliere totală, depliere totală | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Încadrarea liniilor, inspector | ⌥Z, ⌥⌘I |
| Completare, informații, salt la definiție | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Referințe, ierarhia apelurilor, redenumire | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Simboluri din fișier, din proiect | ⇧⌘O, ⌥⌘O |
| Corectură rapidă, formatare | ⌥⌘A, ⌥⇧F |
| Problema următoare și anterioară | ⌘' și ⇧⌘' |
| Previzualizare Markdown, lipire ca text simplu | ⇧⌘V, ⌥⇧⌘V |
| Aldin, cursiv, tăiat, cod, link | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Titlu 1 până la 3 | ⌥⌘1 până la ⌥⌘3 |
| Terminal nou, golirea terminalului | ⌃\`, ⌘K |
| Focalizare pe câmpul de mesaj al agentului, comentarea unui plan | ⌘Esc, ⌥⌘K |
| Câmp de adresă, reîncărcare, înapoi, înainte (browser) | ⌘L, ⌘R, ⌘[, ⌘] |
| Pornirea sau oprirea dictării | ⌃⌥D |
| Punct de întrerupere, activarea sau dezactivarea tuturor | ⌘\\, ⌘Y |
| Continuare, pas peste, în interior, în afară, oprire | ⌃⌘Y, F6, F7, F8, ⌘. |
| Configurări | ⌘, |

`keybindings.json` din dosarul de date al aplicației (consultați [Unde se află datele](#unde-se-află-datele)) modifică oricare dintre ele.

## Confidențialitate și securitate

- **Fără telemetrie,** fără analize, fără rapoarte de blocare trimise undeva. Erorile ajung în jurnalul sistemului (Consolă, sub numele aplicației).
- **Rețeaua doar pentru ce porniți dvs.:** agenții pe care îi folosiți, fiecare cu propriul serviciu; registrul ACP și descărcările de agenți, servere de limbaj, adaptoare de depanare și modele de dictare când le cereți; gazdele CI la care vă autentificați; predicțiile Copilot, dacă l-ați ales; furnizorii agentului propriu; transcrierea online, dacă ați ales-o; paginile pe care le deschideți în fila browserului; și căutarea de actualizări după ce ați permis-o.
- **Codul părăsește Mac-ul** doar acolo unde ați ales un serviciu pentru asta: un agent, predicțiile Copilot, un mesaj de commit scris cu IA. Diferențele pentru un mesaj de commit pleacă doar după ce gitleaks nu a găsit niciun secret în ele.
- **Cheile și tokenurile** rămân în portcheiul macOS, niciodată într-un fișier de configurări. Cheile altor aplicații, cum ar fi cele ale Lowspeak, sunt citite doar dacă sunteți de acord și doar o singură dată.
- **Descărcări fixate.** Agenții, serverele de limbaj și adaptoarele de depanare pe care le instalează Dango sunt fixați la versiuni testate și verificați după sumele lor de control.
- **Semnarea codului** cu Hardened Runtime; actualizările se instalează doar cu semnătura proprie a aplicației.

## Unde se află datele

`<id>` este identificatorul pachetului, `local.agentide` pentru versiunea publicată (`local.agentide.debug` pentru compilările de depanare). Nu conține numele afișat, astfel încât o redenumire păstrează toate datele.

| Loc | Ce |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (lista panoului IA), `agent-threads/` (conversațiile agentului propriu), `layouts/` (file pentru fiecare proiect), `recovery/` (editoare nesalvate după o blocare), `checkpoints/` (pentru dosare fără Git), `worktrees/`, `agents/` și `tools/` (agenți, servere de limbaj și adaptoare de depanare instalate), `task-output/` |
| `~/Library/Caches/<id>/` | imagini lipite, scripturi de integrare cu shellul, registrul ACP; poate fi șters |
| `~/Library/Logs/<id>/dictation/` | fișierele jurnal ale dictării |
| `~/Library/Application Support/Lowspeak` și `FluidAudio` | modele locale de dictare, partajate cu Lowspeak |
| `~/Library/WebKit/<id>/` | cookie-uri și date ale site-urilor din fila browserului |
| `<project>/.dango/settings.json` | configurările proprii ale proiectului |
| depozitul Git al proiectului | puncte de control, ca commituri sub `refs/ide-checkpoints/`; un push sau fetch obișnuit nu le ia niciodată cu el |
| portchei, serviciul `<id>` | cheile API ale agentului propriu și ale dictării, tokenuri CI |
| `~/.local/bin/dango` | comanda de terminal, dacă ați instalat-o |

Claude își păstrează propriile sesiuni în `~/.claude/projects`; Dango le citește pentru a relua o sesiune, dar nu le modifică.

## Limitări cunoscute

- **Fără App Sandbox** pentru aplicația însăși, deoarece terminalele, Git, serverele de limbaj și agenții l-ar moșteni; un dosar trebuie să fie de încredere înainte să ruleze ceva din ce configurează. Antigravity și OpenCode nu au un sandbox pe care Dango să-l poată activa.
- **Cifrele de consum și defalcarea contextului lui Claude** se bazează pe părți ale protocolului său marcate ca experimentale și sunt ascunse când acestea lipsesc.
- **Derularea înapoi a discuției** funcționează doar pentru Claude; la ceilalți agenți se derulează înapoi doar codul.
- **Autentificarea la CI prin browser** este dezactivată; folosiți `gh`, `glab`, un token sau mediul.
- **Gemini CLI nu este oferit:** Google a încheiat autentificarea cu un cont personal și trimite la Antigravity. Conversațiile Gemini anterioare rămân lizibile.

## Compilare din codul sursă

Aveți nevoie de:

- macOS 14 sau o versiune ulterioară cu Xcode Command Line Tools (`xcode-select --install`); Xcode în sine nu este necesar.
- Rust așa cum îl indică `rust-toolchain.toml`. `rust` din Homebrew compilează pentru procesorul acestui Mac; compilarea universală a unei versiuni necesită rustup cu țintele `aarch64-apple-darwin` și `x86_64-apple-darwin`.
- Copii ale [splat](https://github.com/sventmueller/splat) și [lowspeak](https://github.com/sventmueller/lowspeak) lângă aceasta (`../splat`, `../lowspeak`), ambele pe ramura `main`; workspace-ul preia crate-uri din ele prin cale.
- cmake, pentru fiecare compilare cu dictare (`make run`, `make build`, `make release`), care compilează whisper.cpp. `make check` compilează fără.

| Comandă | Ce face |
|---|---|
| `make run` | compilează aplicația de depanare (`target/debug/Dango Debug.app`) și o deschide |
| `make build` | compilează aplicația de versiune (`target/release/Dango.app`), semnată cu un certificat local pe care prima compilare îl creează în portcheiul de autentificare; mărește numărul compilării în `VERSION` |
| `make lint` | formatare și clippy; avertismentele îl fac să eșueze |
| `make check` | `lint`, apoi toate testele; nu are nevoie de portchei, fereastră sau rețea |
| `make test` | `check`, apoi autotestul `probe` și auditul de accesibilitate, de la stânga la dreapta și de la dreapta la stânga, în aplicația reală |
| `make perf` | bugetele de timp, într-o compilare de versiune; relevant doar pe un Mac altfel inactiv |
| `make release` | o repetiție a publicării în `target/release-dry-run/`, semnată cu certificatul local; nu trimite nimic nicăieri |
| `make release-notarized` | `check`, apoi publicarea propriu-zisă în `dist/`: semnătură Developer ID, notarizare, fluxul de actualizări semnat cu cheia proprie a aplicației; mărește numărul compilării |

Depanarea și versiunea sunt aplicații separate, cu configurări și date proprii. Compilarea, testarea, autotestele, publicarea și regulile pe care le urmează codul sunt descrise în [docs/development.md](../../../docs/development.md), iar deciziile din spatele lor în [docs/plan.md](../../../docs/plan.md). Notele de versiune sunt [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), traduse lângă fiecare README în `assets/readme/<language>/`.

## Licență

Dango este licențiat sub oricare dintre

- Licența Apache, versiunea 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Licența MIT ([LICENSE-MIT](../../../LICENSE-MIT))

la alegerea dvs.

Dacă nu declarați explicit altfel, orice contribuție trimisă în mod intenționat pentru includere în Dango de către dvs., așa cum este definită în licența Apache-2.0, va fi licențiată dublu ca mai sus, fără termeni sau condiții suplimentare.
