Det här är den första versionen av Dango, en inbyggd macOS-redigerare för arbete med kodagenter. Redigerare, terminaler,
en webbläsare, Git, CI, en felsökare, AI-trådar och en Software Factory för arbetsordrar delar ett fönster per mapp.
Appen är skriven i Rust på AppKit; WebKit används bara i webbläsarfliken. Gränssnittet finns på 35 språk.

## Systemkrav
- macOS 14 eller senare, på Apple Silicon eller Intel.
- För AI-trådarna den agent du väljer: Claude kräver kommandot `claude`, inloggat; Codex kräver Node.js, och appen
  installerar dess fastlåsta paket när du ber om det; Antigravity och OpenCode använder ditt eget `agy` eller
  `opencode`, eller en fastlåst nedladdning; den egna agenten kräver en API-nyckel, eller Ollama eller LM Studio på den
  här Macen.
- Språkservrar och felsökningsadaptrar installeras när du ber om det. gopls och `dlv` kräver Go; servrarna för
  TypeScript, Python, JSON, YAML, CSS, HTML, ESLint och skal samt JavaScript-felsökaren kräver Node.js; clangd,
  sourcekit-lsp och `lldb-dap` följer med Xcode Command Line Tools.
- Förutsägelser medan du skriver kräver ett GitHub Copilot-abonnemang (det kostnadsfria räcker) eller en
  fill-in-the-middle-modell i Ollama eller LM Studio.

## Fönster
- Varje fil, terminal, webbsida, AI-tråd, diff och förhandsvisning är en macOS-flik i mappens fönster. ”+” i slutet av
  flikfältet och ⌘T erbjuder en ny fil, en terminal, webbläsaren och varje installerad agent, även i en egen
  Git-worktree.
- Sidofältet växlar mellan Filer, Sök, Agenter, Git, Felsök, CI, Tester, Containrar och Software Factory; högerklicka
  för att gömma dem du inte använder. I verktygsfältet finns bakåt och framåt genom ställena du har redigerat (⌃- och
  ⌃⇧-), grenen, Öppna snabbt och översikten Aktivitet.
- Ett fönster delas i rutor sida vid sida eller ovanför varandra, nästlade så djupt du vill, som i Xcode; varje ruta
  håller sina dokument bakom varandra med ett eget flikfält. ”Öppna bredvid” visar ett andra dokument bredvid det
  första.
- Efter en omstart eller en krasch kommer varje fönster tillbaka med sina flikar och rutor, och redigerare med osparad
  text får tillbaka texten.
- Bilder, ljud, video, typsnitt och Office-dokument öppnas i en flik via Quick Look, PDF:er via PDFKit.

## Redigerare
- Syntaxfärger, radnummer, ändringsmarkeringar mot den senaste commiten, sök och ersätt, Öppna snabbt (⌘P) och
  projektsökning (⇧⌘F). Färger och Markdown-stilar hänger med även i filer på många tusen rader utan att hålla
  tillbaka skrivandet.
- Automatiskt indrag, tabbar eller blanksteg enligt `.editorconfig`, parentespar, kommentera och avkommentera, vikning,
  gå till rad, synliga blanksteg, radbrytning, en minikarta och fästa rubriker för de omgivande kodblocken.
- Flera insättningspunkter: ⌥-klicka, ⌥-dra för en kolumn, ⌘D för nästa förekomst.
- **Vim-läge:** slås på i menyn Redigerare, med normal-, infognings- och visuellt läge, rörelser och operatorer,
  register, märken, makron, sökning och Ex-kommandon; dess meddelanden visas på gränssnittets språk.
- Emmet-förkortningar i kompletteringen för HTML, CSS och JSX, och ESLints problem åtgärdas när du sparar, om du vill.
- Filer behåller sin teckenkodning, byteordningsmarkering och sina radslut. En fil som har ändrats på disken läses in
  igen när redigeraren inte har någon osparad text; annars erbjuder ett fält skillnaderna och en sammanslagning.
- **Förutsägelser medan du skriver:** medan du skriver visas ett förslag i grått; Tab accepterar det. Förslagen kommer
  från GitHub Copilot eller från en lokal modell i Ollama eller LM Studio, det du väljer i inställningarna; en lokal
  modell håller din kod kvar på den här Macen.

## Språkservrar
Go, Rust, TypeScript och JavaScript, Python, C, C++ och Objective-C, Swift, skalskript, JSON, YAML, CSS, HTML, TOML,
Markdown och Lua, ESLint bredvid TypeScript-servern, samt egna servrar från inställningarna. Diagnostik, även vid
radslut, hovringsinfo, komplettering, signaturhjälp, gå till definition, typdefinition och implementering, referenser,
anrops- och typhierarkier, byt namn, formatering, semantiska färger, vikning och dispositionen; typer och parameternamn
inne i raden; snabbkorrigeringar och refaktoreringar bakom en glödlampa (⌥⌘A); projektets symboler med ⌥⌘O eller `#` i
Öppna snabbt. Ett TypeScript-projekts eget `tsc` används när det kan fungera som språkserver.

