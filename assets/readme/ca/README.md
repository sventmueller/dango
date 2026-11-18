<p><img src="../../readme-icon.png" alt="Icona del Dango" width="147" height="147"></p>

# Dango

**L’editor natiu de macOS per treballar amb agents de programació.**

El Dango reuneix el codi, els agents que el canvien i tot el que els envolta en una finestra per carpeta: un editor amb servidors de llenguatge i prediccions mentre escrius, terminals, un navegador, Git, CI, un depurador i fils d’IA amb Claude, Codex, Antigravity, OpenCode o un agent integrat a l’app. Cada agent treballa a la mateixa línia de temps, pregunta abans de canviar fitxers o executar ordres i deixa un punt de control abans de cada prompt, perquè puguis revisar i retrocedir el que ha fet, missatge a missatge. Res del que configura una carpeta s’executa abans que hi confiïs, i el text de fora arriba a un agent marcat com a no fiable. Dicta a l’editor i al camp del missatge; el reconeixement de veu es pot quedar al teu Mac. El Dango està escrit en Rust sobre controls reals d’AppKit, sense Electron, parla 35 idiomes i no té telemetria.

[![Descarrega Dango.dmg](https://img.shields.io/badge/Descarrega_Dango-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Descarrega Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: signat i notaritzat per Apple, per a macOS 14 o posterior en Apple Silicon i Intel. L’enllaç estarà disponible amb la primera publicació.

**Has trobat un error?** [Obre un issue](https://github.com/sventmueller/dango/issues/new) i indica amb quin agent i en quin tipus de projecte s’ha produït.

[![Rust, edició 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 o posterior](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Universal: Apple Silicon i Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit mitjançant objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 idiomes d’interfície](https://img.shields.io/badge/languages-35-4C9A2A)
![Sense telemetria](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Signat amb Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Llicència: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#llicència)

[Bahasa Indonesia](../id/README.md) ·
[Bahasa Melayu](../ms/README.md) ·
**Català** ·
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
[Română](../ro/README.md) ·
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

## Contingut

- [Funcions en resum](#funcions-en-resum)
- [Requisits](#requisits)
- [Instal·lació](#installació)
- [Primers passos](#primers-passos)
- [Agents IA](#agents-ia)
  - [La línia de temps](#la-línia-de-temps)
  - [Permisos i modes](#permisos-i-modes)
  - [Punts de control i worktrees](#punts-de-control-i-worktrees)
- [Editor](#editor)
- [Servidors de llenguatge](#servidors-de-llenguatge)
- [Proves](#proves)
- [Depurador](#depurador)
- [Terminal](#terminal)
- [Navegador](#navegador)
- [Git](#git)
- [CI](#ci)
- [Contenidors](#contenidors)
- [Markdown](#markdown)
- [Dictat](#dictat)
- [Confiança](#confiança)
- [Sandbox](#sandbox)
- [Aparença i accessibilitat](#aparença-i-accessibilitat)
- [Idiomes](#idiomes)
- [Dreceres de teclat](#dreceres-de-teclat)
- [Privadesa i seguretat](#privadesa-i-seguretat)
- [On són les dades](#on-són-les-dades)
- [Limitacions conegudes](#limitacions-conegudes)
- [Compilar des del codi font](#compilar-des-del-codi-font)
- [Llicència](#llicència)

## Funcions en resum

- **Tots els agents en una línia de temps.** Claude amb la seva pròpia ordre `claude`, Codex, Antigravity i OpenCode amb l’Agent Client Protocol, més des del registre ACP, i un agent integrat a l’app per a Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama i LM Studio. Respostes, crides a eines, sol·licituds de permís, plans, tasques, subagents i tasques en segon pla tenen el mateix aspecte, sigui quin sigui l’agent.
- **Revisar i retrocedir.** Abans de cada prompt, el Dango desa un punt de control. El que ha canviat un agent es pot revisar i retrocedir per missatge, i un fil pot treballar en el seu propi worktree de Git i lliurar la feina fusionant o amb cherry-pick.
- **Una finestra per carpeta.** Fitxers, terminals, pàgines web, fils d’IA, diffs i previsualitzacions són pestanyes de macOS d’aquesta finestra. La barra lateral conté Fitxers, Cerca, Agents, Git, Proves, Depura, CI i Contenidors.
- **Un editor de codi de debò.** Colors de sintaxi, diversos punts d’inserció, plegat, `.editorconfig`, servidors de llenguatge per a 17 llenguatges amb compleció, anar a la definició, referències, canvi de nom, correccions ràpides i jerarquies, i prediccions mentre escrius de GitHub Copilot o d’un model local.
- **Depurador, Git i CI integrats.** Punts d’interrupció i execució pas a pas per a Rust, C, C++, Swift, Go, JavaScript, TypeScript i Python; preparar fragments solts, branques, stashes, conflictes i un graf de commits; execucions, jobs i logs de GitHub Actions i GitLab CI.
- **Curós amb les teves carpetes.** Una carpeta en què no confies no inicia res: ni processos de Git, ni servidors de llenguatge, ni hooks, ni agents. Les ordres destructives de Git mai no es permeten per sempre, i els commits es revisen per buscar-hi secrets.
- **Dictat.** Parla a l’editor i al camp del missatge, en línia amb OpenAI o Groq o en local amb Whisper i Parakeet.
- **Natiu fins al moll de l’os.** Finestres, pestanyes, menús i barres d’eines d’AppKit, TextKit 2 per a l’editor, Metal per al terminal, WebKit només a la pestanya del navegador. El mode fosc, Augmenta el contrast i el VoiceOver funcionen com en qualsevol app del Mac.
- **En el teu idioma.** La interfície està en 35 idiomes: àrab, català, xinès (simplificat), xinès (tradicional), croat, txec, danès, neerlandès, anglès, finès, francès, alemany, grec, hebreu, hindi, hongarès, indonesi, italià, japonès, coreà, malai, noruec, polonès, portuguès (Brasil), portuguès (Portugal), romanès, rus, eslovac, eslovè, castellà, suec, tailandès, turc, ucraïnès i vietnamita.

## Requisits

- macOS 14 Sonoma o posterior, en Apple Silicon o Intel. El Dango es desenvolupa i es prova a macOS 27.
- Per als fils d’IA, l’agent que vulguis fer servir; consulta [Agents IA](#agents-ia).
- Per als servidors de llenguatge i els adaptadors de depuració, que el Dango instal·la quan l’hi demanes: Go per a gopls i `dlv`; Node.js per als servidors de TypeScript, Python, JSON, YAML, CSS, HTML i shell i per al depurador de JavaScript; les Xcode Command Line Tools per a clangd, sourcekit-lsp i `lldb-dap`. rust-analyzer es baixa de les seves versions de GitHub.
- Per a les prediccions mentre escrius, un pla de GitHub Copilot (el gratuït ja va bé) o un model fill-in-the-middle a Ollama o LM Studio.

## Instal·lació

**Des de la imatge de disc.** Cada versió és una imatge de disc, `Dango-<version>.dmg`, amb una app universal per a Apple Silicon i Intel, signada amb un Developer ID i notaritzada per Apple. Obre-la i arrossega el Dango a la carpeta Aplicacions. Un Cask de Homebrew instal·la la mateixa imatge. Encara no s’ha publicat cap versió.

**Actualitzacions.** El Dango de la imatge de disc es manté al dia amb [Sparkle](https://sparkle-project.org). En el segon inici pregunta si pot cercar actualitzacions pel seu compte; fins que no hi estiguis d’acord, no fa cap sol·licitud. **Dango › Cerca actualitzacions…** cerca immediatament. Les actualitzacions estan signades amb la clau pròpia de l’app, que Sparkle comprova abans d’instal·lar res. Una versió compilada per tu no té actualitzador.

**Des del codi font.** Consulta [Compilar des del codi font](#compilar-des-del-codi-font) i després mou `target/release/Dango.app` a `/Applications`.

**L’ordre de terminal.** **Dango › Instal·la l’ordre “dango”…** escriu un petit script a `~/.local/bin/dango`, sense permisos d’administrador; `~/.local/bin` ha de ser al teu `PATH`. Després, `dango .` obre la carpeta actual i `dango file.rs` un fitxer. El Dango mai no substitueix allà un fitxer que no hagi escrit ell mateix.

## Primers passos

1. **Obre una carpeta** amb ⌘O o `dango <folder>`. La primera vegada, el Dango et pregunta si hi confies (consulta [Confiança](#confiança)).
2. **La barra lateral** canvia entre Fitxers (⌘1), Cerca (⌘2), Agents (⌘3), Git (⌘4), Proves (⌘5), Depura (⌘6), CI (⌘7) i Contenidors (⌘8); amb un clic dret amagues els que no fas servir, i ⌃⌘S plega la barra lateral.
3. **Cada document és una pestanya** de la finestra de la carpeta, les pestanyes pròpies de macOS. El “+” al final de la barra de pestanyes i ⌘T ofereixen un fitxer nou, un terminal, el navegador i cada agent instal·lat, també en el seu propi worktree de Git; si mantens premut el “+”, apareix el mateix com a menú.
4. **Obertura ràpida** amb ⌘P troba fitxers, i amb `#` els símbols del projecte; ⇧⌘F cerca i substitueix al projecte.
5. **Parla amb un agent** en una pestanya d’IA. ⌘Esc porta el focus del teclat al seu camp del missatge i de tornada.
6. **Configuració** amb ⌘, conté les pestanyes General, CI, Agents, Agent propi, Servidor de llenguatge, Dictat i Predicció.

La barra d’eines conté enrere i endavant pels llocs que has editat (⌃- i ⌃⇧-), la branca, Obertura ràpida i el resum d’Activitat, que llista tot el que s’executa. **Obre al costat** al menú contextual d’un fitxer mostra un segon document al costat del primer. Després d’un reinici o d’una fallada, cada finestra torna amb les seves pestanyes, i els editors amb text sense desar tornen amb aquest text. Imatges, àudio, vídeo, tipus de lletra i documents d’Office s’obren en una pestanya amb Quick Look, els PDF amb PDFKit.

## Agents IA

| Agent | Com funciona | Què necessita |
|---|---|---|
| Claude | l’ordre `claude`, a la qual es parla directament amb el seu protocol stream-json | `claude` instal·lat i amb la sessió iniciada; el Dango busca al `PATH` del shell d’inici de sessió, a `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` i `/usr/local/bin`. La sessió només s’inicia al mateix `claude` |
| Codex | `codex-acp` amb l’Agent Client Protocol (ACP) | Node.js i npm; el Dango instal·la el paquet de versió fixada quan l’hi demanes |
| Antigravity | `agy acp` | el teu propi `agy` al `PATH`, o la baixada de versió fixada, instal·lada quan ho demanes |
| OpenCode | `opencode acp` | el teu propi `opencode` al `PATH`, o la baixada de versió fixada, instal·lada quan ho demanes |
| Altres agents ACP | del registre ACP, que només es baixa quan ho demanes | el que indiqui l’entrada del registre; les entrades sense suma de verificació només s’instal·len després que ho confirmis |
| Agent propi | integrat a l’app, amb eines per a fitxers, cerca, ordres, diagnòstics i sol·licituds web | una clau d’API per a Anthropic, OpenAI, Google Gemini o OpenRouter, desada al clauer; Ollama o LM Studio en aquest Mac sense clau; qualsevol servidor que parli l’API d’OpenAI o d’Anthropic |

**Configuració › Agents** mostra quins agents té aquest Mac, instal·la els altres i les seves actualitzacions, i obre el registre ACP. Els agents que instal·la el Dango estan fixats a versions provades i es comproven amb les seves sumes de verificació publicades (npm integrity o SHA-256); el Dango no executa paquets `npx` sense versió.

### La línia de temps

Cada agent treballa en una pestanya d’IA amb la mateixa línia de temps: respostes, crides a eines amb la seva sortida, sol·licituds de permís, preguntes, plans per aprovar i comentar (⌥⌘K), tasques, subagents i tasques en segon pla. Dos botons a la capçalera de la pestanya llisten els subagents i processos en segon pla en curs. Esmenta un fitxer, `@terminal` o `@browser` per donar-ne el contingut a l’agent. Un punt verd a la pestanya indica una sessió que treballa, un de vermell una sessió que t’espera, i una notificació avisa quan acaba o espera mentre mires una altra cosa. Amb Claude, la línia d’estat mostra el context i l’ús.

### Permisos i modes

Els fils nous comencen en el mode “Manual”: l’agent pregunta abans de canviar fitxers o executar ordres. Per a les ordres destructives de Git (`push --force`, `reset --hard`, `clean -fd`), el Dango no ofereix “Permet sempre”. L’agent propi desa les seves regles de “Permet sempre” per projecte, les llista a **Configuració › Agent propi** i pregunta cada vegada per les ordres destructives de Git, tingui el que tingui desat; Claude i els agents ACP desen aquestes regles a la seva pròpia configuració. Amb “Permet‑ho tot” i sense sandbox, el fil mostra un avís permanent.

### Punts de control i worktrees

Abans de cada prompt, el Dango desa un punt de control, com a commit sota `refs/ide-checkpoints/` al repositori del projecte, o a la seva pròpia carpeta per a un projecte sense Git; un push o un fetch normal mai no se’ls emporta. El que ha canviat un agent es pot revisar i retrocedir per missatge. Un fil pot treballar en el seu propi worktree de Git: el tauler de Git mostra el que ha canviat i n’incorpora la feina fusionant o amb cherry-pick.

## Editor

- **Colors de sintaxi, números de línia i marques de canvis** respecte de l’últim commit, cerca i substitueix, Obertura ràpida (⌘P) i cerca al projecte (⇧⌘F).
- **El sagnat** segueix el fitxer i `.editorconfig`: tabulacions o espais, sagnat automàtic, parelles de parèntesis.
- **Diversos punts d’inserció:** ⌥-clic, ⌥-arrossegar per a una columna, ⌃⇧↑ i ⌃⇧↓, ⌘D per a la següent aparició.
- **Plegat** (⌥⌘← i ⌥⌘→, amb ⇧ per a tot), comentar o descomentar (⌘/), anar a la línia (⌃G), espais visibles i ajustar les línies (⌥Z).
- **Els fitxers conserven la codificació**, la marca d’ordre de bytes i els finals de línia. Un fitxer que canvia al disc es torna a carregar si l’editor no té text sense desar; si no, una barra n’ofereix les diferències i una fusió.
- **Prediccions mentre escrius.** Mentre escrius apareix un suggeriment en gris, i Tab l’accepta. Ve de GitHub Copilot o d’un model local a Ollama o LM Studio, segons el que hagis triat a **Configuració › Predicció**; un model local manté el teu codi en aquest Mac.
- **Minimapa i Desplaçament fix** (menú Editor): una imatge estreta de tot el fitxer al costat del text, i les línies de capçalera dels blocs on es troba la línia superior, fixades a sobre seu.
- **Diagnòstics al final de la línia** (menú Mostra): el missatge més greu de cada línia després d’un segon de repòs, amb un símbol per a la seva gravetat.
- **Previsualització en directe de pàgines estàtiques:** Editor › Obre a la pestanya del navegador serveix les pàgines d’una carpeta de confiança des del mateix Dango i torna a carregar la pestanya quan es desa un fitxer.
- **El Prettier** dona format a JavaScript, TypeScript, CSS, HTML, Markdown i més allà on el projecte el configura, amb el `node_modules/.bin/prettier` propi del projecte; l’opció `formatter` tria el formatador per a cada llenguatge.
- **Mode Vim** (Editor › Mode Vim): modes Normal, Inserir, Visual i Substituir, comptadors, operadors amb moviments i objectes de text, `.`, registres, marques i macros. `u` i ⌃R fan servir el desfer de l’editor, i una passada d’inserció és un sol pas. `"+` i `"*` són el porta-retalls; l’opció `vimClipboard` fa que el registre sense nom també sigui el porta-retalls. `/`, `?`, `n`, `N`, `*`, `:s` i `:%s` cerquen amb les expressions regulars de Rust, no amb els patrons del Vim: sense `\v`, i `\b` en lloc de `\<` i `\>`. Les ordres Ex són `:w`, `:q`, `:wq`, `:x`, `:noh` i `:<line>`. En mode Normal, ⌃G és del Vim en lloc d’anar a la línia.
- **Sincronització de la configuració** a través d’una carpeta de l’iCloud Drive (**Configuració › General**): la teva configuració i les teves dreceres et segueixen als teus altres Mac; els camins, les adreces locals i els consentiments es queden a cada un.

## Servidors de llenguatge

Go, Rust, TypeScript i JavaScript, Python, C, C++ i Objective-C, Swift, scripts de shell, JSON, YAML, CSS, HTML, TOML, Markdown i Lua, i servidors propis des de **Configuració › Servidor de llenguatge**. El Dango instal·la un servidor quan ho demanes, a la seva pròpia carpeta d’eines, i fa servir el `tsc` d’un projecte de TypeScript quan pot fer de servidor de llenguatge.

El que aporten: diagnòstics (⌘' i ⇧⌘' passen de l’un a l’altre), informació del símbol (⌃⌘I), compleció (⌥Esc), ajuda de signatures, anar a la definició (⌃⌘J), a la definició de tipus i a la implementació, referències (⌃⇧⌘F), jerarquies de crides i de tipus (⌃⇧⌘H), canvi de nom (⌃⌘E), format (⌥⇧F), colors semàntics, plegat i l’esquema (⇧⌘O); tipus i noms de paràmetres dins de la línia; correccions ràpides i refactoritzacions darrere d’una bombeta (⌥⌘A); els símbols del projecte amb ⌥⌘O.

A més del servidor propi d’un fitxer, l’ESLint revisa el JavaScript i el TypeScript dels projectes que el fan servir, amb les seves correccions ràpides, i les abreviatures d’Emmet apareixen a la compleció per a HTML, CSS i JSX. El Dango els instal·la tots dos quan ho demanes; **Configuració › General** corregeix els problemes d’ESLint en desar, si vols.

## Proves

El tauler de Proves llista les proves d’una carpeta per paquet i fitxer amb el seu estat: `cargo test` o cargo-nextest, Go, pytest, Vitest i Jest. Un símbol al costat de cada prova al marge l’executa, ⌥-clic la depura, i una asserció fallida en marca la línia. El menú Depura executa la prova al punt d’inserció (⌃⌥⌘U), les últimes proves una altra vegada (⌃⌥⌘G) o totes, i depura la prova al punt d’inserció. Les proves només s’executen en carpetes de confiança, perquè executen el codi del projecte; `cargo test` llista les seves proves quan ho demanes, ja que llistar-les compila tots els programes de prova.

## Depurador

Punts d’interrupció, també amb condicions i com a logpoints, execució pas a pas, fils d’execució amb les seves piles de crides, variables i expressions vigilades a l’Inspector (⌥⌘I), valors al costat del codi mentre el programa està en pausa, i una consola de depuració que avalua en el frame en pausa.

| Llenguatge | Adaptador de depuració |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` de les Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | el depurador de JavaScript, sobre Node.js |
| Python | debugpy |

Es llegeix el `.vscode/launch.json` d’un projecte. El menú Depura fa servir les dreceres de l’Xcode: ⌘\\ posa un punt d’interrupció, ⌘Y els activa o desactiva tots, ⌃⌘Y continua, F6, F7 i F8 passen per sobre, entren i surten, ⌘. atura.

## Terminal

Terminals en pestanyes (⌃\`) amb l’entorn del shell d’inici de sessió, dibuixats amb Metal. ⌘K esborra la pantalla. Una imatge enganxada o arrossegada va a un agent de programació que s’executi al terminal, o enganxa el camí d’una còpia desada. ⌘-clic obre enllaços i noms de fitxer. Un servidor de desenvolupament que inicia el terminal es pot obrir a la pestanya del navegador.

## Navegador

Una pestanya de navegador amb camp d’adreça (⌘L), enrere i endavant (⌘[ i ⌘]), torna a carregar (⌘R), cerca, baixades que pregunten primer i l’inspector web. `@browser` dona la pàgina a un agent, marcada com a text de fora.

## Git

El tauler de Git prepara i descarta fitxers o fragments solts, fa commits, fa commit i push o sincronitza, canvia i crea branques, desa stashes, resol conflictes de fusió i dibuixa un graf dels commits recents. Blame mostra qui va canviar per última vegada la línia sota el cursor, o una columna per a tot el fitxer. Un botó escriu el missatge de commit amb IA; el diff només surt quan gitleaks no hi ha trobat cap secret, i cada commit també es revisa per buscar-hi secrets.

## CI

GitHub Actions i GitLab CI, també GitHub Enterprise Server i GitLab autogestionat:

- execucions, jobs i logs de la branca actual;
- tornar a executar jobs fallits, un sol job o, a GitHub, una execució sencera; iniciar jobs manuals o aprovar desplegaments en espera; iniciar un pipeline nou o un workflow que es pot iniciar a mà; cancel·lar;
- les pull requests o merge requests obertes i els issues, que s’obren a la pestanya del navegador;
- el log d’un job fallit lliurat a un agent, marcat com a text de fora.

El token ve del clauer, de `gh` o `glab`, de l’entorn del shell d’inici de sessió, o és un que introdueixes a **Configuració › CI**.

## Contenidors

El tauler de Contenidors mostra els projectes Compose de Docker amb els seus serveis, els contenidors solts, les imatges i els volums, i segueix els esdeveniments de Docker a mesura que passen. Un fitxer Compose de la carpeta apareix com a projecte fins i tot abans que existeixi cap dels seus contenidors. Els contenidors s’inicien, s’aturen i es reinicien; els seus logs i un shell s’obren en pestanyes de terminal, i un port publicat s’obre a la pestanya del navegador. Eliminar un contenidor, una imatge o un volum, i netejar les dades no utilitzades, demanen confirmació abans. Les ordres up, down i pull de Compose només s’executen en carpetes de confiança, perquè Compose construeix i executa el que descriu el projecte. El menú del tauler tria el context de Docker de la finestra sense canviar el que fan servir els teus shells; les mateixes ordres són al menú Contenidors. Cal la línia d’ordres `docker`, Docker 24.0.6 o posterior.

## Markdown

- **Previsualització.** Un fitxer Markdown s’obre amb la previsualització al costat; **Mostra › Mostra/amaga la previsualització al costat** (⇧⌘V) la tanca i l’obre, i **Configuració › General** desactiva la previsualització automàtica. La previsualització dibuixa fórmules, diagrames de Mermaid, Graphviz i PlantUML i les imatges de la carpeta; les imatges del web només es carreguen en una carpeta de confiança o quan les demanes a la previsualització.
- **Menú Format:** negreta (⌘B), cursiva (⌘I), ratllat (⇧⌘X), codi (⌥⇧⌘C), enllaç (⇧⌘K), capçaleres 1–3 (⌥⌘1–⌥⌘3), llistes amb vinyetes i numerades, tasques, citacions i blocs de codi. Cada ordre activa o desactiva, sobre la selecció o les línies, i és un sol pas de desfer.
- **Enganxar.** El text de Word o d’una pàgina web arriba com a Markdown, i les seves imatges com a fitxers al costat del document; ⌥⇧⌘V enganxa text sense format.
- **Exportar.** **Arxiu › Exporta** escriu un PDF, un sol fitxer HTML amb les imatges a dins o un document de Word. Les imatges del web només s’hi inclouen des d’una carpeta de confiança.
- **Obre a Splat.** Si [Splat](https://github.com/sventmueller/splat) està instal·lat, **Arxiu › Obre a Splat** i els menús contextuals de la llista de fitxers i de les pestanyes hi obren el document, després de desar-lo.

## Dictat

Dicta al camp del missatge d’una pestanya d’IA o a l’editor: amb el botó del micròfon que hi ha, **Edita › Inicia el dictat** (⌃⌥D) o la tecla fn (mantén-la premuda per gravar, prem-la dues vegades per continuar gravant). El reconeixement de veu és el de [Lowspeak](https://github.com/sventmueller/lowspeak); no cal tenir el Lowspeak instal·lat.

- **Configuració › Dictat** estableix el mode i el model (en línia amb OpenAI o Groq, en local amb Whisper o Parakeet), l’idioma, el postprocessament, el micròfon i el teu propi vocabulari.
- **Els models locals** es baixen de Hugging Face quan en tries un, amb el progrés i l’opció de cancel·lar. Es desen a les carpetes que fa servir el Lowspeak, de manera que cada app fa servir el que l’altra ha baixat. Un model local manté la gravació en aquest Mac.
- **La transcripció en línia** necessita una clau d’OpenAI o Groq, que el Dango desa en un element propi del clauer.
- **Des del Lowspeak.** Si el Lowspeak està configurat de manera diferent de la predeterminada, la pestanya Dictat ofereix una vegada adoptar-ne la configuració, i les claus si ho demanes; el macOS pregunta abans que el Dango en llegeixi una (tria “Permet”, no “Permet sempre”).
- **Permisos.** El macOS demana accés al micròfon en el primer dictat. La tecla fn funciona mentre el Dango és l’app activa i el Lowspeak no s’està executant; necessita el permís d’accessibilitat, que el Dango només demana després del primer dictat, mai en iniciar-se.

## Confiança

Una carpeta pot fer que s’executin programes tan bon punt s’hi inicia alguna cosa: hooks de Git i configuració del repositori com `core.fsmonitor`, servidors de llenguatge de `node_modules`, hooks d’agents, `.mcp.json`. La primera vegada que obres una carpeta, el Dango et pregunta si hi confies.

- **De confiança:** Git, els servidors de llenguatge, els depuradors, els agents i la CI s’executen per a aquesta carpeta i tot el que conté.
- **Restringida:** pots llegir i editar; les marques de canvis de l’editor surten de llegir `.git` directament. No s’inicia res del que configura la carpeta. Els taulers de Git, CI i IA ho indiquen i ofereixen confiar en la carpeta.

Un text que no ve de tu pot dur instruccions pensades per dirigir un agent (injecció de prompts). La sortida d’un terminal (`@terminal`), una pàgina web (`@browser`), un log de CI lliurat a un agent, un fitxer fora del projecte i un text més llarg enganxat des d’una pàgina web s’envien per això marcats: l’etiqueta diu “Contingut extern”, i l’agent rep el text amb l’avís que són dades no fiables de les quals no ha de seguir les instruccions. Les pàgines web de l’agent propi arriben amb el mateix avís. Un text enganxat compta com a provinent d’una pàgina web quan el navegador ho indica al porta-retalls, com fan Safari, Chrome i Firefox; el text d’altres fonts, i una frase curta de poques paraules, continua sent teu.

## Sandbox

El mateix Dango s’executa sense App Sandbox, perquè tots els programes que inicia l’heretarien: terminals, Git, servidors de llenguatge i agents no podrien funcionar. En comptes d’això, cada fil d’IA té un interruptor de sandbox, desactivat per defecte. El que limita depèn de l’agent, i l’ajuda de l’interruptor ho explica:

| Agent | Amb el sandbox activat |
|---|---|
| Claude | les ordres de shell s’executen al sandbox propi de Claude; les seves eines de fitxers i web, no |
| Codex | les ordres i els canvis només escriuen al projecte, sense xarxa |
| Agent propi | fitxers, sol·licituds web i ordres es queden dins del projecte i de les carpetes permeses, sense xarxa (`sandbox-exec`) |
| Antigravity, OpenCode | cap sandbox que el Dango pugui activar |

## Aparença i accessibilitat

- **El mode fosc** segueix el sistema; es respecten **Augmenta el contrast** i **Redueix el moviment**.
- **El VoiceOver** anomena la barra lateral, les pestanyes, les targetes de la línia de temps d’IA, les files de Git i CI, el depurador i el text del terminal. Els punts d’interrupció mai no es distingeixen només pel color.
- **Teclat.** Cada ordre té una drecera, i totes es poden canviar a `keybindings.json`; ⌥⌘\` passa al tauler següent.
- **De dreta a esquerra.** En àrab i hebreu, la finestra s’emmiralla, mentre que les vistes de codi continuen d’esquerra a dreta.

El que el Dango afegeix a AppKit, àrea per àrea, és a [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Idiomes

La interfície està traduïda a 35 idiomes: àrab, català, xinès (simplificat), xinès (tradicional), croat, txec, danès, neerlandès, anglès, finès, francès, alemany, grec, hebreu, hindi, hongarès, indonesi, italià, japonès, coreà, malai, noruec, polonès, portuguès (Brasil), portuguès (Portugal), romanès, rus, eslovac, eslovè, castellà, suec, tailandès, turc, ucraïnès i vietnamita.

El Dango segueix l’ordre d’idiomes de **Configuració del Sistema › General › Idioma i regió**; allà també pots triar un idioma només per al Dango a **Apps**. Si no hi ha traducció per al teu idioma, fa servir l’anglès. Els termes segueixen un glossari, [docs/glossary.md](../../../docs/glossary.md), i les traduccions d’Apple, perquè el Dango es llegeixi com la resta del teu Mac.

## Dreceres de teclat

| Acció | Drecera |
|---|---|
| Obrir carpeta, finestra nova, tancar finestra | ⌘O, ⇧⌘N, ⇧⌘W |
| Fitxer nou, desar, desar com a | ⌘N, ⌘S, ⇧⌘S |
| Pestanya nova, tancar pestanya, pestanya següent i anterior | ⌘T, ⌘W, ⇧⌘] i ⇧⌘[ |
| Obertura ràpida, cercar al projecte | ⌘P, ⇧⌘F |
| Fitxers, Cerca, Agents, Git, Proves, Depura, CI, Contenidors | ⌘1 a ⌘8 |
| Mostrar o amagar la barra lateral, tauler següent | ⌃⌘S, ⌥⌘\` |
| Enrere i endavant pels llocs editats | ⌃- i ⌃⇧- |
| Comentar o descomentar, anar a la línia | ⌘/, ⌃G |
| Següent aparició, punt d’inserció a dalt i a baix | ⌘D, ⌃⇧↑ i ⌃⇧↓ |
| Plegar, desplegar, plegar-ho tot, desplegar-ho tot | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Ajustar les línies, Inspector | ⌥Z, ⌥⌘I |
| Compleció, informació del símbol, anar a la definició | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Referències, jerarquia de crides, canviar el nom | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Símbols del fitxer, del projecte | ⇧⌘O, ⌥⌘O |
| Correcció ràpida, donar format | ⌥⌘A, ⌥⇧F |
| Problema següent i anterior | ⌘' i ⇧⌘' |
| Previsualització de Markdown, enganxar com a text sense format | ⇧⌘V, ⌥⇧⌘V |
| Negreta, cursiva, ratllat, codi, enllaç | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Capçalera 1 a 3 | ⌥⌘1 a ⌥⌘3 |
| Terminal nou, esborrar el terminal | ⌃\`, ⌘K |
| Focus al camp del missatge de l’agent, comentar un pla | ⌘Esc, ⌥⌘K |
| Camp d’adreça, tornar a carregar, enrere, endavant (navegador) | ⌘L, ⌘R, ⌘[, ⌘] |
| Iniciar o aturar el dictat | ⌃⌥D |
| Punt d’interrupció, activar o desactivar-los tots | ⌘\\, ⌘Y |
| Executar la prova al punt d’inserció, tornar a executar les últimes proves | ⌃⌥⌘U, ⌃⌥⌘G |
| Continuar, passar per sobre, entrar, sortir, aturar | ⌃⌘Y, F6, F7, F8, ⌘. |
| Configuració | ⌘, |

`keybindings.json` a la carpeta de dades de l’app (consulta [On són les dades](#on-són-les-dades)) en canvia qualsevol.

## Privadesa i seguretat

- **Sense telemetria,** sense analítiques, sense informes de fallades enviats enlloc. Els errors van al registre del sistema (Consola, amb el nom de l’app).
- **La xarxa només per al que inicies tu:** els agents que fas servir, cadascun amb el seu propi servei; el registre ACP i les baixades d’agents, servidors de llenguatge, adaptadors de depuració i models de dictat quan els demanes; els servidors de CI on inicies sessió; les prediccions de Copilot si l’has triat; els proveïdors de l’agent propi; la transcripció en línia si l’has triada; les pàgines que obres a la pestanya del navegador; i la cerca d’actualitzacions un cop l’has permesa.
- **El codi només surt del Mac** on has triat un servei per a això: un agent, les prediccions de Copilot, un missatge de commit escrit amb IA. El diff per a un missatge de commit només surt quan gitleaks no hi ha trobat cap secret.
- **Les claus i els tokens** es queden al clauer del macOS, mai en un fitxer de configuració. Les claus d’altres apps, com les del Lowspeak, només es llegeixen si hi estàs d’acord, i només una vegada.
- **Baixades fixades.** Els agents, servidors de llenguatge i adaptadors de depuració que instal·la el Dango estan fixats a versions provades i es comproven amb les seves sumes de verificació.
- **Signatura de codi** amb el Hardened Runtime; les actualitzacions només s’instal·len amb la signatura pròpia de l’app.

## On són les dades

`<id>` és l’identificador del paquet, `local.agentide` per a la versió publicada (`local.agentide.debug` per a les compilacions de depuració). No conté el nom visible, de manera que un canvi de nom conserva totes les dades.

| Lloc | Què |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (la llista del tauler d’IA), `agent-threads/` (converses de l’agent propi), `layouts/` (pestanyes per projecte), `recovery/` (editors sense desar després d’una fallada), `checkpoints/` (per a carpetes sense Git), `worktrees/`, `agents/` i `tools/` (agents, servidors de llenguatge i adaptadors de depuració instal·lats), `task-output/` |
| `~/Library/Caches/<id>/` | imatges enganxades, scripts d’integració amb el shell, el registre ACP; es pot eliminar |
| `~/Library/Logs/<id>/dictation/` | els fitxers de registre del dictat |
| `~/Library/Application Support/Lowspeak` i `FluidAudio` | models de dictat locals, compartits amb el Lowspeak |
| `~/Library/WebKit/<id>/` | galetes i dades de llocs web de la pestanya del navegador |
| `<project>/.dango/settings.json` | la configuració pròpia del projecte |
| el repositori de Git del projecte | punts de control, com a commits sota `refs/ide-checkpoints/`; un push o un fetch normal mai no se’ls emporta |
| clauer, servei `<id>` | claus d’API de l’agent propi i del dictat, tokens de CI |
| `~/.local/bin/dango` | l’ordre de terminal, si l’has instal·lada |

Claude desa les seves pròpies sessions a `~/.claude/projects`; el Dango les llegeix per reprendre una sessió, però no les canvia.

## Limitacions conegudes

- **Sense App Sandbox** per a l’app mateixa, ja que terminals, Git, servidors de llenguatge i agents l’heretarien; una carpeta ha de ser de confiança abans que s’executi res del que configura. Antigravity i OpenCode no tenen cap sandbox que el Dango pugui activar.
- **Les xifres d’ús i el desglossament del context de Claude** depenen de parts del seu protocol marcades com a experimentals, i s’amaguen quan falten.
- **Retrocedir la conversa** només funciona amb Claude; amb els altres agents només retrocedeix el codi.
- **L’inici de sessió a la CI amb el navegador** està desactivat; fes servir `gh`, `glab`, un token o l’entorn.
- **Gemini CLI no s’ofereix:** Google ha deixat de permetre l’inici de sessió amb un compte personal i remet a Antigravity. Els fils de Gemini anteriors encara es poden llegir.

## Compilar des del codi font

Necessites:

- macOS 14 o posterior amb les Xcode Command Line Tools (`xcode-select --install`); l’Xcode en si no cal.
- Rust tal com l’indica `rust-toolchain.toml`. El `rust` de Homebrew compila per al processador d’aquest Mac; la compilació universal d’una versió necessita rustup amb els destins `aarch64-apple-darwin` i `x86_64-apple-darwin`.
- Còpies de [splat](https://github.com/sventmueller/splat) i [lowspeak](https://github.com/sventmueller/lowspeak) al costat d’aquesta (`../splat`, `../lowspeak`), totes dues a la seva branca `main`; l’espai de treball en pren crates per camí.
- cmake, per a cada compilació amb dictat (`make run`, `make build`, `make release`), que compila whisper.cpp. `make check` compila sense.

| Ordre | Què fa |
|---|---|
| `make run` | compila l’app de depuració (`target/debug/Dango Debug.app`) i l’obre |
| `make build` | compila l’app de versió (`target/release/Dango.app`), signada amb un certificat local que la primera compilació crea al clauer d’inici de sessió; deixa `VERSION` sense canvis |
| `make lint` | format i clippy; els avisos el fan fallar |
| `make check` | `lint` i després totes les proves; no necessita clauer, finestra ni xarxa |
| `make test` | `check` i després l’autoprova `probe` i l’auditoria d’accessibilitat, d’esquerra a dreta i de dreta a esquerra, a l’app real |
| `make perf` | els pressupostos de temps, en una compilació de versió; només té sentit en un Mac que altrament estigui inactiu |
| `make release` | un assaig de la publicació a `target/release-dry-run/`, signat amb el certificat local; no envia res enlloc |
| `make release-notarized` | `check` i després la publicació mateixa a `dist/`: signatura amb Developer ID, notarització, el canal d’actualitzacions signat amb la clau pròpia de l’app; deixa `VERSION` sense canvis |
| `make deploy` | en un `main` net, amb segell de CI o sense: notaritza la versió següent, commet `VERSION`, puja els fitxers de publicació, envia el codi i l’etiqueta de versió, amb el segell verd de CI si n’hi ha, i després publica la versió ell mateix |

Depuració i versió són apps separades amb la seva pròpia configuració i dades. La compilació, les proves, les autoproves, la publicació i les regles que segueix el codi es descriuen a [docs/development.md](../../../docs/development.md), i les decisions que hi ha al darrere, a [docs/plan.md](../../../docs/plan.md). Les notes de la versió són [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), traduïdes al costat de cada README a `assets/readme/<language>/`.

## Llicència

El Dango té llicència sota qualsevol de les dues següents:

- Llicència Apache, versió 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Llicència MIT ([LICENSE-MIT](../../../LICENSE-MIT))

a la teva elecció.

Tret que indiquis explícitament el contrari, qualsevol contribució enviada intencionadament per tu per ser inclosa al Dango, tal com es defineix a la llicència Apache-2.0, tindrà la doble llicència anterior, sense cap condició addicional.
