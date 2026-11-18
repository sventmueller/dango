Aceasta este prima versiune a Dango, un editor nativ pentru macOS destinat lucrului cu agenți de programare. Editorul,
terminalele, un browser, Git, CI, un depanator, conversațiile IA și un Software Factory pentru ordine de lucru împart o
singură fereastră pentru fiecare dosar. Aplicația este scrisă în Rust pe AppKit; WebKit apare numai în fila
browserului. Interfața este disponibilă în 35 de limbi.

## Cerințe
- macOS 14 sau o versiune ulterioară, pe Apple silicon sau Intel.
- Pentru conversațiile IA, agentul pe care îl alegeți: Claude are nevoie de comanda `claude`, cu autentificarea făcută;
  Codex are nevoie de Node.js, iar aplicația îi instalează pachetul în versiunea fixată când i-o cereți; Antigravity și
  OpenCode folosesc propriul dvs. `agy` sau `opencode` ori o descărcare în versiune fixată; agentul propriu are nevoie
  de o cheie API sau de Ollama ori LM Studio pe acest Mac.
- Serverele de limbaj și adaptoarele de depanare se instalează când le cereți. gopls și `dlv` au nevoie de Go; serverele
  pentru TypeScript, Python, JSON, YAML, CSS, HTML, ESLint și shell și depanatorul JavaScript au nevoie de Node.js;
  clangd, sourcekit-lsp și `lldb-dap` vin cu Xcode Command Line Tools.
- Predicțiile în timpul tastării necesită un abonament GitHub Copilot (este suficient și cel gratuit) sau un model
  fill-in-the-middle în Ollama ori LM Studio.

## Fereastră
- Fiecare fișier, terminal, pagină de browser, conversație IA, diff și previzualizare este o filă macOS în fereastra
  dosarului. „+” de la capătul barei de file și ⌘T oferă un fișier nou, un terminal, browserul și fiecare agent
  instalat, inclusiv într-un worktree Git propriu.
- Bara laterală comută între Fișiere, Căutare, Agenți, Git, Depanare, CI, Teste, Containere și Software Factory; un clic
  dreapta le ascunde pe cele pe care nu le folosiți. Bara de instrumente conține înapoi și înainte prin locurile pe care
  le-ați editat (⌃- și ⌃⇧-), ramura, Deschidere rapidă și prezentarea Activitate.
- O fereastră se împarte în zone una lângă alta sau una deasupra celeilalte, imbricate oricât de adânc doriți, ca în
  Xcode; fiecare zonă își ține documentele unul în spatele altuia, cu o bară de file proprie. „Deschideți alături”
  afișează un al doilea document lângă primul.
- După o repornire sau o închidere neașteptată, fiecare fereastră revine cu filele și zonele sale, iar editoarele cu
  text nesalvat revin cu acel text.
- Imaginile, fișierele audio și video, fonturile și documentele Office se deschid într-o filă prin Quick Look, iar
  PDF-urile prin PDFKit.

## Editor
- Culori de sintaxă, numere de linie, marcaje de modificare față de ultimul commit, căutare și înlocuire, Deschidere
  rapidă (⌘P) și căutare în proiect (⇧⌘F). Culorile și stilurile Markdown țin pasul chiar și în fișiere de multe mii de
  linii, fără să încetinească tastarea.
- Indentare automată, tabulatori sau spații după `.editorconfig`, perechi de paranteze, comutarea comentariilor, pliere,
  salt la linie, spații albe vizibile, încadrarea liniilor, o minihartă și antetele fixate ale blocurilor care cuprind
  linia.
- Mai multe cursoare: ⌥-clic, ⌥-tragere pentru o coloană, ⌘D pentru următoarea apariție.
- **Mod Vim:** se activează în meniul Editor, cu modurile normal, insert și visual, mișcări și operatori, registre,
  marcaje, macrocomenzi, căutare și comenzi Ex; mesajele sale apar în limba interfeței.
