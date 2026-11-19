## AI-flikar
- En AI-flik som har varit utom synhåll i tre minuter släpper sin Claude-process medan sessionen är inaktiv.
  Konversationen och det du har skrivit finns kvar i fliken, och nästa prompt återupptar sessionen. En flik behåller sin
  process medan Claude arbetar, en prompt väntar, en fråga är öppen, en underagent, en bakgrundsuppgift eller ett
  kommando fortfarande körs eller Remote Control är på.
- Att avsluta en agent avslutar också de kommandon som den startade i en egen processgrupp eller session och som
  tidigare kunde leva kvar efter den.
- Tidslinjen lägger till nya poster i slutet i stället för att indexera om alla tidigare rader.

## Hastighet
- Återställda fönster öppnas snabbare: en filflik utan osparade ändringar läser in sin fil först när den hamnar längst
  fram.
- En sökning i projektet vars panel är dold pausas och fortsätter när panelen visas.
- Redigerarens Git-ändringsmarkeringar kör högst en jämförelse åt gången per fil; ändringar under tiden slås ihop till
  en enda uppföljande jämförelse, och filens incheckade text läses bara in igen när committen har ändrats.
- Minikartan för en fil med ihopfälld kod behåller en post per utfälld del i stället för en per rad.

## Sök
- Filtrens pil är i linje med pilen för Ersätt ovanför.
