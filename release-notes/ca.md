<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Aquesta és la primera versió de Dango, un editor natiu per a macOS per treballar amb agents de programació. L’editor,
els terminals, un navegador, Git, CI, un depurador i els fils d’IA comparteixen una finestra per carpeta. L’app està
escrita en Rust sobre AppKit; WebKit només apareix a la pestanya del navegador. La interfície està disponible en 35
idiomes.

## Requisits
- macOS 14 o posterior, en Apple silicon o Intel.
- Per als fils d’IA, l’agent que triïs: Claude necessita l’ordre `claude` amb la sessió iniciada; Codex necessita
  Node.js, i l’app instal·la el seu paquet de versió fixada quan ho demanes; Antigravity i OpenCode fan servir el teu
  `agy` o `opencode`, o una baixada de versió fixada; l’agent propi necessita una clau d’API, o bé Ollama o LM Studio en
  aquest Mac.
- Els servidors de llenguatge i els adaptadors de depuració s’instal·len quan ho demanes. gopls i `dlv` necessiten Go;
  els servidors de TypeScript, Python, JSON, YAML, CSS, HTML i shell i el depurador de JavaScript necessiten Node.js;
  clangd, sourcekit-lsp i `lldb-dap` vénen amb les Xcode Command Line Tools.
- Les prediccions d’edició necessiten una subscripció de GitHub Copilot (n’hi ha prou amb la gratuïta), o un model
  fill-in-the-middle a Ollama o LM Studio.

## Finestra
- Cada fitxer, terminal, pàgina del navegador, fil d’IA, vista de diferències i previsualització és una pestanya de
  macOS a la finestra de la carpeta. El “+” al final de la barra de pestanyes i ⌘T ofereixen un fitxer nou, un terminal,
  el navegador i cada agent instal·lat, també en un worktree de Git propi.
- La barra lateral canvia entre Fitxers, Cerca, Agents, Git, CI i Depura (⌘1–⌘6); amb un clic secundari amagues els que
  no fas servir. La barra d’eines conté enrere i endavant pels llocs que has editat (⌃- i ⌃⇧-), la branca, Obertura
  ràpida i la visió general d’Activitat.
- “Obre al costat” mostra un segon document al costat del primer. Després d’un reinici, o d’un error inesperat, cada
  finestra torna amb les seves pestanyes, i els editors amb text sense desar tornen amb aquest text.
- Les imatges, l’àudio, el vídeo, els tipus de lletra i els documents d’Office s’obren en una pestanya amb Quick Look, i
  els PDF amb PDFKit.

## Editor
- Colors de sintaxi, números de línia, marques de canvi respecte a l’últim commit, cerca i substitució, Obertura ràpida
  (⌘P) i cerca al projecte (⇧⌘F).
- Sagnat automàtic, tabulacions o espais segons `.editorconfig`, parelles de claudàtors, activar i desactivar
  comentaris, plegat, anar a una línia, espais en blanc visibles i ajust de línies.
- Diversos cursors: ⌥-clic, ⌥-arrossegar per a una columna, ⌘D per a la següent aparició.
- Els fitxers conserven la codificació, la marca d’ordre de bytes i els finals de línia. Un fitxer que canvia al disc es
  torna a carregar si l’editor no té text sense desar; si no, una barra n’ofereix les diferències i una fusió.
- **Prediccions d’edició:** mentre escrius, apareix un suggeriment en gris; la tecla Tab l’accepta. Vénen de GitHub
  Copilot o d’un model local a Ollama o LM Studio, el que triïs a la configuració; un model local manté el teu codi en
  aquest Mac.

## Servidors de llenguatge
Go, Rust, TypeScript i JavaScript, Python, C, C++ i Objective-C, Swift, scripts de shell, JSON, YAML, CSS, HTML, TOML,
Markdown i Lua, i servidors propis des de la configuració. Diagnòstics, informació en passar el punter, compleció, ajuda
de signatura, anar a la definició, a la definició del tipus i a la implementació, referències, jerarquies de crides i de
tipus, canvi de nom, format, colors semàntics, plegat i l’esquema; tipus i noms de paràmetres dins de la línia;
correccions ràpides i refactoritzacions darrere d’una bombeta (⌥⌘A); els símbols del projecte amb ⌥⌘O o `#` a Obertura
ràpida. Si el `tsc` propi d’un projecte TypeScript pot fer de servidor de llenguatge, s’utilitza.

## Depurador
Punts d’interrupció, també amb condicions, execució pas a pas, fils d’execució amb les seves piles de crides, variables
i expressions vigilades a l’Inspector (⌥⌘I), valors al costat del codi mentre el programa està en pausa, i una consola
de depuració que avalua en el frame en pausa. Rust, C, C++ i Swift amb `lldb-dap`, Go amb `dlv`, JavaScript i TypeScript
amb el depurador de JavaScript, Python amb debugpy. Es llegeix el `.vscode/launch.json` del projecte. El menú Depura fa
servir les dreceres de Xcode.

## Terminal
Terminals en pestanyes amb l’entorn de l’intèrpret d’ordres d’inici de sessió, dibuixats amb Metal. Enganxar o deixar
anar una imatge la lliura a un agent de programació que s’executa al terminal, o bé enganxa el camí d’una còpia desada.
⌘-clic obre enllaços i noms de fitxer. Un servidor de desenvolupament que el terminal inicia es pot obrir a la pestanya
del navegador.

## Markdown
Un fitxer Markdown s’obre amb la previsualització al costat (⇧⌘V), amb fórmules, diagrames de Mermaid, Graphviz i
PlantUML i les imatges de la carpeta. Un menú Format per a negreta, cursiva, enllaços, encapçalaments, llistes, cites i
blocs de codi; el text enganxat des de Word o d’una pàgina web arriba com a Markdown. **Arxiu › Exporta** desa en PDF,
en un sol fitxer HTML o en un document de Word. Si Splat està instal·lat, **Arxiu › Obre a Splat** hi obre el document.

