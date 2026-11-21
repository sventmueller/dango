## Geschwindigkeit
- Was ein Agent startet, etwa Builds, Tests und Shells, läuft mit niedrigster Priorität statt im
  Hintergrund-Zustand von macOS. Dango kommt weiter zuerst und die Agenten vor dem, was sie starten, aber ihre Builds
  nutzen wieder die Performance-Kerne und die volle Geschwindigkeit der Platte; der Hintergrund-Zustand hatte ihren
  Durchsatz selbst auf einem sonst untätigen Mac auf etwa ein Zehntel gesenkt.

## Agenten
- Der Punkt eines AI-Tabs bleibt grün, solange ein Hintergrundbefehl seiner Sitzung noch läuft, und der Hinweis, dass
  eine Sitzung fertig ist, kommt erst, wenn auch ihre Hintergrundarbeit beendet ist.
