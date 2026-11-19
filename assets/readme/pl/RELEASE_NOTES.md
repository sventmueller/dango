## Pamięć
- Długie sesje agentów potrzebują znacznie mniej pamięci:
  - Otwarcie wcześniejszej sesji wczytuje jej transkrypcję wiersz po wierszu; sesja z transkrypcją o rozmiarze 100 MB
    zajmuje teraz w szczycie około 20 MB zamiast 65 MB.
  - Zakończone wywołanie narzędzia zachowuje tylko to, co pokazuje jego element na osi czasu: pierwsze 400 wierszy
    wyjścia, a surowego wejścia już nie, gdy tylko element pokaże swoje pola. „Dalsze wiersze: N” nadal liczy każdy
    wiersz.
  - Karta AI, która przez 30 sekund pozostaje niewidoczna, zwalnia swoje elementy, a gdy znów się pokaże, buduje te
    widoczne, w miejscu, w którym była.
  - Po zamknięciu karty lub otwarciu wcześniejszej sesji Dango oddaje zwolnioną pamięć systemowi macOS.
- Listy nie rosną już w pamięci podczas odświeżania. W macOS 27 każde odświeżenie listy z rozwijanymi wierszami
  zostawiało na stałe trójkąt rozwijania, a każde odświeżenie zostawiało trochę z każdego wiersza:
  - Listy CI, Kontenery, Software Factory, Testy, Debugowanie, Git i Wyszukiwanie oraz hierarchia wywołań rysują własne
    strzałki.
  - Niewidoczne listy nie są już odświeżane; nadrabiają to, gdy się pokażą.
  - Listy Sesje, CI, Software Factory i Kontenery zachowują wiersze, które się nie zmieniły, a Git odświeża tylko
    wiersze plików, których stan się zmienił.
- Panele zamkniętego okna są zwalniane razem z nim.

## Karty AI
- „Pokaż wyjście” polecenia w tle, które samo zapisuje swoje wyjście do pliku, na przykład `make > build.log 2>&1`,
  pokazuje ten plik, gdy CLI nie ma nic z wyjścia.
