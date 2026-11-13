<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
To pierwsze wydanie Dango, natywnego edytora dla macOS do pracy z agentami programistycznymi. Edytor, terminale,
przeglądarka, Git, CI, debuger i wątki AI dzielą jedno okno na każdy folder. Aplikacja jest napisana w Rust na AppKit;
WebKit pojawia się tylko w karcie przeglądarki. Interfejs jest dostępny w 35 językach.

## Wymagania
- macOS 14 lub nowszy, na Macu z procesorem Apple lub Intel.
- Do wątków AI wybrany agent: Claude wymaga polecenia `claude` z zalogowanym kontem; Codex wymaga Node.js, a aplikacja
  na żądanie instaluje jego pakiet w przypiętej wersji; Antigravity i OpenCode używają twojego własnego `agy` lub
  `opencode` albo pobrania w przypiętej wersji; wbudowany agent wymaga klucza API albo aplikacji Ollama lub LM Studio na
  tym Macu.
- Serwery języka i adaptery debugowania są instalowane na żądanie. gopls i `dlv` wymagają Go; serwery dla TypeScript,
  Python, JSON, YAML, CSS, HTML i powłoki oraz debuger JavaScript wymagają Node.js; clangd, sourcekit-lsp i `lldb-dap`
  są dostarczane z Xcode Command Line Tools.
- Przewidywanie podczas pisania wymaga subskrypcji GitHub Copilot (wystarczy darmowa) albo modelu fill-in-the-middle w
  Ollama lub LM Studio.

## Okno
- Każdy plik, terminal, strona przeglądarki, wątek AI, widok różnic i podgląd to karta macOS w oknie folderu. „+” na
  końcu paska kart i ⌘T oferują nowy plik, terminal, przeglądarkę i każdego zainstalowanego agenta, również we własnym
  worktree Git.
- Pasek boczny przełącza między widokami Pliki, Wyszukiwanie, Agenty, Git, Debuguj i CI (⌘1–⌘6); kliknięcie prawym
  przyciskiem ukrywa te, których nie używasz. Na pasku narzędzi są Wstecz i Dalej przez edytowane miejsca (⌃- i ⌃⇧-),
  gałąź, Szybko otwórz i przegląd Aktywność.
- „Otwórz obok” pokazuje drugi dokument obok pierwszego. Po ponownym uruchomieniu lub awarii każde okno wraca ze swoimi
  kartami, a edytory z niezachowanym tekstem wracają razem z nim.
- Obrazy, dźwięk, wideo, czcionki i dokumenty Office otwierają się w karcie przez Quick Look, pliki PDF przez PDFKit.

## Edytor
- Kolorowanie składni, numery wierszy, znaczniki zmian względem ostatniego commitu, znajdowanie i zastępowanie, Szybko
  otwórz (⌘P) i wyszukiwanie w projekcie (⇧⌘F).
- Automatyczne wcięcia, tabulatory lub spacje według `.editorconfig`, pary nawiasów, przełączanie komentarzy, zwijanie,
  przejście do wiersza, widoczne białe znaki i zawijanie wierszy.
- Wiele kursorów: ⌥-kliknięcie, ⌥-przeciągnięcie dla kolumny, ⌘D dla następnego wystąpienia.
- Pliki zachowują kodowanie, znacznik kolejności bajtów i znaki końca wiersza. Plik zmieniony na dysku jest wczytywany
  ponownie, gdy edytor nie ma niezachowanego tekstu; w przeciwnym razie pasek oferuje różnice i scalanie.
- **Przewidywanie podczas pisania:** gdy piszesz, propozycja pojawia się w kolorze szarym; Tab ją przyjmuje. Propozycje
  pochodzą z GitHub Copilot albo z lokalnego modelu w Ollama lub LM Studio, zależnie od wyboru w ustawieniach; lokalny
  model zatrzymuje twój kod na tym Macu.