- Abrevieri Emmet în completare pentru HTML, CSS și JSX și, dacă doriți, remedierea problemelor ESLint la salvare.
- Fișierele își păstrează codificarea, marcajul de ordine a octeților și terminațiile de linie. Un fișier modificat pe
  disc se reîncarcă atunci când editorul nu are text nesalvat; altfel, o bară oferă diferențele și comasarea.
- **Predicții în timpul tastării:** în timp ce tastați, apare o propunere în gri; Tab o acceptă. Propunerile vin de la
  GitHub Copilot sau de la un model local din Ollama ori LM Studio, după cum alegeți în Configurări; un model local vă
  păstrează codul pe acest Mac.

## Servere de limbaj
Go, Rust, TypeScript și JavaScript, Python, C, C++ și Objective-C, Swift, scripturi shell, JSON, YAML, CSS, HTML, TOML,
Markdown și Lua, ESLint alături de serverul TypeScript, plus servere proprii din Configurări. Diagnostice, inclusiv la
sfârșitul rândului, informații la trecerea cursorului, completare, ajutor pentru semnătură, salt la definiție, definiția
tipului și implementare, referințe, ierarhia apelurilor și ierarhia tipurilor, redenumire, formatare, culori semantice,
pliere și structura; tipuri și nume de parametri în interiorul liniei; remedieri rapide și refactorizări în spatele unui
bec (⌥⌘A); simbolurile proiectului cu ⌥⌘O sau `#` în Deschidere rapidă. `tsc`-ul propriu al unui proiect TypeScript
este folosit atunci când poate servi ca server de limbaj.

## Depanator
Puncte de întrerupere, inclusiv cu condiții, execuție pas cu pas, fire de execuție cu stivele lor de apeluri, variabile
și expresii urmărite în Inspector (⌥⌘I), valori lângă cod cât timp programul este întrerupt și o consolă de depanare
care evaluează în cadrul întrerupt. Rust, C, C++ și Swift prin `lldb-dap`, Go prin `dlv`, JavaScript și TypeScript prin
depanatorul JavaScript, Python prin debugpy. Fișierul `.vscode/launch.json` al unui proiect este citit. Meniul Depanare
folosește scurtăturile din Xcode.

## Teste și containere
- Panoul Teste listează testele proiectelor Rust, Go, Python, Jest, Vitest și Bun încă dinainte de prima rulare,
  rulează unul, un fișier sau pe toate, arată fiecare rezultat în timp real și îl marchează în marginea editorului.
- Panoul Containere listează containerele, imaginile, volumele și proiectele Compose din Docker; le pornește, le
  oprește și le elimină, le urmărește jurnalele și deschide un shell într-o filă de terminal.

## Terminal
Terminale în file cu mediul shellului de autentificare, desenate cu Metal. O imagine lipită sau trasă în terminal ajunge
la un agent de programare care rulează acolo; altfel, se lipește calea unei copii salvate. ⌘-clic deschide linkuri și
nume de fișiere. Un server de dezvoltare pornit din terminal poate fi deschis în fila browserului.

## Markdown
Un fișier Markdown se deschide cu previzualizarea alături (⇧⌘V), cu formule, diagrame Mermaid, Graphviz și PlantUML și
imaginile din dosar. Un meniu Format pentru aldin, cursiv, linkuri, titluri, liste, citate și blocuri de cod; textul
lipit din Word sau dintr-o pagină web ajunge ca Markdown. **Fișier › Exportați** scrie PDF, un singur fișier HTML sau un
document Word. Acolo unde Splat este instalat, **Fișier › Deschideți în Splat** deschide documentul în Splat.

## Browser
O filă de browser cu câmp de adresă, înapoi și înainte, căutare, descărcări cu confirmare prealabilă și Inspectorul web.

## Git
Un panou Git cu pregătirea pentru commit și renunțarea la fișiere întregi sau la fragmente individuale, commituri,
commit și push sau sincronizare, ramuri, stashuri, conflicte de comasare și un grafic al commiturilor recente. Blame
arată cine a modificat ultima dată linia de sub cursor sau o coloană pentru tot fișierul. Un buton scrie mesajul de
commit cu IA; diffurile pleacă doar după ce gitleaks nu a găsit niciun secret în ele, iar fiecare commit este verificat
și el pentru secrete. O conversație IA poate lucra într-un worktree Git propriu; panoul arată ce a modificat și preia
lucrul prin comasare sau cherry-pick.

