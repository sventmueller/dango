## Terminal
- Das Terminal ist jetzt das von Tapa, Dangos Schwester-App fürs Terminal, mit allem, was es mitbringt; was Dangos Terminal bisher konnte, bleibt: Bilder an Agenten einsetzen, Dev-Server im Browser-Tab, ⌘-Klick in den Editor.
- **Darstellung › Eingabefeld ein-/ausblenden** (⌘I, solange ein Terminal den Fokus hat) macht die Befehlszeile zu einem Textfeld, darüber Ordner und Git-Branch. Befehle sind nach ihren Teilen eingefärbt, und einer, den die Shell nicht ausführen kann, bekommt eine rot gestrichelte Unterstreichung.
- Vorschläge erscheinen beim Tippen in Grau, im Feld und ohne es: aus dem Verlauf, aus Specs für mehr als 700 Kommandozeilen-Werkzeuge und aus dem Projekt, etwa Git-Branches, `package.json`-Skripte und Ziele von make, just und cargo. → übernimmt einen; Tab vervollständigt und öffnet bei mehreren Kandidaten ein Menü, das Tippen filtert. Ins Netz gehen sie nie.
- Befehlsblöcke: ⌘↑ und ⌘↓ springen von Prompt zu Prompt, **Bearbeiten** wählt die Ausgabe eines Befehls aus, kopiert und sichert sie, und Lesezeichen markieren Zeilen. Mit **Befehle als Blöcke markieren** zeigt ein Balken jeden Block, fehlgeschlagene Befehle sind markiert, und Knöpfe über einem Block zeigen Status und Dauer.
- **Block an Agent übergeben** (Menü Bearbeiten und Kontextmenü des Terminals) öffnet einen AI-Tab, an dem Befehlszeile und Ausgabe des aktuellen Blocks als Text von außen hängen.
- **Suchen** (⌘F, solange ein Terminal den Fokus hat) durchsucht den ganzen Verlauf mit regulären Ausdrücken, Groß-/Kleinschreibung und ganzen Wörtern; die Scrollleiste markiert die Treffer.
- Eine Mitteilung eines Programms (OSC 9 und OSC 777) und seine Glocke setzen den roten Punkt auf den Tab des Terminals, solange du es nicht siehst, und die Mitteilung geht an macOS, solange eine andere App vorn ist. Fortschritt, den ein Programm meldet (OSC 9;4), erscheint als Ring auf seinem Tab und als Balken auf dem Dock-Symbol.
- Ein Doppelklick wählt eine ganze URL, einen Pfad oder einen Hash, ⌥-Ziehen ein Rechteck, **Mit Formatierung kopieren** behält Farben und Stile, und **Auswahl mit der Tastatur** bewegt eine Auswahl mit Vims Tasten.
- Mehrzeiliges Einsetzen, das sofort ausgeführt würde, erscheint vorher, und **Dango › Sichere Tastatureingabe** hält andere Apps davon ab, mitzulesen, was du in ein Terminal tippst.
- Links anderer Art als Web-Adressen, `mailto:` und Dateien öffnet das Terminal nicht mehr.

## Einstellungen
- **Einstellungen › Terminal** schaltet das Eingabefeld für neue Terminals, seine Vim-Tasten, die Vorschläge, das Einfärben von Befehlen, das Unterstreichen unbekannter, Befehlsblöcke, das sofortige Kopieren der Auswahl, das Vorzeigen mehrzeiligen Einsetzens und die Wahltaste als Meta-Taste, die von Allgemein hierher gezogen ist.