## Felsökare
Brytpunkter, även med villkor, stegning, trådar med sina anropsstackar, variabler och bevakade uttryck i inspektören
(⌥⌘I), värden bredvid koden medan programmet är pausat, och en felsökningskonsol som utvärderar i den pausade ramen.
Rust, C, C++ och Swift via `lldb-dap`, Go via `dlv`, JavaScript och TypeScript via JavaScript-felsökaren, Python via
debugpy. Ett projekts `.vscode/launch.json` läses in. Menyn Felsök använder Xcodes kortkommandon.

## Tester och containrar
- Panelen Tester listar testerna i Rust-, Go-, Python-, Jest-, Vitest- och Bun-projekt före den första körningen, kör
  ett test, en fil eller alla, visar varje resultat direkt och markerar det i redigerarens marginal.
- Panelen Containrar listar Dockers containrar, images, volymer och Compose-projekt; den startar, stoppar och tar bort
  dem, följer deras loggar och öppnar ett skal i en terminalflik.

## Terminal
Terminaler i flikar med inloggningsskalets miljö, ritade med Metal. En bild som du klistrar in eller släpper lämnas till
en kodagent som körs i terminalen, annars klistras sökvägen till en sparad kopia in. ⌘-klicka öppnar länkar och filnamn.
En utvecklingsserver som terminalen startar kan öppnas i webbläsarfliken.

## Markdown
En Markdown-fil öppnas med sin förhandsvisning bredvid (⇧⌘V), med formler, Mermaid-, Graphviz- och PlantUML-diagram och
mappens bilder. En Format-meny för fetstil, kursiv, länkar, rubriker, listor, citat och kodblock; text som klistras in
från Word eller en webbsida kommer fram som Markdown. **Arkiv › Exportera** skriver PDF, en enda HTML-fil eller ett
Word-dokument. Där Splat är installerat öppnar **Arkiv › Öppna i Splat** dokumentet där.

## Webbläsare
En webbläsarflik med adressfält, bakåt och framåt, sökning, hämtningar som frågar först och webbinspektören.

## Git
En Git-panel för att stagea och kasta filer eller enskilda ändringsblock, för commits, commit och push eller synk,
grenar, stashar och sammanslagningskonflikter, och med en graf över de senaste commitarna. Blame visar vem som senast
ändrade raden under markören, eller en kolumn för hela filen. En knapp skriver commitmeddelandet med AI; diffen skickas
först när gitleaks inte har hittat någon hemlighet i den, och varje commit kontrolleras också efter hemligheter. En
AI-tråd kan arbeta i en egen Git-worktree; panelen visar vad den har ändrat och tar över arbetet genom sammanslagning
eller cherry-pick.

## CI
GitHub Actions och GitLab CI, även GitHub Enterprise Server och egendriftat GitLab: körningar, jobb och loggar för den
aktuella grenen; kör om misslyckade jobb, ett enskilt jobb eller (på GitHub) en hel körning, starta manuella jobb eller
godkänn väntande driftsättningar, starta en ny pipeline eller ett arbetsflöde som får startas för hand, och avbryt.
Öppna merge requests eller pull requests och issues listas i samma panel och öppnas i webbläsarfliken. Loggen från ett
misslyckat jobb kan gå till en agent, märkt som text utifrån. Token hämtas från nyckelringen, `gh` eller `glab`,
inloggningsskalets miljö eller en som du anger.

## AI
Varje agent arbetar i en AI-flik med samma tidslinje: svar, verktygsanrop med sina utdata, behörighetsförfrågningar,
frågor, planer att godkänna och kommentera, att göra-listor, underagenter och bakgrundsuppgifter; två knappar i flikens
sidhuvud listar de underagenter och bakgrundsprocesser som körs. Före varje prompt sparar appen en kontrollpunkt;
ändringar kan granskas och spolas tillbaka per meddelande. Nya trådar startar i läget ”Manuell”, och destruktiva
Git-kommandon tillåts aldrig permanent. Översikten Aktivitet listar allt som körs. En grön prick på fliken visar en
session som arbetar, en röd en session som väntar på dig, och en notis talar om när den är klar eller väntar medan du
tittar på något annat. Tidslinjen, meddelandefältet och statusraden har en egen teckenstorlek i inställningarna.
- **Claude** via kommandot `claude`, oförändrat, med sessioner, underagenter, bakgrundsuppgifter, kontext, användning
  och Remote Control från telefonen.
- **Codex** via sin appserver och **OpenCode** via sin server, var och en i sitt eget protokoll, med sina trådar,
  tillbakaspolning i själva agenten och Remote Control för Codex.
- **ACP-agenter:** Antigravity, och fler från ACP-registret när du ber om det; installerade agenter är låsta till
  testade versioner och kontrolleras mot sina kontrollsummor.