## Navegador
Una pestanya de navegador amb camp d’adreça, enrere i endavant, cerca, baixades que pregunten abans, i el Web Inspector.

## Git
Un tauler de Git per preparar i descartar fitxers o fragments solts, commits, fer commit i push o sincronitzar,
branques, stashes, conflictes de fusió i un gràfic dels commits recents. Blame mostra qui va canviar per última vegada
la línia del cursor, o una columna per a tot el fitxer. Un botó escriu el missatge del commit amb IA; el diff només surt
després que gitleaks no hi hagi trobat cap secret, i cada commit també es revisa per si conté secrets. Un fil d’IA pot
treballar en un worktree de Git propi; el tauler mostra què ha canviat i n’incorpora la feina amb una fusió o un
cherry-pick.

## CI
GitHub Actions i GitLab CI, també GitHub Enterprise Server i GitLab autogestionat: execucions, jobs i logs de la branca
actual; tornar a executar els jobs fallits, un sol job o (a GitHub) una execució sencera, iniciar jobs manuals o aprovar
desplegaments en espera, iniciar una pipeline nova o un workflow que es pot iniciar a mà, i cancel·lar. Els merge
requests o pull requests oberts i les issues apareixen al mateix tauler i s’obren a la pestanya del navegador. El log
d’un job fallit es pot enviar a un agent, marcat com a text extern. El token surt del clauer, de `gh` o `glab`, de
l’entorn de l’intèrpret d’ordres d’inici de sessió, o l’introdueixes tu.

## IA
Cada agent treballa en una pestanya d’IA amb el mateix historial: respostes, crides a eines amb la seva sortida,
sol·licituds de permís, preguntes, plans per aprovar i comentar, tasques pendents, subagents i tasques en segon pla; dos
botons a la capçalera de la pestanya llisten els subagents i els processos en segon pla que s’estan executant. Abans de
cada prompt, l’app desa un punt de control; els canvis es poden revisar i retrocedir per missatge. Els fils nous
comencen en el mode “Manual”, i les ordres de Git destructives no es permeten mai de manera permanent. La visió general
d’Activitat llista tot el que s’executa. Un punt verd a la pestanya indica una sessió que treballa, un de vermell una
sessió que t’espera, i una notificació t’avisa quan acaba o espera mentre mires una altra cosa.
- **Claude** amb l’ordre `claude`, sense canvis, amb sessions, subagents, tasques en segon pla, context i ús.
- **Agents ACP:** Codex, Antigravity i OpenCode, i més del registre d’ACP quan ho demanes; els agents instal·lats estan
  fixats a versions provades i es comproven amb les seves sumes de verificació.
- **Agent propi:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio i servidors que parlen l’API d’OpenAI
  o d’Anthropic, amb eines per a fitxers, cerca, ordres, diagnòstics i sol·licituds web; les claus es queden al clauer.
- Un interruptor de sandbox per fil limita el que poden fer les ordres d’un agent, en la mesura que l’agent ho admet.
- El contingut extern, com la sortida del terminal, una pàgina web o un log de CI, arriba a l’agent marcat com a no
  fiable.

## Dictat
Dictat al camp de missatge d’IA i a l’editor amb el reconeixement de veu de Lowspeak: el botó del micròfon, ⌃⌥D o la
tecla fn mentre Dango és l’app activa. Fa servir la configuració i els models de Lowspeak; la transcripció en línia
necessita una clau d’OpenAI o de Groq.

## Actualitzacions
L’app s’actualitza amb Sparkle. En el segon inici pregunta si pot cercar actualitzacions pel seu compte; fins que no hi
estiguis d’acord, no fa cap sol·licitud. **Dango › Cerca actualitzacions…** les cerca a l’instant.

## Accessibilitat
Noms de VoiceOver per a la barra lateral, les pestanyes, les targetes de l’historial d’IA, les files de Git i CI, el
depurador i el text del terminal; control complet amb el teclat amb dreceres que pots canviar a `keybindings.json`;
Augmentar el contrast i Reduir el moviment; les vistes de codi es mantenen d’esquerra a dreta en interfícies de dreta a
esquerra. Els detalls són a ACCESSIBILITY.md.

## Privadesa
Sense telemetria, sense analítiques i sense informes d’errors. L’app només es connecta per al que tu inicies: els agents
que fas servir, les baixades que demanes, els hosts de CI on inicies la sessió, les prediccions d’edició de Copilot si
les has triat, la transcripció en línia si l’has triada, les pàgines que obres i la cerca d’actualitzacions un cop
l’hagis permesa. Les claus i els tokens es queden al clauer de macOS.

## Limitacions conegudes
- L’inici de sessió a CI des del navegador està desactivat; fes servir `gh`, `glab`, un token o l’entorn.
- L’app funciona sense l’App Sandbox, ja que els terminals, Git, els servidors de llenguatge i els agents l’heretarien;
  cal confiar en una carpeta abans que s’executi res del que configura. Antigravity i OpenCode no tenen cap sandbox que
  l’app pugui activar.
- Les xifres d’ús de Claude i el desglossament del context depenen de parts del seu protocol marcades com a
  experimentals i s’amaguen quan falten. Retrocedir la conversa només funciona amb Claude; amb els altres agents només
  es retrocedeix el codi.
- No s’ofereix Gemini CLI: Google ha deixat de permetre l’inici de sessió amb un compte personal i remet a Antigravity.
  Els fils de Gemini anteriors continuen sent llegibles.