## Serwery języka
Go, Rust, TypeScript i JavaScript, Python, C, C++ i Objective-C, Swift, skrypty powłoki, JSON, YAML, CSS, HTML, TOML,
Markdown i Lua, a także własne serwery z ustawień. Diagnostyka, podpowiedzi po najechaniu, uzupełnianie, pomoc dotycząca
sygnatur, przejście do definicji, definicji typu i implementacji, odwołania, hierarchia wywołań i hierarchia typów,
zmiana nazwy, formatowanie, kolory semantyczne, zwijanie i konspekt; typy i nazwy parametrów wewnątrz wiersza; szybkie
poprawki i refaktoryzacje pod żarówką (⌥⌘A); symbole projektu przez ⌥⌘O lub `#` w Szybko otwórz. Własny `tsc` projektu
TypeScript jest używany, gdy może służyć jako serwer języka.

## Debuger
Punkty przerwania, także z warunkami, praca krokowa, wątki z ich stosami wywołań, zmienne i obserwowane wyrażenia w
Inspektorze (⌥⌘I), wartości obok kodu, gdy program jest wstrzymany, oraz konsola debugowania, która oblicza wyrażenia we
wstrzymanej ramce. Rust, C, C++ i Swift przez `lldb-dap`, Go przez `dlv`, JavaScript i TypeScript przez debuger
JavaScript, Python przez debugpy. Plik `.vscode/launch.json` projektu jest odczytywany. Menu Debuguj używa skrótów z
Xcode.

## Terminal
Terminale w kartach ze środowiskiem powłoki logowania, rysowane przez Metal. Wklejony lub upuszczony obraz trafia do
agenta programistycznego działającego w terminalu, a w przeciwnym razie wklejana jest ścieżka zachowanej kopii.
⌘-kliknięcie otwiera łącza i nazwy plików. Serwer deweloperski uruchomiony z terminala można otworzyć w karcie
przeglądarki.

## Markdown
Plik Markdown otwiera się z podglądem obok (⇧⌘V), ze wzorami, diagramami Mermaid, Graphviz i PlantUML oraz obrazkami z
folderu. Menu Format do pogrubienia, kursywy, łączy, nagłówków, list, cytatów i bloków kodu; tekst wklejony z programu
Word lub ze strony internetowej trafia jako Markdown. **Plik › Eksportuj** zapisuje PDF, pojedynczy plik HTML lub
dokument Word. Gdy Splat jest zainstalowany, **Plik › Otwórz w Splat** otwiera w nim dokument.

## Przeglądarka
Karta przeglądarki z polem adresu, przyciskami Wstecz i Dalej, wyszukiwaniem, pobieraniem poprzedzonym pytaniem oraz
Inspektorem www.

## Git
Panel Git z przygotowywaniem i odrzucaniem całych plików lub pojedynczych fragmentów, commitami, commitem z pushem lub
synchronizacją, gałęziami, stashami, konfliktami scalania i grafem ostatnich commitów. Blame pokazuje, kto ostatni
zmienił wiersz pod kursorem, albo kolumnę dla całego pliku. Przycisk pisze komunikat commitu za pomocą AI; różnice są
wysyłane dopiero wtedy, gdy gitleaks nie znajdzie w nich żadnego sekretu, a każdy commit jest również sprawdzany pod
kątem sekretów. Wątek AI może pracować we własnym worktree Git; panel pokazuje, co zmienił, i przejmuje jego pracę przez
scalenie lub cherry-pick.

## CI
GitHub Actions i GitLab CI, także GitHub Enterprise Server i samodzielnie zarządzany GitLab: uruchomienia, joby i logi
bieżącej gałęzi; ponowne uruchamianie nieudanych jobów, pojedynczego joba lub (w GitHub) całego uruchomienia,
uruchamianie ręcznych jobów lub zatwierdzanie oczekujących wdrożeń, uruchamianie nowego pipeline'u lub workflow, który
można uruchomić ręcznie, oraz anulowanie. Otwarte merge requesty lub pull requesty i zgłoszenia są wyświetlane w tym
samym panelu i otwierają się w karcie przeglądarki. Log nieudanego joba można przekazać agentowi, oznaczony jako treść
zewnętrzna. Token pochodzi z pęku kluczy, z `gh` lub `glab`, ze środowiska powłoki logowania albo jest wpisywany
ręcznie.

