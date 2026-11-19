## Karty AI
- „Pokaż wyjście” przy poleceniu w tle otwiera kartę, która śledzi wyjście, dopóki polecenie działa, zamiast kopii
  wyjścia z tamtej chwili. Karta otwiera się na końcu wyjścia i pozostaje tam, gdy dochodzą nowe wiersze, dopóki nie
  przewiniesz w górę; po przewinięciu z powrotem na koniec znów je śledzi. „Pokaż wyjście” przy karcie, która jest już
  otwarta, przenosi ją na wierzch i przewija na koniec.
  - Polecenie, które samo zapisuje swoje wyjście do pliku, na przykład `make > build.log 2>&1`, pokazuje ten plik, w
    miarę jak rośnie.
  - Po zakończeniu polecenia karta pokazuje całe jego wyjście; „Otwórz w edytorze” otwiera plik, który za nią stoi.
  - Karta wczytuje wyjście tylko wtedy, gdy jest na wierzchu.
