## Arbeitsspeicher
- Lange Agenten-Sitzungen brauchen deutlich weniger Arbeitsspeicher:
  - Eine frühere Sitzung wird beim Öffnen Zeile für Zeile aus ihrem Transkript gelesen; eine Sitzung mit 100 MB
    Transkript braucht dabei höchstens etwa 20 MB statt 65 MB.
  - Ein beendeter Werkzeugaufruf behält nur, was seine Karte zeigt: die ersten 400 Zeilen seiner Ausgabe, und die
    rohe Eingabe nicht mehr, sobald die Karte ihre Felder zeigt. „N weitere Zeilen“ zählt weiterhin jede Zeile.
  - Ein AI-Tab, der 30 Sekunden nicht zu sehen ist, gibt seine Karten frei und baut beim Zeigen die sichtbaren neu
    auf, an derselben Stelle.
  - Nach dem Schließen eines Tabs und dem Öffnen einer früheren Sitzung gibt Dango den frei gewordenen Speicher an
    macOS zurück.
- Listen wachsen beim Aktualisieren nicht mehr im Speicher. Unter macOS 27 ließ jede Aktualisierung einer Liste mit
  aufklappbaren Zeilen ein Aufklapp-Dreieck dauerhaft zurück, und jede Aktualisierung ein wenig von jeder Zeile:
  - Die Listen von CI, Container, Software Factory, Tests, Debuggen, Git und Suche sowie die Aufrufhierarchie zeichnen
    ihre Pfeile selbst.
  - Listen, die nicht zu sehen sind, aktualisieren sich nicht mehr; sie holen es nach, sobald sie sich zeigen.
  - Die Listen Sitzungen, CI, Software Factory und Container behalten die Zeilen, die sich nicht geändert haben, und
    Git aktualisiert nur die Zeilen der Dateien, deren Zustand sich geändert hat.
- Die Panels eines geschlossenen Fensters werden mit ihm freigegeben.

## AI-Tabs
- „Ausgabe zeigen“ eines Hintergrundbefehls, der seine Ausgabe selbst in eine Datei schreibt, etwa
  `make > build.log 2>&1`, zeigt diese Datei, wenn die CLI nichts von der Ausgabe hat.