## CI
GitHub Actions și GitLab CI, inclusiv GitHub Enterprise Server și GitLab autogestionat: rulările, joburile și jurnalele
ramurii curente; rularea din nou a joburilor eșuate, a unui singur job sau (pe GitHub) a unei rulări întregi, pornirea
joburilor manuale sau aprobarea implementărilor în așteptare, pornirea unui pipeline nou sau a unui workflow care poate
fi pornit manual, precum și anularea. Merge requesturile sau pull requesturile deschise și issue-urile sunt listate în
același panou și se deschid în fila browserului. Jurnalul unui job eșuat poate fi trimis unui agent, marcat ca conținut
extern. Tokenul vine din portchei, de la `gh` sau `glab`, din mediul shellului de autentificare sau îl introduceți dvs.

## IA
Fiecare agent lucrează într-o filă IA cu aceeași cronologie: răspunsuri, apeluri de instrumente cu rezultatul lor,
cereri de permisiune, întrebări, planuri de aprobat și de comentat, liste De făcut, subagenți și sarcini în fundal; două
butoane din antetul filei listează subagenții și procesele în fundal care rulează. Înainte de fiecare prompt, aplicația
creează un punct de control; modificările pot fi revizuite și derulate înapoi pentru fiecare mesaj. Conversațiile noi
încep în modul „Manual”, iar comenzile Git distructive nu sunt niciodată permise definitiv. Prezentarea Activitate
listează tot ce rulează. Un punct verde pe filă arată o sesiune care lucrează, unul roșu o sesiune care vă așteaptă, iar
o notificare vă anunță când o sesiune se termină sau așteaptă în timp ce vă uitați în altă parte. Cronologia, câmpul de
mesaj și linia de stare au o dimensiune proprie a textului în Configurări.
- **Claude** prin comanda `claude`, nemodificată, cu sesiuni, subagenți, sarcini în fundal, context, utilizare și
  Remote Control de pe telefon.
- **Codex** prin serverul său de aplicație și **OpenCode** prin serverul său, fiecare în propriul protocol, cu
  conversațiile lor, derularea înapoi în agentul însuși și Remote Control pentru Codex.
- **Agenți ACP:** Antigravity, plus alții din registrul ACP la cerere; agenții instalați sunt fixați pe versiuni testate
  și verificați după sumele lor de control.
- **Agent propriu:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio și servere compatibile cu API-ul
  OpenAI sau Anthropic, cu instrumente pentru fișiere, căutare, comenzi, diagnostice și cereri web; cheile rămân în
  portchei.
- Comenzile proprii ale unui agent se deschid ca suprapuneri peste filă, la fel pentru fiecare agent: stare, export,
  servere MCP, permisiuni, hookuri, memorie, configurație, skilluri, pluginuri, model, cont și feedback.
- Un comutator de sandbox pentru fiecare conversație limitează ce pot face comenzile unui agent, în măsura în care
  agentul permite acest lucru.
- Conținutul extern, cum ar fi rezultatul din terminal, o pagină web sau un jurnal CI, ajunge la agent marcat ca
  nesigur.

## Software Factory
Ordine de lucru în loc de prompturi izolate: un obiectiv cu criterii de acceptare, scris de mână sau preluat dintr-un
issue. Fiecare ordin este planificat, construit și verificat într-un worktree Git propriu, de agenți ai unor producători
diferiți, în roluri fixe: un planificator scrie specificația, un constructor face lucrul, iar un evaluator de la alt
producător verifică fiecare criteriu. Comenzile de control ale proiectului, cum ar fi build-ul și testele, rulează
într-un sandbox, iar constructorul remediază ce găsesc acestea.
- Decideți în două puncte: aprobați specificația și acceptați rezultatul cu dovezile sale, integral sau secțiune cu
  secțiune. Nimic nu este preluat fără acceptarea dvs.
