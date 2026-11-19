## AI taby
- „Zobraziť výstup“ pri príkaze na pozadí otvorí tab, ktorý sleduje výstup, kým príkaz beží, namiesto kópie výstupu z
  daného okamihu. Tab sa otvorí na konci výstupu a zostane tam aj pri nových riadkoch, kým sa neposuniete nahor; keď sa
  posuniete späť na koniec, opäť výstup sleduje. „Zobraziť výstup“ pri tabe, ktorý je už otvorený, ho presunie do
  popredia a zobrazí koniec výstupu.
  - Príkaz, ktorý svoj výstup sám zapisuje do súboru, napríklad `make > build.log 2>&1`, zobrazuje tento súbor, ako
    postupne rastie.
  - Keď príkaz skončí, tab zobrazí celý jeho výstup; „Otvoriť v editore“ otvorí súbor, z ktorého tab číta.
  - Tab číta výstup, iba kým je v popredí.
