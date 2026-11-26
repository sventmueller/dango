## Terminal
- Terminal pochodzi teraz z aplikacji Tapa, siostrzanej aplikacji terminalowej Dango, ze wszystkim, co wnosi; to, co terminal Dango potrafił dotąd, zostaje: wklejanie obrazków do agentów, serwery deweloperskie w karcie przeglądarki, ⌘-kliknięcie do edytora.
- **Widok › Pokaż/ukryj pole wprowadzania** (⌘I, gdy terminal ma fokus) zamienia wiersz poleceń w pole tekstowe z folderem i gałęzią Git nad nim. Polecenia są kolorowane według swoich części, a takie, którego Twoja powłoka nie może uruchomić, dostaje czerwone przerywane podkreślenie.
- Propozycje pojawiają się na szaro już podczas pisania, w polu wprowadzania i bez niego: z Twojej historii, ze specyfikacji ponad 700 narzędzi wiersza poleceń oraz z projektu, takie jak gałęzie git, skrypty `package.json` oraz cele make, just i cargo. → przyjmuje propozycję; Tab uzupełnia, a przy kilku kandydatach otwiera menu, które filtruje się podczas pisania. Nigdy nie korzystają z sieci.
- Bloki poleceń: ⌘↑ i ⌘↓ przeskakują między znakami zachęty, w menu **Edycja** można zaznaczyć, skopiować i zachować wynik polecenia, a zakładki oznaczają wiersze. Gdy włączone jest **Oznaczaj polecenia jako bloki**, pasek pokazuje każdy blok, nieudane polecenia są oznaczane, a przyciski nad blokiem pokazują jego stan i czas trwania.
- **Wyślij blok do agenta** (menu Edycja i menu kontekstowe terminala) otwiera kartę AI z wierszem poleceń i wynikiem bieżącego bloku dołączonymi jako Treść zewnętrzna.
- **Znajdź** (⌘F, gdy terminal ma fokus) przeszukuje całą historię z wyrażeniami regularnymi, uwzględnianiem wielkości liter i całymi słowami; pasek przewijania oznacza wyniki.
- Powiadomienie programu (OSC 9 i OSC 777) i jego dzwonek umieszczają czerwoną kropkę na karcie terminala, gdy go nie widzisz, a gdy z przodu jest inna aplikacja, powiadomienie trafia do macOS. Postęp zgłaszany przez program (OSC 9;4) widać jako pierścień na jego karcie i jako pasek na ikonie w Docku.
- Dwukrotne kliknięcie zaznacza w całości URL, ścieżkę lub hash, przeciąganie z ⌥ zaznacza prostokąt, **Kopiuj z formatowaniem** zachowuje kolory i style, a **Zaznaczanie klawiaturą** przesuwa zaznaczenie klawiszami Vima.
- Wklejany tekst z wieloma wierszami, który od razu by się wykonał, pojawia się najpierw, a **Dango › Bezpieczne wprowadzanie z klawiatury** nie pozwala innym aplikacjom odczytywać tego, co piszesz w terminalu.
- Łącza innych rodzajów niż adresy WWW, `mailto:` i pliki nie otwierają się już z terminala.

## Ustawienia
- **Ustawienia › Terminal** przełącza pole wprowadzania dla nowych terminali, jego klawisze Vim, propozycje, kolorowanie poleceń, podkreślanie nieznanych poleceń, bloki poleceń, kopiowanie zaznaczenia od razu, pokazywanie najpierw wklejania wielu wierszy oraz ⌥ jako metaklawisz, przeniesiony tu z karty Ogólne.
