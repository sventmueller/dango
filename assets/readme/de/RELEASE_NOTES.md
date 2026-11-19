## Geschwindigkeit und Arbeitsspeicher
- Alles, was ein Agent startet, in jeder Sprache, etwa Builds, Tests, Shells und MCP-Server, läuft im
  Hintergrund-Zustand von macOS: mit niedrigster Priorität und gedrosseltem Platten- und Netzzugriff. Die Agenten
  selbst, mit den Subagenten von Claude, laufen mit mittlerer Priorität. Agenten unter Volllast verlangsamen Dango
  nicht mehr; das gilt auch für die Agenten der Software Factory.
- Wiederhergestellte Fenster starten nur, was sich zeigt: Ein AI-Tab hinter anderen startet seine CLI, wenn er nach
  vorn kommt oder einen Prompt bekommt, und Browser-, Seitenvorschau- und Terminal-Tabs laden ihre Seite oder starten
  ihre Shell, wenn sie sich zum ersten Mal zeigen.
- Eine lange Projektsuche zeichnet ihre Treffer nicht mehr ständig neu, während sie läuft. Software Factory, CI-Logs
  und das Tests-Panel tun weniger, solange sie nicht zu sehen sind; Tests und Debug-Konfigurationen werden gelesen,
  wenn ihr Panel sich zum ersten Mal zeigt, und verborgene CI- und Container-Panels lesen nichts, bis sie sich zeigen.
- Lange Sitzungen brauchen weniger Arbeitsspeicher: Ein fertiger Diff behält die Zeilen, die seine Karte zeigt, eine
  beantwortete Freigabeanfrage gibt ihre Eingabe frei, der Glyphen-Speicher des Terminals wächst nicht mehr
  unbegrenzt, und die Panels Dateien und Git geben Zeilen frei, die es nicht mehr gibt.
- Das Programm selbst ist rund 40 % kleiner.

## Fenster und Menüs
- Ein geschlossener Bereich schließt seine Tabs nicht mehr, sie wandern in den Bereich davor, sonst in den danach.
  Nur der Bereich einer Vorschau und der letzte Bereich schließen mit ihren Tabs.
- Das Menü Darstellung nennt die Panels in der Reihenfolge der Symbole der Seitenleiste.
- „Agenten verwalten …“ steht nicht mehr im +-Menü; es bleibt unter „+ Neue Sitzung“ im Panel Agenten.

## Fehlerbehebungen
- „Dateien ein- und ausschließen“ im Suchpanel zeigt keinen verirrten Buchstaben mehr neben seinem Pfeil.
- Das Schließen des Kommentar-Popovers einer Spezifikation führt nicht mehr zum Absturz.