- Panoul și fila Software Factory arată fiecare ordin cu stația sa, costul și ceea ce așteaptă; fila unui ordin îi
  arată cronologia, dovezile și pasul următor și îi deschide sesiunea în curs ca filă IA.
- Bugete pe ordin și pe zi, limite de runde, o pauză la ieșire, la repaus și la baterie descărcată și un jurnal scris
  înaintea fiecărui efect, astfel încât ordinele se reiau după o repornire.
- Issue-urile cu o etichetă aleasă pot intra ca ordine; un ordin acceptat poate pleca, la clicul dvs., ca merge request
  cu un comentariu la issue-ul său. Ordinele cu domenii de căi separate rulează în paralel și sunt preluate unul după
  altul.
- Indicatori pe agent și pe producător, rutare după rata primei treceri, propuneri de reguli pentru `AGENTS.md` și
  `CLAUDE.md` din constatările recurente și ordine de lucru recurente din fișierul workflow al proiectului.
- Fișierul workflow al unui dosar, `.dango/factory.toml`, conține controalele, clasele de risc și domeniile de căi ale
  acestuia; comenzile de control modificate și tot ce relaxează limitele așteaptă confirmarea dvs.

## Dictare
Dictare în câmpul de mesaj al filei IA și în editor prin recunoașterea vocală a Lowspeak: butonul de microfon, ⌃⌥D sau
tasta fn cât timp Dango este aplicația activă. Folosește configurările și modelele Lowspeak; transcrierea online
necesită o cheie OpenAI sau Groq.

## Actualizări
Aplicația se actualizează singură prin Sparkle. La a doua lansare vă întreabă dacă poate căuta singură actualizări; până
nu sunteți de acord, nu face nicio cerere. **Dango › Căutați actualizări…** caută imediat.

## Accesibilitate
Nume VoiceOver pentru bara laterală, file, cardurile din cronologia IA, rândurile Git și CI, Software Factory, depanator
și textul terminalului; control complet de la tastatură, cu scurtături pe care le puteți modifica în
`keybindings.json`; Măriți contrastul și Reduceți mișcarea; vizualizările de cod rămân de la stânga la dreapta în
interfețele de la dreapta la stânga. Detalii în ACCESSIBILITY.md.

## Confidențialitate
Fără telemetrie, fără analize și fără rapoarte de erori. Aplicația se conectează doar pentru ce porniți dvs.: agenții pe
care îi folosiți, descărcările pe care le cereți, gazdele CI la care vă autentificați, predicțiile de la Copilot dacă
le-ați ales, transcrierea online dacă ați ales-o, paginile pe care le deschideți și căutarea actualizărilor după ce ați
permis-o. Cheile și tokenurile rămân în portcheiul macOS, iar agenții din Software Factory nu le văd niciodată.

## Limitări cunoscute
- Autentificarea la CI prin browser este dezactivată; folosiți `gh`, `glab`, un token sau mediul.
- Aplicația rulează fără App Sandbox, deoarece terminalele, Git, serverele de limbaj și agenții l-ar moșteni; trebuie să
  aveți încredere într-un dosar înainte să ruleze ceva configurat de acesta. Antigravity și OpenCode nu au un sandbox pe
  care aplicația să-l poată activa.
- În Software Factory, un constructor sau evaluator Codex poate citi în afara worktree-ului său, deoarece protocolul
  Codex nu are limite de citire; scrierile sale rămân în interior.
- Cifrele de utilizare și defalcarea contextului pentru Claude se bazează pe părți ale protocolului său marcate ca
  experimentale și sunt ascunse când acestea lipsesc. Derularea înapoi a discuției funcționează pentru Claude, Codex și
  OpenCode; la ceilalți agenți se derulează înapoi doar codul.
- Gemini CLI nu este oferit: Google a încheiat autentificarea cu un cont personal și trimite la Antigravity.
  Conversațiile Gemini anterioare rămân lizibile.
