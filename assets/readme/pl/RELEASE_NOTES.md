## Szybkość i pamięć
- Wszystko, co uruchamia agent, w dowolnym języku, na przykład kompilacje, testy, powłoki i serwery MCP, działa w stanie
  tła macOS: z najniższym priorytetem i z ograniczonym dostępem do dysku i sieci. Same agenty, wraz z subagentami
  Claude, działają ze średnim priorytetem. Agenty pod pełnym obciążeniem nie spowalniają już Dango; dotyczy to także
  agentów Software Factory.
- Przywrócone okna uruchamiają tylko to, co widać: karta AI schowana za innymi uruchamia swoje CLI, gdy przejdzie na
  wierzch lub dostanie prompt, a karty przeglądarki, podglądu strony i terminala wczytują swoją stronę lub uruchamiają
  powłokę, gdy pokażą się po raz pierwszy.
- Długie wyszukiwanie w projekcie nie rysuje już wszystkich wyników od nowa, dopóki trwa. Software Factory, logi CI i
  panel Testy robią mniej, gdy ich nie widać; testy i konfiguracje debugowania są wczytywane, gdy ich panel pokaże się
  po raz pierwszy, a ukryte panele CI i Kontenery nie wczytują niczego, dopóki się nie pokażą.
- Długie sesje potrzebują mniej pamięci: ukończony diff zachowuje tylko wiersze widoczne na jego kafelku, prośba o
  uprawnienie, na którą odpowiedziano, zwalnia swoje dane wejściowe, pamięć podręczna glifów terminala nie rośnie już
  bez ograniczeń, a panele Pliki i Git zwalniają wiersze, których już nie ma.
- Sam program jest o około 40% mniejszy.

## Okna i menu
- Zamknięcie okienka przenosi jego karty do okienka przed nim, a jeśli go nie ma, do okienka po nim, zamiast je zamykać.
  Tylko okienko podglądu i ostatnie okienko zamykają się razem ze swoimi kartami.
- Menu Widok wymienia panele w kolejności symboli na pasku bocznym.
- „Zarządzaj agentami…” zniknęło z menu +; pozostaje pod „+ Nowa sesja” w panelu Agenty.

## Poprawki
- „Uwzględnij i wyklucz pliki” w panelu wyszukiwania nie pokazuje już zbłąkanej litery obok swojej strzałki.
- Zamknięcie dymka z komentarzami do specyfikacji nie powoduje już awarii.
