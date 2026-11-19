## Rýchlosť a pamäť
- Všetko, čo agent spustí, v akomkoľvek jazyku, napríklad buildy, testy, shelly a MCP servery, beží v stave pozadia
  macOS: s najnižšou prioritou a s obmedzeným prístupom na disk a do siete. Samotní agenti vrátane subagentov, ktorých
  spúšťa Claude, bežia so strednou prioritou. Agenti pri plnej záťaži už Dango nespomaľujú; platí to aj pre agentov
  Software Factory.
- Obnovené okná spúšťajú len to, čo je vidieť: AI tab za inými tabmi spustí svoje CLI, až keď príde do popredia alebo
  dostane prompt, a taby prehliadača, náhľadu webovej stránky a terminálu načítajú svoju stránku alebo spustia svoj
  shell, až keď sa prvýkrát zobrazia.
- Dlhé vyhľadávanie v projekte už počas behu neprekresľuje znova všetky svoje výsledky. Software Factory, logy CI a
  panel Testy robia menej, kým nie sú vidieť; testy a konfigurácie ladenia sa načítajú, až keď sa ich panel prvýkrát
  zobrazí, a skryté panely CI a Kontajnery nenačítajú nič, kým sa nezobrazia.
- Dlhé relácie potrebujú menej pamäte: karta s hotovými rozdielmi si ponechá len riadky, ktoré zobrazuje, zodpovedaná
  žiadosť o povolenie uvoľní svoj vstup, vyrovnávacia pamäť glyfov terminálu už nerastie bez obmedzenia a panely Súbory
  a Git uvoľňujú riadky, ktoré už neexistujú.
- Samotný program je približne o 40 % menší.

## Okná a ponuky
- Zatvorenie oblasti presunie jej taby do oblasti pred ňou, inak do oblasti za ňou, namiesto toho, aby ich zatvorilo.
  Spolu so svojimi tabmi sa zatvorí len oblasť náhľadu a posledná oblasť.
- Ponuka Zobraziť uvádza panely v poradí ikon na postrannom paneli.
- „Spravovať agentov…“ už nie je v ponuke +; zostáva pod „+ Nová relácia“ v paneli Agenti.

## Opravy
- „Zahrnúť a vylúčiť súbory“ v paneli Vyhľadávanie už vedľa svojej šípky nezobrazuje zatúlané písmeno.
- Zatvorenie okna s komentármi k špecifikácii už nespôsobí pád aplikácie.
