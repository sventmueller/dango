<p><img src="../../readme-icon.png" alt="Ikona Dango" width="147" height="147"></p>

# Dango

**Natywny edytor dla macOS do pracy z agentami programistycznymi.**

Dango zbiera kod, agenty, które go zmieniają, i wszystko wokół nich w jednym oknie na folder: edytor z serwerami języka i przewidywaniem podczas pisania, terminale, przeglądarkę, Git, CI, debuger i wątki AI z Claude, Codex, Antigravity, OpenCode albo agentem wbudowanym w aplikację. Każdy agent pracuje na tej samej osi czasu, pyta, zanim zmieni pliki lub uruchomi polecenia, i przed każdym promptem zostawia punkt kontrolny, więc możesz przejrzeć i przewinąć do tyłu to, co zrobił, wiadomość po wiadomości. Nic, co konfiguruje folder, nie uruchamia się, zanim mu zaufasz, a tekst z zewnątrz trafia do agenta oznaczony jako niezaufany. Dyktuj do edytora i pola wiadomości; rozpoznawanie mowy może zostać na Twoim Macu. Dango jest napisane w Rust na prawdziwych kontrolkach AppKit, bez Electrona, mówi w 35 językach i nie ma telemetrii.

[![Pobierz Dango.dmg](https://img.shields.io/badge/Pobierz_Dango-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Pobierz Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: podpisany i poświadczony przez Apple, dla macOS 14 lub nowszego na Apple Silicon i Intel. Łącze będzie dostępne od pierwszego wydania.

**Znalazłeś błąd?** [Otwórz zgłoszenie](https://github.com/sventmueller/dango/issues/new) i napisz, z jakim agentem i w jakim projekcie wystąpił.

[![Rust, edycja 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org) [![macOS 14 lub nowszy](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/) ![Uniwersalna: Apple Silicon i Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white) [![AppKit przez objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2) [![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com) [![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/) [![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/) [![Terminal: Tapa term-vt](https://img.shields.io/badge/terminal-Tapa_term--vt-F46D01)](https://github.com/sventmueller/tapa) ![35 języków interfejsu](https://img.shields.io/badge/languages-35-4C9A2A) ![Bez telemetrii](https://img.shields.io/badge/telemetry-none-4C9A2A) ![Podpisane z Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A) [![Licencja: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#licencja)

[Bahasa Indonesia](../id/README.md) · [Bahasa Melayu](../ms/README.md) · [Català](../ca/README.md) · [Čeština](../cs/README.md) · [Dansk](../da/README.md) · [Deutsch](../de/README.md) · [English](../../../README.md) · [Español](../es/README.md) · [Français](../fr/README.md) · [Hrvatski](../hr/README.md) · [Italiano](../it/README.md) · [Magyar](../hu/README.md) · [Nederlands](../nl/README.md) · [Norsk](../no/README.md) · **Polski** · [Português (Brasil)](../pt_BR/README.md) · [Português (Portugal)](../pt_PT/README.md) · [Română](../ro/README.md) · [Slovenčina](../sk/README.md) · [Slovenščina](../sl/README.md) · [Suomi](../fi/README.md) · [Svenska](../sv/README.md) · [Tiếng Việt](../vi/README.md) · [Türkçe](../tr/README.md) · [Ελληνικά](../el/README.md) · [Русский](../ru/README.md) · [Українська](../uk/README.md) · [עברית](../he/README.md)‎ · [العربية](../ar/README.md)‎ · [हिन्दी](../hi/README.md) · [ไทย](../th/README.md) · [한국어](../ko/README.md) · [日本語](../ja/README.md) · [简体中文](../zh-Hans/README.md) · [繁體中文](../zh-Hant/README.md)

## Spis treści

- [Funkcje w skrócie](#funkcje-w-skrócie)
- [Wymagania](#wymagania)
- [Instalacja](#instalacja)
- [Pierwsze kroki](#pierwsze-kroki)
- [Agenty AI](#agenty-ai)
  - [Oś czasu](#oś-czasu)
  - [Uprawnienia i tryby](#uprawnienia-i-tryby)
  - [Punkty kontrolne i worktree](#punkty-kontrolne-i-worktree)
- [Edytor](#edytor)
- [Serwery języka](#serwery-języka)
- [Testy](#testy)
- [Debuger](#debuger)
- [Terminal](#terminal)
- [Przeglądarka](#przeglądarka)
- [Git](#git)
- [CI](#ci)
- [Kontenery](#kontenery)
- [Markdown](#markdown)
- [Dyktowanie](#dyktowanie)
- [Zaufanie](#zaufanie)
- [Piaskownica](#piaskownica)
- [Wygląd i dostępność](#wygląd-i-dostępność)
- [Języki](#języki)
- [Skróty klawiszowe](#skróty-klawiszowe)
- [Prywatność i bezpieczeństwo](#prywatność-i-bezpieczeństwo)
- [Gdzie są dane](#gdzie-są-dane)
- [Znane ograniczenia](#znane-ograniczenia)
- [Kompilacja ze źródeł](#kompilacja-ze-źródeł)
- [Licencja](#licencja)

## Funkcje w skrócie

- **Wszystkie agenty na jednej osi czasu.** Claude przez własne polecenie `claude`, Codex, Antigravity i OpenCode przez Agent Client Protocol, kolejne z rejestru ACP oraz agent wbudowany w aplikację dla Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama i LM Studio. Odpowiedzi, wywołania narzędzi, prośby o pozwolenie, plany, listy do zrobienia, subagenty i zadania w tle wyglądają tak samo, niezależnie od agenta.
- **Przeglądanie i przewijanie do tyłu.** Przed każdym promptem Dango zachowuje punkt kontrolny. To, co zmienił agent, można przejrzeć i przewinąć do tyłu dla każdej wiadomości, a wątek może pracować we własnym worktree Git i przekazać pracę przez scalenie lub cherry-pick.
- **Jedno okno na folder.** Pliki, terminale, strony WWW, wątki AI, różnice i podglądy są kartami macOS tego okna. Pasek boczny zawiera Pliki, Wyszukiwanie, Agenty, Git, Testy, Debugowanie, CI i Kontenery.
- **Prawdziwy edytor kodu.** Kolorowanie składni, wiele punktów wstawiania, zwijanie, `.editorconfig`, serwery języka dla 17 języków z uzupełnianiem, przejściem do definicji, odwołaniami, zmianą nazwy, szybkimi poprawkami i hierarchiami, a także przewidywanie podczas pisania z GitHub Copilot lub modelu lokalnego.
- **Wbudowany debuger, Git i CI.** Punkty przerwania i praca krokowa dla Rust, C, C++, Swift, Go, JavaScript, TypeScript i Python; przygotowywanie pojedynczych fragmentów, gałęzie, stashe, konflikty i graf commitów; uruchomienia, joby i logi z GitHub Actions i GitLab CI.
- **Ostrożność wobec Twoich folderów.** Folder, któremu nie ufasz, niczego nie uruchamia: żadnych procesów Git, serwerów języka, hooków ani agentów. Destrukcyjne polecenia Git nigdy nie są dozwolone na stałe, a commity są sprawdzane pod kątem sekretów.
- **Dyktowanie.** Mów do edytora i pola wiadomości, online przez OpenAI lub Groq albo lokalnie z Whisper i Parakeet.
- **Natywne do szpiku kości.** Okna, karty, menu i paski narzędzi AppKit, TextKit 2 dla edytora, Metal dla terminala, WebKit tylko w karcie przeglądarki. Tryb ciemny, Zwiększ kontrast i VoiceOver działają jak w każdej aplikacji na Maca.
- **W Twoim języku.** Interfejs jest dostępny w 35 językach: arabskim, katalońskim, chińskim (uproszczonym), chińskim (tradycyjnym), chorwackim, czeskim, duńskim, holenderskim, angielskim, fińskim, francuskim, niemieckim, greckim, hebrajskim, hindi, węgierskim, indonezyjskim, włoskim, japońskim, koreańskim, malajskim, norweskim, polskim, portugalskim (Brazylia), portugalskim (Portugalia), rumuńskim, rosyjskim, słowackim, słoweńskim, hiszpańskim, szwedzkim, tajskim, tureckim, ukraińskim i wietnamskim.

## Wymagania

- macOS 14 Sonoma lub nowszy, na Apple Silicon lub Intel. Dango jest rozwijane i testowane na macOS 27.
- Dla wątków AI agent, którego chcesz używać; zobacz [Agenty AI](#agenty-ai).
- Dla serwerów języka i adapterów debugowania, które Dango instaluje na żądanie: Go dla gopls i `dlv`; Node.js dla serwerów TypeScript, Python, JSON, YAML, CSS, HTML i powłoki oraz debugera JavaScript; Xcode Command Line Tools dla clangd, sourcekit-lsp i `lldb-dap`. rust-analyzer jest pobierany z jego wydań na GitHubie.
- Dla przewidywania podczas pisania plan GitHub Copilot (wystarczy darmowy) albo model fill-in-the-middle w Ollama lub LM Studio.

## Instalacja

**Z obrazu dysku.** Każda wersja to obraz dysku, `Dango-<version>.dmg`, z jedną uniwersalną aplikacją dla Apple Silicon i Intel, podpisaną identyfikatorem Developer ID i poświadczoną przez Apple. Otwórz go i przeciągnij Dango do folderu Aplikacje. Cask Homebrew instaluje ten sam obraz. Żadna wersja nie została jeszcze wydana.

**Uaktualnienia.** Dango z obrazu dysku samo dba o aktualność za pomocą [Sparkle](https://sparkle-project.org). Przy drugim uruchomieniu pyta, czy może samo sprawdzać uaktualnienia; dopóki się nie zgodzisz, nie wysyła żadnego żądania. **Dango › Sprawdź uaktualnienia…** sprawdza od razu. Uaktualnienia są podpisane własnym kluczem aplikacji, który Sparkle sprawdza przed instalacją czegokolwiek. Samodzielnie skompilowana wersja nie ma mechanizmu uaktualnień.

**Ze źródeł.** Zobacz [Kompilacja ze źródeł](#kompilacja-ze-źródeł), a potem przenieś `target/release/Dango.app` do `/Applications`.

**Polecenie terminala.** **Dango › Zainstaluj polecenie „dango”…** zapisuje mały skrypt w `~/.local/bin/dango`, bez uprawnień administratora; `~/.local/bin` musi być w Twoim `PATH`. Potem `dango .` otwiera bieżący folder, a `dango file.rs` plik. Dango nigdy nie zastępuje tam pliku, którego samo nie zapisało.

## Pierwsze kroki

1. **Otwórz folder** za pomocą ⌘O lub `dango <folder>`. Za pierwszym razem Dango pyta, czy mu ufasz (zobacz [Zaufanie](#zaufanie)).
2. **Pasek boczny** przełącza między Plikami (⌘1), Wyszukiwaniem (⌘2), Agentami (⌘3), Git (⌘4), Testami (⌘5), Debugowaniem (⌘6), CI (⌘7) i Kontenerami (⌘8); kliknięcie prawym przyciskiem ukrywa te, których nie używasz, a ⌃⌘S zwija pasek boczny.
3. **Każdy dokument jest kartą** okna folderu, kartą samego macOS. „+” na końcu paska kart i ⌘T oferują nowy plik, terminal, przeglądarkę i każdego zainstalowanego agenta, także we własnym worktree Git; przytrzymanie „+” pokazuje to samo jako menu.
4. **Szybko otwórz** za pomocą ⌘P znajduje pliki, a z `#` symbole projektu; ⇧⌘F wyszukuje i zastępuje w projekcie.
5. **Rozmawiaj z agentem** w karcie AI. ⌘Esc przenosi fokus klawiatury do jego pola wiadomości i z powrotem.
6. **Ustawienia** za pomocą ⌘, zawierają karty Ogólne, CI, Agenty, Wbudowany agent, Serwer języka, Dyktowanie i Przewidywanie.

Pasek narzędzi zawiera wstecz i dalej przez edytowane miejsca (⌃- i ⌃⇧-), gałąź, Szybko otwórz i przegląd Aktywność, który pokazuje wszystko, co działa. **Otwórz obok** w menu kontekstowym pliku pokazuje drugi dokument obok pierwszego. Po ponownym uruchomieniu lub awarii każde okno wraca ze swoimi kartami, a edytory z niezachowanym tekstem wracają z tym tekstem. Obrazki, dźwięk, wideo, czcionki i dokumenty Office otwierają się w karcie przez Szybki przegląd, pliki PDF przez PDFKit.

## Agenty AI

| Agent | Jak działa | Czego potrzebuje |
|---|---|---|
| Claude | polecenie `claude`, z którym Dango rozmawia bezpośrednio przez jego protokół stream-json | zainstalowany i zalogowany `claude`; Dango szuka w `PATH` powłoki logowania, w `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` i `/usr/local/bin`. Logowanie odbywa się tylko w samym `claude` |
| Codex | `codex-acp` przez Agent Client Protocol (ACP) | Node.js i npm; Dango instaluje przypięty pakiet na żądanie |
| Antigravity | `agy acp` | własne `agy` w `PATH` albo przypięte pobranie, instalowane na żądanie |
| OpenCode | `opencode acp` | własne `opencode` w `PATH` albo przypięte pobranie, instalowane na żądanie |
| Inne agenty ACP | z rejestru ACP, pobieranego tylko na żądanie | to, co podaje wpis w rejestrze; wpisy bez sumy kontrolnej instalują się dopiero po Twoim potwierdzeniu |
| Wbudowany agent | wbudowany w aplikację, z narzędziami do plików, wyszukiwania, poleceń, diagnostyki i żądań sieciowych | klucz API dla Anthropic, OpenAI, Google Gemini lub OpenRouter, przechowywany w pęku kluczy; Ollama lub LM Studio na tym Macu bez klucza; każdy serwer obsługujący API OpenAI lub Anthropic |

**Ustawienia › Agenty** pokazuje, jakie agenty ma ten Mac, instaluje pozostałe i ich uaktualnienia oraz otwiera rejestr ACP. Agenty instalowane przez Dango są przypięte do przetestowanych wersji i sprawdzane według opublikowanych sum kontrolnych (npm integrity lub SHA-256); Dango nie uruchamia pakietów `npx` bez wersji.

### Oś czasu

Każdy agent pracuje w karcie AI z tą samą osią czasu: odpowiedzi, wywołania narzędzi z ich wynikami, prośby o pozwolenie, pytania, plany do zatwierdzenia i skomentowania (⌥⌘K), listy do zrobienia, subagenty i zadania w tle. Dwa przyciski w nagłówku karty pokazują działające subagenty i procesy w tle. Wspomnij plik, `@terminal` lub `@browser`, aby przekazać agentowi ich zawartość. Zielona kropka na karcie oznacza pracującą sesję, czerwona sesję, która na Ciebie czeka, a powiadomienie informuje, gdy sesja skończy lub czeka, a Ty patrzysz gdzie indziej. W przypadku Claude wiersz stanu pokazuje kontekst i zużycie.

### Uprawnienia i tryby

Nowe wątki zaczynają w trybie „Ręcznie”: agent pyta, zanim zmieni pliki lub uruchomi polecenia. Dla destrukcyjnych poleceń Git (`push --force`, `reset --hard`, `clean -fd`) Dango nie oferuje „Zawsze pozwalaj”. Wbudowany agent przechowuje swoje reguły „Zawsze pozwalaj” dla każdego projektu, pokazuje je w **Ustawienia › Wbudowany agent** i przy destrukcyjnych poleceniach Git pyta za każdym razem, niezależnie od tego, co zapisano; Claude i agenty ACP trzymają takie reguły we własnej konfiguracji. Przy „Pozwól na wszystko” i bez piaskownicy wątek pokazuje stałe ostrzeżenie.

### Punkty kontrolne i worktree

Przed każdym promptem Dango zachowuje punkt kontrolny jako commit w `refs/ide-checkpoints/` w repozytorium projektu albo we własnym folderze dla projektu bez Git; zwykły push lub fetch nigdy ich nie zabiera. To, co zmienił agent, można przejrzeć i przewinąć do tyłu dla każdej wiadomości. Wątek może pracować we własnym worktree Git: panel Git pokazuje, co zmienił, i przejmuje pracę przez scalenie lub cherry-pick.

## Edytor

- **Kolorowanie składni, numery wierszy i znaczniki zmian** względem ostatniego commitu, znajdź i zastąp, Szybko otwórz (⌘P) i wyszukiwanie w projekcie (⇧⌘F).
- **Wcięcia** zależą od pliku i `.editorconfig`: tabulatory lub spacje, automatyczne wcięcia, pary nawiasów.
- **Wiele punktów wstawiania:** ⌥-kliknięcie, ⌥-przeciągnięcie dla kolumny, ⌃⇧↑ i ⌃⇧↓, ⌘D dla następnego wystąpienia.
- **Zwijanie** (⌥⌘← i ⌥⌘→, z ⇧ dla wszystkiego), przełączanie komentarza (⌘/), przejście do wiersza (⌃G), widoczne białe znaki i zawijanie wierszy (⌥Z).
- **Pliki zachowują kodowanie**, znacznik kolejności bajtów i końce wierszy. Plik zmieniony na dysku jest wczytywany ponownie, gdy edytor nie ma niezachowanego tekstu; w przeciwnym razie pasek oferuje różnice i scalenie.
- **Przewidywanie podczas pisania.** Gdy piszesz, pojawia się szara podpowiedź, a Tab ją przyjmuje. Pochodzi z GitHub Copilot lub z modelu lokalnego w Ollama lub LM Studio, zależnie od wyboru w **Ustawienia › Przewidywanie**; model lokalny zostawia Twój kod na tym Macu.
- **Minimapa i przyklejone przewijanie** (menu Edytor): wąski obraz całego pliku obok tekstu oraz wiersze nagłówków bloków, w których leży górny wiersz, przytrzymane nad nim.
- **Diagnostyka na końcu wiersza** (menu Widok): najpoważniejszy komunikat każdego wiersza po sekundzie spokoju, z symbolem jego ważności.
- **Podgląd na żywo stron statycznych:** Edytor › Otwórz w karcie przeglądarki udostępnia strony zaufanego folderu prosto z Dango i odświeża kartę, gdy plik zostanie zachowany.
- **Prettier** formatuje JavaScript, TypeScript, CSS, HTML, Markdown i inne, gdzie projekt go konfiguruje, za pomocą własnego `node_modules/.bin/prettier` projektu; ustawienie `formatter` wybiera formater dla każdego języka.
- **Tryb Vim** (Edytor › Tryb Vim): tryb normalny, wprowadzania, wizualny i zamiany, liczniki, operatory z ruchami i obiektami tekstowymi, `.`, rejestry, zakładki i makra. `u` i ⌃R korzystają z cofania edytora, a jedno przejście wprowadzania to jeden krok. `"+` i `"*` to schowek; ustawienie `vimClipboard` sprawia, że schowkiem staje się też rejestr nienazwany. `/`, `?`, `n`, `N`, `*`, `:s` i `:%s` szukają wyrażeniami regularnymi Rust, nie wzorcami Vim: bez `\v`, a `\b` zamiast `\<` i `\>`. Polecenia Ex to `:w`, `:q`, `:wq`, `:x`, `:noh` i `:<line>`. W trybie normalnym Vim zabiera ⌃G przejściu do wiersza.
- **Synchronizacja ustawień** przez folder na iCloud Drive (**Ustawienia › Ogólne**): Twoje ustawienia i skróty podążają za Tobą na Twoje inne Maki; ścieżki, adresy lokalne i zgody zostają na każdym z osobna.

## Serwery języka

Go, Rust, TypeScript i JavaScript, Python, C, C++ i Objective-C, Swift, skrypty powłoki, JSON, YAML, CSS, HTML, TOML, Markdown i Lua, a także własne serwery z **Ustawienia › Serwer języka**. Dango instaluje serwer na żądanie we własnym folderze narzędzi i używa `tsc` projektu TypeScript, gdy ten może służyć jako serwer języka.

Co dają: diagnostykę (⌘' i ⇧⌘' przechodzą między komunikatami), informacje po najechaniu (⌃⌘I), uzupełnianie (⌥Esc), pomoc dla sygnatur, przejście do definicji (⌃⌘J), definicji typu i implementacji, odwołania (⌃⇧⌘F), hierarchie wywołań i typów (⌃⇧⌘H), zmianę nazwy (⌃⌘E), formatowanie (⌥⇧F), kolory semantyczne, zwijanie i konspekt (⇧⌘O); typy i nazwy parametrów wewnątrz wiersza; szybkie poprawki i refaktoryzacje pod żarówką (⌥⌘A); symbole projektu z ⌥⌘O.

Obok własnego serwera pliku ESLint sprawdza JavaScript i TypeScript projektów, które go używają, wraz ze swoimi szybkimi poprawkami, a skróty Emmet pojawiają się w uzupełnianiu dla HTML, CSS i JSX. Dango instaluje oba na żądanie; **Ustawienia › Ogólne** naprawiają problemy ESLint przy zachowywaniu, jeśli chcesz.

## Testy

Panel Testy wyświetla testy folderu według pakietu i pliku wraz z ich stanem: `cargo test` lub cargo-nextest, Go, pytest, Vitest i Jest. Symbol przy każdym teście na marginesie uruchamia go, ⌥-kliknięcie uruchamia jego debugowanie, a nieudana asercja oznacza swój wiersz. Menu Debugowanie uruchamia test w punkcie wstawiania (⌃⌥⌘U), ponownie ostatnie testy (⌃⌥⌘G) lub wszystkie oraz debuguje test w punkcie wstawiania. Testy działają tylko w zaufanych folderach, ponieważ uruchamiają kod projektu; `cargo test` wyświetla listę swoich testów dopiero na prośbę, bo tworzenie listy buduje każdy program testowy.

## Debuger

Punkty przerwania, także z warunkami i jako punkty rejestrowania, praca krokowa, wątki z ich stosami wywołań, zmienne i obserwowane wyrażenia w inspektorze (⌥⌘I), wartości obok kodu, gdy program jest wstrzymany, oraz konsola debugowania, która oblicza wyrażenia we wstrzymanej ramce.

| Język | Adapter debugowania |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` z Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | debuger JavaScript, na Node.js |
| Python | debugpy |

Plik `.vscode/launch.json` projektu jest odczytywany. Menu Debugowanie używa skrótów z Xcode: ⌘\\ ustawia punkt przerwania, ⌘Y włącza lub wyłącza wszystkie, ⌃⌘Y kontynuuje, F6, F7 i F8 przechodzą nad, wchodzą i wychodzą, ⌘. zatrzymuje.

## Terminal

Terminale w kartach (⌃\`) ze środowiskiem powłoki logowania, rysowane przez Metal. ⌘K czyści ekran. Wklejony lub przeciągnięty obrazek trafia do agenta programistycznego działającego w terminalu albo wkleja ścieżkę zachowanej kopii. ⌘-kliknięcie otwiera łącza i nazwy plików. Serwer deweloperski uruchomiony z terminala można otworzyć w karcie przeglądarki.

## Przeglądarka

Karta przeglądarki z polem adresu (⌘L), wstecz i dalej (⌘[ i ⌘]), odświeżaniem (⌘R), wyszukiwaniem, pobieraniem z pytaniem i inspektorem WWW. `@browser` przekazuje stronę agentowi, oznaczoną jako tekst z zewnątrz.

## Git

Panel Git przygotowuje i odrzuca pliki lub pojedyncze fragmenty, tworzy commity, commituje i wypycha lub synchronizuje, przełącza i tworzy gałęzie, przechowuje stashe, rozwiązuje konflikty scalania i rysuje graf ostatnich commitów. Blame pokazuje, kto ostatnio zmienił wiersz pod kursorem, albo kolumnę dla całego pliku. Przycisk pisze opis commitu za pomocą AI; różnice wychodzą dopiero wtedy, gdy gitleaks nie znalazł w nich żadnego sekretu, a każdy commit również jest sprawdzany pod kątem sekretów.

## CI

GitHub Actions i GitLab CI, także GitHub Enterprise Server i samodzielnie hostowany GitLab:

- uruchomienia, joby i logi bieżącej gałęzi;
- ponowne uruchamianie nieudanych jobów, pojedynczego joba lub, na GitHubie, całego uruchomienia; uruchamianie ręcznych jobów lub zatwierdzanie oczekujących wdrożeń; uruchamianie nowego pipeline’u lub workflow, który można uruchomić ręcznie; anulowanie;
- otwarte pull requesty lub merge requesty i zgłoszenia, które otwierają się w karcie przeglądarki;
- log nieudanego joba przekazany agentowi, oznaczony jako tekst z zewnątrz.

Token pochodzi z pęku kluczy, z `gh` lub `glab`, ze środowiska powłoki logowania albo jest tym, który wpiszesz w **Ustawienia › CI**.

## Kontenery

Panel Kontenery pokazuje projekty Compose Dockera z ich usługami, pojedyncze kontenery, obrazy i woluminy oraz śledzi zdarzenia Dockera na bieżąco. Plik Compose z folderu pojawia się jako projekt, zanim jeszcze istnieje którykolwiek z jego kontenerów. Kontenery można uruchamiać, zatrzymywać i uruchamiać ponownie; ich logi i powłoka otwierają się w kartach terminala, a opublikowany port otwiera się w karcie przeglądarki. Usunięcie kontenera, obrazu lub woluminu oraz czyszczenie nieużywanych danych najpierw pytają o potwierdzenie. Polecenia up, down i pull z Compose działają tylko w zaufanych folderach, ponieważ Compose buduje i uruchamia to, co opisuje projekt. Menu panelu wybiera kontekst Dockera dla okna, nie zmieniając tego, którego używają twoje powłoki; te same polecenia są w menu Kontenery. Panel wymaga wiersza poleceń `docker`, Dockera 24.0.6 lub nowszego.

## Markdown

- **Podgląd.** Plik Markdown otwiera się z podglądem obok; **Widok › Pokaż/ukryj podgląd obok** (⇧⌘V) zamyka go i otwiera, a **Ustawienia › Ogólne** wyłączają automatyczny podgląd. Podgląd rysuje wzory, diagramy Mermaid, Graphviz i PlantUML oraz obrazki z folderu; obrazki z sieci ładują się tylko w zaufanym folderze albo gdy poprosisz o nie w podglądzie.
- **Menu Format:** pogrubienie (⌘B), kursywa (⌘I), przekreślenie (⇧⌘X), kod (⌥⇧⌘C), łącze (⇧⌘K), nagłówki 1–3 (⌥⌘1–⌥⌘3), listy punktowane i numerowane, zadania, cytaty i bloki kodu. Każde polecenie przełącza, na zaznaczeniu lub wierszach, i jest jednym krokiem cofania.
- **Wklejanie.** Tekst z Worda lub strony WWW trafia jako Markdown, a jego obrazki jako pliki obok dokumentu; ⌥⇧⌘V wkleja zwykły tekst.
- **Eksport.** **Plik › Eksportuj** zapisuje PDF, jeden plik HTML z obrazkami w środku albo dokument Word. Obrazki z sieci są dołączane tylko z zaufanego folderu.
- **Otwórz w Splat.** Jeśli [Splat](https://github.com/sventmueller/splat) jest zainstalowany, **Plik › Otwórz w Splat** i menu kontekstowe listy plików oraz kart otwierają tam dokument, po jego zachowaniu.

## Dyktowanie

Dyktuj do pola wiadomości karty AI lub do edytora: przyciskiem mikrofonu, **Edycja › Rozpocznij dyktowanie** (⌃⌥D) albo klawiszem fn (przytrzymaj, aby nagrywać, naciśnij dwa razy, aby nagrywać dalej). Rozpoznawanie mowy pochodzi z [Lowspeak](https://github.com/sventmueller/lowspeak); sam Lowspeak nie musi być zainstalowany.

- **Ustawienia › Dyktowanie** ustalają tryb i model (online z OpenAI lub Groq, lokalnie z Whisper lub Parakeet), język, końcowe przetwarzanie, mikrofon i Twoje własne słownictwo.
- **Modele lokalne** są pobierane z Hugging Face, gdy wybierzesz któryś, z postępem i możliwością anulowania. Trafiają do folderów, których używa Lowspeak, więc każda aplikacja korzysta z tego, co pobrała druga. Model lokalny zostawia nagranie na tym Macu.
- **Transkrypcja online** wymaga klucza do OpenAI lub Groq, który Dango przechowuje we własnej rzeczy w pęku kluczy.
- **Z Lowspeak.** Jeśli Lowspeak jest ustawiony inaczej niż domyślnie, karta Dyktowanie jednorazowo proponuje przejęcie jego ustawień, a na życzenie także kluczy; macOS pyta, zanim Dango odczyta któryś z nich (wybierz „Pozwól”, nie „Zawsze pozwalaj”).
- **Uprawnienia.** macOS pyta o mikrofon przy pierwszym dyktowaniu. Klawisz fn działa, gdy Dango jest aktywną aplikacją, a Lowspeak nie działa; wymaga uprawnienia dostępności, o które Dango prosi dopiero po pierwszym dyktowaniu, nigdy przy uruchomieniu.

## Zaufanie

Folder może sprawić, że programy uruchomią się, gdy tylko coś w nim wystartuje: hooki Git i ustawienia repozytorium, takie jak `core.fsmonitor`, serwery języka z `node_modules`, hooki agentów, `.mcp.json`. Gdy otwierasz folder po raz pierwszy, Dango pyta, czy mu ufasz.

- **Zaufany:** Git, serwery języka, debugery, agenty i CI działają dla tego folderu i wszystkiego, co w nim jest.
- **Ograniczony:** możesz czytać i edytować; znaczniki zmian w edytorze pochodzą z bezpośredniego odczytu `.git`. Nic, co konfiguruje folder, nie jest uruchamiane. Panele Git, CI i AI informują o tym i proponują zaufanie folderowi.

Tekst, który nie pochodzi od Ciebie, może zawierać polecenia mające sterować agentem (prompt injection). Wynik terminala (`@terminal`), strona WWW (`@browser`), log CI przekazany agentowi, plik spoza projektu i dłuższy tekst wklejony ze strony WWW trafiają więc do agenta oznaczone: etykieta mówi „Treść zewnętrzna”, a agent dostaje tekst z informacją, że to niezaufane dane, których poleceń nie wolno mu wykonywać. Strony WWW wbudowanego agenta przychodzą z tą samą informacją. Wklejony tekst uznaje się za pochodzący ze strony WWW, gdy przeglądarka zaznacza to w schowku, tak jak robią to Safari, Chrome i Firefox; tekst z innych miejsc oraz krótka fraza z kilku słów pozostają Twoje.

## Piaskownica

Samo Dango działa bez App Sandbox, ponieważ odziedziczyłby ją każdy uruchamiany przez nie program: terminale, Git, serwery języka i agenty nie mogłyby działać. Zamiast tego każdy wątek AI ma przełącznik piaskownicy, domyślnie wyłączony. To, co ogranicza, zależy od agenta, a podpowiedź przełącznika to opisuje:

| Agent | Z włączoną piaskownicą |
|---|---|
| Claude | polecenia powłoki działają we własnej piaskownicy Claude; jego narzędzia do plików i sieci nie |
| Codex | polecenia i zmiany zapisują tylko w projekcie, bez sieci |
| Wbudowany agent | pliki, żądania sieciowe i polecenia pozostają w projekcie i dozwolonych folderach, bez sieci (`sandbox-exec`) |
| Antigravity, OpenCode | brak piaskownicy, którą Dango mogłoby włączyć |

## Wygląd i dostępność

- **Tryb ciemny** podąża za systemem; **Zwiększ kontrast** i **Ogranicz ruch** są respektowane.
- **VoiceOver** nazywa pasek boczny, karty, kafelki osi czasu AI, wiersze Git i CI, debuger i tekst terminala. Punkty przerwania nigdy nie różnią się wyłącznie kolorem.
- **Klawiatura.** Każde polecenie ma skrót i każdy można zmienić w `keybindings.json`; ⌥⌘\` przechodzi do następnego panelu.
- **Od prawej do lewej.** W arabskim i hebrajskim okno jest odbijane lustrzanie, a widoki kodu pozostają od lewej do prawej.

Co Dango dodaje do AppKit, obszar po obszarze, opisuje [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Języki

Interfejs jest przetłumaczony na 35 języków: arabski, kataloński, chiński (uproszczony), chiński (tradycyjny), chorwacki, czeski, duński, holenderski, angielski, fiński, francuski, niemiecki, grecki, hebrajski, hindi, węgierski, indonezyjski, włoski, japoński, koreański, malajski, norweski, polski, portugalski (Brazylia), portugalski (Portugalia), rumuński, rosyjski, słowacki, słoweński, hiszpański, szwedzki, tajski, turecki, ukraiński i wietnamski.

Dango podąża za kolejnością języków w **Ustawienia systemowe › Ogólne › Język i region**; tam w sekcji **Aplikacje** możesz też wybrać język tylko dla Dango. Jeśli nie ma tłumaczenia na Twój język, używa angielskiego. Terminy są zgodne z jednym glosariuszem, [docs/glossary.md](../../../docs/glossary.md), i tłumaczeniami Apple, więc Dango czyta się jak reszta Twojego Maca.

## Skróty klawiszowe

| Czynność | Skrót |
|---|---|
| Otwórz folder, nowe okno, zamknij okno | ⌘O, ⇧⌘N, ⇧⌘W |
| Nowy plik, zachowaj, zachowaj jako | ⌘N, ⌘S, ⇧⌘S |
| Nowa karta, zamknij kartę, następna i poprzednia karta | ⌘T, ⌘W, ⇧⌘] i ⇧⌘[ |
| Szybko otwórz, szukaj w projekcie | ⌘P, ⇧⌘F |
| Pliki, Wyszukiwanie, Agenty, Git, Testy, Debugowanie, CI, Kontenery | ⌘1 do ⌘8 |
| Pokaż lub ukryj pasek boczny, następny panel | ⌃⌘S, ⌥⌘\` |
| Wstecz i dalej przez edytowane miejsca | ⌃- i ⌃⇧- |
| Przełącz komentarz, przejdź do wiersza | ⌘/, ⌃G |
| Następne wystąpienie, punkt wstawiania powyżej i poniżej | ⌘D, ⌃⇧↑ i ⌃⇧↓ |
| Zwiń, rozwiń, zwiń wszystko, rozwiń wszystko | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Zawijanie wierszy, inspektor | ⌥Z, ⌥⌘I |
| Uzupełnianie, informacje, przejdź do definicji | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Odwołania, hierarchia wywołań, zmień nazwę | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Symbole w pliku, w projekcie | ⇧⌘O, ⌥⌘O |
| Szybka poprawka, formatuj | ⌥⌘A, ⌥⇧F |
| Następny i poprzedni problem | ⌘' i ⇧⌘' |
| Podgląd Markdown, wklej jako zwykły tekst | ⇧⌘V, ⌥⇧⌘V |
| Pogrubienie, kursywa, przekreślenie, kod, łącze | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Nagłówek 1 do 3 | ⌥⌘1 do ⌥⌘3 |
| Nowy terminal, wyczyść terminal | ⌃\`, ⌘K |
| Fokus na polu wiadomości agenta, skomentuj plan | ⌘Esc, ⌥⌘K |
| Pole adresu, odśwież, wstecz, dalej (przeglądarka) | ⌘L, ⌘R, ⌘[, ⌘] |
| Rozpocznij lub zatrzymaj dyktowanie | ⌃⌥D |
| Punkt przerwania, włącz lub wyłącz wszystkie | ⌘\\, ⌘Y |
| Uruchom test w punkcie wstawiania, uruchom ponownie ostatnie testy | ⌃⌥⌘U, ⌃⌥⌘G |
| Kontynuuj, przejdź nad, wejdź, wyjdź, zatrzymaj | ⌃⌘Y, F6, F7, F8, ⌘. |
| Ustawienia | ⌘, |

`keybindings.json` w folderze danych aplikacji (zobacz [Gdzie są dane](#gdzie-są-dane)) zmienia każdy z nich.

## Prywatność i bezpieczeństwo

- **Bez telemetrii,** bez analityki, bez raportów o awariach wysyłanych gdziekolwiek. Błędy trafiają do dziennika systemowego (Konsola, pod nazwą aplikacji).
- **Sieć tylko dla tego, co uruchomisz:** agenty, których używasz, każdy z własną usługą; rejestr ACP i pobieranie agentów, serwerów języka, adapterów debugowania i modeli dyktowania, gdy o nie poprosisz; hosty CI, do których się logujesz; przewidywanie z Copilota, jeśli je wybrałeś; dostawcy wbudowanego agenta; transkrypcja online, jeśli ją wybrałeś; strony otwierane w karcie przeglądarki; i sprawdzanie uaktualnień, gdy już na nie pozwolisz.
- **Kod opuszcza Maca** tylko tam, gdzie wybrałeś do tego usługę: agenta, przewidywanie z Copilota, opis commitu napisany przez AI. Różnice dla opisu commitu wychodzą dopiero wtedy, gdy gitleaks nie znalazł w nich żadnego sekretu.
- **Klucze i tokeny** pozostają w pęku kluczy macOS, nigdy w pliku ustawień. Klucze innych aplikacji, takich jak Lowspeak, są odczytywane tylko za Twoją zgodą i tylko raz.
- **Przypięte pobrania.** Agenty, serwery języka i adaptery debugowania instalowane przez Dango są przypięte do przetestowanych wersji i sprawdzane według sum kontrolnych.
- **Podpisywanie kodu** z Hardened Runtime; uaktualnienia instalują się tylko z własnym podpisem aplikacji.

## Gdzie są dane

`<id>` to identyfikator pakietu, `local.agentide` dla wydania (`local.agentide.debug` dla kompilacji debugowania). Nie zawiera wyświetlanej nazwy, więc zmiana nazwy zachowuje wszystkie dane.

| Miejsce | Co |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (lista panelu AI), `agent-threads/` (rozmowy wbudowanego agenta), `layouts/` (karty dla każdego projektu), `recovery/` (niezachowane edytory po awarii), `checkpoints/` (dla folderów bez Git), `worktrees/`, `agents/` i `tools/` (zainstalowane agenty, serwery języka i adaptery debugowania), `task-output/` |
| `~/Library/Caches/<id>/` | wklejone obrazki, skrypty integracji z powłoką, rejestr ACP; można usunąć |
| `~/Library/Logs/<id>/dictation/` | pliki dziennika dyktowania |
| `~/Library/Application Support/Lowspeak` i `FluidAudio` | lokalne modele dyktowania, współdzielone z Lowspeak |
| `~/Library/WebKit/<id>/` | ciasteczka i dane witryn z karty przeglądarki |
| `<project>/.dango/settings.json` | własne ustawienia projektu |
| repozytorium Git projektu | punkty kontrolne jako commity w `refs/ide-checkpoints/`; zwykły push lub fetch nigdy ich nie zabiera |
| pęk kluczy, usługa `<id>` | klucze API wbudowanego agenta i dyktowania, tokeny CI |
| `~/.local/bin/dango` | polecenie terminala, jeśli je zainstalowałeś |

Claude przechowuje własne sesje w `~/.claude/projects`; Dango je odczytuje, aby wznowić sesję, ale ich nie zmienia.

## Znane ograniczenia

- **Brak App Sandbox** dla samej aplikacji, bo odziedziczyłyby ją terminale, Git, serwery języka i agenty; folder musi być zaufany, zanim uruchomi się cokolwiek, co konfiguruje. Antigravity i OpenCode nie mają piaskownicy, którą Dango mogłoby włączyć.
- **Liczby zużycia i podział kontekstu Claude** opierają się na częściach jego protokołu oznaczonych jako eksperymentalne i są ukrywane, gdy ich brakuje.
- **Przewijanie rozmowy do tyłu** działa tylko dla Claude; w pozostałych agentach do tyłu przewijany jest tylko kod.
- **Logowanie do CI przez przeglądarkę** jest wyłączone; użyj `gh`, `glab`, tokena lub środowiska.
- **Gemini CLI nie jest oferowany:** Google zakończył logowanie kontem osobistym i odsyła do Antigravity. Wcześniejsze wątki Gemini pozostają czytelne.

## Kompilacja ze źródeł

Potrzebujesz:

- macOS 14 lub nowszego z Xcode Command Line Tools (`xcode-select --install`); sam Xcode nie jest potrzebny.
- Rust w wersji podanej w `rust-toolchain.toml`. `rust` z Homebrew kompiluje dla procesora tego Maca; uniwersalna kompilacja wydania wymaga rustup z celami `aarch64-apple-darwin` i `x86_64-apple-darwin`.
- Kopii [splat](https://github.com/sventmueller/splat) i [lowspeak](https://github.com/sventmueller/lowspeak) obok tej (`../splat`, `../lowspeak`), obu na gałęzi `main`; workspace pobiera z nich crate’y po ścieżce.
- cmake dla każdej kompilacji z dyktowaniem (`make run`, `make build`, `make release`), która kompiluje whisper.cpp. `make check` kompiluje bez niego.

| Polecenie | Co robi |
|---|---|
| `make run` | kompiluje aplikację debugowania (`target/debug/Dango Debug.app`) i ją otwiera |
| `make build` | kompiluje aplikację wydania (`target/release/Dango.app`), podpisaną lokalnym certyfikatem, który pierwsza kompilacja tworzy w pęku kluczy logowania; pozostawia `Cargo.toml` bez zmian |
| `make lint` | formatowanie i clippy; ostrzeżenia kończą go błędem |
| `make check` | `lint`, a potem wszystkie testy; nie wymaga pęku kluczy, okna ani sieci |
| `make test` | `check`, a potem autotest `probe` i audyt dostępności, od lewej do prawej i od prawej do lewej, w prawdziwej aplikacji |
| `make perf` | budżety czasowe w kompilacji wydania; miarodajne tylko na Macu, który poza tym jest bezczynny |
| `make release` | próba wydania do `target/release-dry-run/`, podpisana lokalnym certyfikatem; niczego nigdzie nie wysyła |
| `make release-notarized` | `check`, a potem samo wydanie do `dist/`: podpis Developer ID, poświadczenie, kanał uaktualnień podpisany własnym kluczem aplikacji; pozostawia `Cargo.toml` bez zmian |
| `make deploy` | na czystej gałęzi `main`, z pieczęcią CI lub bez niej: poświadcza następną wersję, commituje `Cargo.toml`, przesyła pliki wydania, wypycha kod i tag wersji, a także zieloną pieczęć CI, jeśli istnieje, po czym sam publikuje wydanie |

Debugowanie i wydanie to oddzielne aplikacje z własnymi ustawieniami i danymi. Kompilację, testy, autotesty, wydanie i zasady, którymi kieruje się kod, opisuje [docs/development.md](../../../docs/development.md), a decyzje za nimi [docs/plan.md](../../../docs/plan.md). Informacje o wydaniu to [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), przetłumaczone obok każdego README w `assets/readme/<language>/`.

## Licencja

Dango jest udostępniane na licencji, do wyboru, albo

- Licencja Apache, wersja 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Licencja MIT ([LICENSE-MIT](../../../LICENSE-MIT))

według Twojego wyboru.

O ile wyraźnie nie zaznaczysz inaczej, każdy wkład celowo przekazany do włączenia do Dango przez Ciebie, w rozumieniu licencji Apache-2.0, będzie objęty podwójną licencją jak wyżej, bez dodatkowych warunków.
