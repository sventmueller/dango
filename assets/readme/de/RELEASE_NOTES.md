## AI-Tabs
- „Ausgabe zeigen“ eines Hintergrundbefehls öffnet einen Tab, der der Ausgabe folgt, solange der Befehl läuft, statt
  einer Kopie vom Moment des Klicks. Der Tab öffnet am Ende der Ausgabe und bleibt dort, während neue Zeilen kommen,
  bis du nach oben scrollst; scrollst du wieder ans Ende, folgt er erneut. „Ausgabe zeigen“ bei einem offenen Tab
  holt ihn nach vorn, am Ende.
  - Ein Befehl, der seine Ausgabe selbst in eine Datei schreibt, etwa `make > build.log 2>&1`, zeigt diese Datei,
    während sie wächst.
  - Ist der Befehl beendet, zeigt der Tab seine ganze Ausgabe; „Im Editor öffnen“ öffnet die Datei dahinter.
  - Ein Tab liest die Ausgabe nur, solange er vorn liegt.