## AI
Każdy agent pracuje w karcie AI z taką samą osią czasu: odpowiedzi, wywołania narzędzi z ich wynikami, prośby o
uprawnienia, pytania, plany do zatwierdzenia i skomentowania, listy do zrobienia, subagenty i zadania w tle; dwa
przyciski w nagłówku karty wyświetlają działające subagenty i procesy w tle. Przed każdym promptem aplikacja tworzy
punkt kontrolny; zmiany można przejrzeć i przewinąć do tyłu dla każdej wiadomości. Nowe wątki zaczynają w trybie
„Ręcznie”, a destrukcyjne polecenia Git nigdy nie są dozwolone na stałe. Przegląd Aktywność pokazuje wszystko, co
działa. Zielona kropka na karcie oznacza pracującą sesję, czerwona – sesję, która czeka na ciebie, a powiadomienie
informuje, gdy sesja się zakończy lub zacznie czekać, podczas gdy zajmujesz się czymś innym.
- **Claude** przez niezmienione polecenie `claude`, z sesjami, subagentami, zadaniami w tle, kontekstem i użyciem.
- **Agenty ACP:** Codex, Antigravity i OpenCode, a na żądanie kolejne z rejestru ACP; zainstalowane agenty są przypięte
  do przetestowanych wersji i sprawdzane względem ich sum kontrolnych.
- **Wbudowany agent:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio i serwery obsługujące API OpenAI
  lub Anthropic, z narzędziami do plików, wyszukiwania, poleceń, diagnostyki i żądań sieciowych; klucze pozostają w pęku
  kluczy.
- Przełącznik piaskownicy w każdym wątku ogranicza, co mogą robić polecenia agenta, o ile agent to obsługuje.
- Treść zewnętrzna, taka jak wynik terminala, strona internetowa czy log CI, trafia do agenta oznaczona jako niezaufana.

## Dyktowanie
Dyktowanie do pola wiadomości w karcie AI i do edytora przez rozpoznawanie mowy Lowspeak: przycisk mikrofonu, ⌃⌥D lub
klawisz fn, gdy Dango jest aktywną aplikacją. Korzysta z ustawień i modeli Lowspeak; transkrypcja online wymaga klucza
OpenAI lub Groq.

## Uaktualnienia
Aplikacja uaktualnia się sama przez Sparkle. Przy drugim uruchomieniu pyta, czy może sama sprawdzać uaktualnienia;
dopóki się nie zgodzisz, nie wysyła żadnych żądań. **Dango › Sprawdź uaktualnienia…** sprawdza od razu.

## Dostępność
Nazwy VoiceOver dla paska bocznego, kart, elementów osi czasu AI, wierszy Git i CI, debugera i tekstu terminala; pełna
obsługa z klawiatury ze skrótami, które można zmienić w `keybindings.json`; Zwiększ kontrast i Ogranicz ruch; widoki
kodu pozostają od lewej do prawej w interfejsach pisanych od prawej do lewej. Szczegóły w ACCESSIBILITY.md.

## Prywatność
Bez telemetrii, bez analityki i bez raportów o awariach. Aplikacja łączy się z siecią tylko na potrzeby tego, co
uruchamiasz: agentów, których używasz, pobrań, o które prosisz, hostów CI, do których się logujesz, przewidywania z
Copilot, jeśli je wybierzesz, transkrypcji online, jeśli ją wybierzesz, otwieranych stron oraz sprawdzania uaktualnień,
gdy na nie pozwolisz. Klucze i tokeny pozostają w pęku kluczy macOS.

## Znane ograniczenia
- Logowanie do CI przez przeglądarkę jest wyłączone; użyj `gh`, `glab`, tokenu lub środowiska.
- Aplikacja działa bez App Sandbox, ponieważ terminale, Git, serwery języka i agenty by ją dziedziczyły; folder musi być
  zaufany, zanim uruchomi się cokolwiek, co on konfiguruje. Antigravity i OpenCode nie mają piaskownicy, którą aplikacja
  mogłaby włączyć.
- Statystyki użycia i rozbicie kontekstu Claude opierają się na częściach jego protokołu oznaczonych jako
  eksperymentalne i są ukrywane, gdy ich brakuje. Przewijanie rozmowy do tyłu działa tylko dla Claude; w pozostałych
  agentach do tyłu przewijany jest tylko kod.
- Gemini CLI nie jest oferowany: Google zakończył logowanie kontem osobistym i odsyła do Antigravity. Wcześniejsze wątki
  Gemini pozostają czytelne.
