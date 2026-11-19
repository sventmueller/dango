## Karty AI
- Karta AI, która przez trzy minuty pozostaje niewidoczna, zwalnia swój proces Claude, dopóki sesja jest bezczynna.
  Rozmowa i wpisany tekst zostają w karcie, a następny prompt wznawia sesję. Karta zachowuje swój proces, dopóki Claude
  pracuje, prompt czeka, pytanie jest otwarte, subagent, zadanie w tle lub polecenie nadal działa albo Remote Control
  jest włączony.
- Zakończenie agenta kończy też polecenia, które uruchomił we własnej grupie procesów lub sesji; wcześniej mogły działać
  dłużej niż on.
- Oś czasu dopisuje nowe elementy na końcu zamiast ponownie indeksować wszystkie wcześniejsze wiersze.

## Szybkość
- Przywrócone okna otwierają się szybciej: karta pliku bez niezachowanych zmian wczytuje swój plik dopiero wtedy, gdy
  znajdzie się na wierzchu.
- Wyszukiwanie w projekcie, którego panel jest ukryty, wstrzymuje się i wznawia, gdy panel się pokaże.
- Znaczniki zmian Git w edytorze wykonują dla każdego pliku najwyżej jedno porównanie naraz; edycje wprowadzone w tym
  czasie są zbierane w jedno kolejne porównanie, a tekst pliku z commitu jest wczytywany ponownie tylko wtedy, gdy
  commit się zmienił.
- Minimapa pliku ze zwiniętym kodem przechowuje jeden element na każdy rozwinięty fragment zamiast jednego na wiersz.

## Wyszukiwanie
- Strzałka filtrów jest teraz wyrównana ze strzałką Zastąp nad nią.