- **Egen agent:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio och servrar som talar OpenAI:s eller
  Anthropics API, med verktyg för filer, sökning, kommandon, diagnostik och webbförfrågningar; nycklar stannar i
  nyckelringen.
- En agents egna kommandon öppnas som överlägg i fliken, likadant för alla agenter: status, export, MCP-servrar,
  behörigheter, hooks, minne, konfiguration, skills, insticksprogram, modell, konto och feedback.
- En sandlådeknapp per tråd begränsar vad en agents kommandon får göra, så långt agenten stöder det.
- Text utifrån, som terminalutdata, en webbsida eller en CI-logg, går till en agent märkt som ej betrodd.

## Software Factory
Arbetsordrar i stället för enskilda prompter: ett mål med acceptanskriterier, skrivet för hand eller hämtat från en
issue. Varje arbetsorder planeras, byggs och kontrolleras i en egen Git-worktree, av agenter från olika tillverkare i
fasta roller: en planerare skriver specifikationen, en byggare gör arbetet och en granskare från en annan tillverkare
kontrollerar varje kriterium. Projektets grindkommandon, som bygge och tester, körs i en sandlåda, och byggaren åtgärdar
det de hittar.
- Du bestämmer vid två tillfällen: du godkänner specifikationen, och du accepterar resultatet med dess bevis, helt eller
  avsnitt för avsnitt. Inget tas över utan din acceptans.
- Panelen och fliken Software Factory visar varje arbetsorder med dess station, kostnad och vad den väntar på; en
  arbetsorders flik visar dess tidslinje, bevis och nästa steg och öppnar dess pågående session som en AI-flik.
- Budgetar per arbetsorder och per dag, gränser för omgångar, paus vid avslutning, vila och låg batterinivå, och en
  logg som skrivs före varje åtgärd, så att arbetsordrar fortsätter efter en omstart.
- Issues med en vald etikett kan komma in som arbetsordrar; en accepterad arbetsorder kan gå ut som en merge request
  med en kommentar på sin issue, när du klickar. Arbetsordrar med separata sökvägsomfång körs sida vid sida och tas
  över en i taget.
- Nyckeltal per agent och tillverkare, routning efter förstagångsandelen, regelförslag för `AGENTS.md` och `CLAUDE.md`
  utifrån återkommande fynd, och återkommande arbetsordrar från projektets workflowfil.
- En mapps workflowfil `.dango/factory.toml` innehåller dess grindar, riskklasser och sökvägsomfång; ändrade
  grindkommandon och allt som lättar på gränserna väntar på din bekräftelse.

## Diktering
Diktering i AI-meddelandefältet och i redigeraren via Lowspeaks taligenkänning: mikrofonknappen, ⌃⌥D eller fn-tangenten
medan Dango är den aktiva appen. Den använder Lowspeaks inställningar och modeller; transkribering online kräver en
nyckel för OpenAI eller Groq.

## Uppdateringar
Appen uppdaterar sig själv via Sparkle. Vid den andra starten frågar den om den får leta efter uppdateringar på egen
hand; tills du godkänner gör den inga förfrågningar. **Dango › Leta efter uppdateringar…** letar direkt.

## Hjälpmedel
VoiceOver-namn för sidofältet, flikar, korten i AI-tidslinjen, Git- och CI-rader, Software Factory, felsökaren och
terminalens text; full tangentbordsstyrning med kortkommandon som du kan ändra i `keybindings.json`; Öka kontrasten och
Minska rörelse; kodvyer förblir vänster till höger i gränssnitt som läses från höger till vänster. Detaljer finns i
ACCESSIBILITY.md.

## Integritet
Ingen telemetri, ingen analys och inga kraschrapporter. Appen ansluter bara för det du startar: agenterna du använder,
hämtningarna du ber om, CI-värdarna du loggar in på, förutsägelser från Copilot om du har valt det, transkribering
online om du har valt det, sidorna du öppnar och uppdateringskontrollen när du har tillåtit den. Nycklar och tokens
stannar i nyckelringen i macOS, och agenterna i Software Factory ser dem aldrig.

## Kända begränsningar
- Inloggning till CI via webbläsaren är avstängd; använd `gh`, `glab`, en token eller miljön.
- Appen körs utan App Sandbox, eftersom terminaler, Git, språkservrar och agenter skulle ärva den; du måste lita på en
  mapp innan något som den konfigurerar körs. Antigravity och OpenCode har ingen sandlåda som appen kan slå på.
- I Software Factory kan en Codex-byggare eller -granskare läsa utanför sin worktree, eftersom Codex protokoll saknar
  läsgränser; det den skriver stannar innanför.
- Claudes användningssiffror och kontextuppdelning bygger på delar av protokollet som är märkta som experimentella och
  göms när de saknas. Att spola tillbaka konversationen fungerar för Claude, Codex och OpenCode; för de andra agenterna
  spolas bara koden tillbaka.
- Gemini CLI erbjuds inte: Google har avslutat inloggningen med personligt konto och hänvisar till Antigravity. Tidigare
  Gemini-trådar går fortfarande att läsa.
