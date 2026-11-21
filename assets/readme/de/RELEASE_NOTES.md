## Sicherheit
- Links in Antworten, Plänen und Berichten eines Agenten, in der Markdown-Vorschau und in PDFs öffnen nicht mehr mit
  einem Klick jede beliebige App. Webadressen öffnen in einem Browser-Tab, Dateien im Projekt in einem Editor-Tab;
  alles andere, etwa eine Datei anderswo, eine Netzwerkfreigabe oder der Link einer anderen App, fragt vorher und
  zeigt, wohin es führt.
- Anmelde- und Remote-Control-Adressen, die ein Agent oder ein MCP-Server übergibt, öffnen nur noch, wenn es
  Webadressen sind.
- Ordnern, die weit mehr als ein Projekt enthalten, etwa dem Benutzerordner, Schreibtisch, Dokumenten, Downloads oder
  der Wurzel eines Volumes, lässt sich nicht mehr als Ganzes vertrauen; vertraue ihnen Projekt für Projekt. Die
  Vertrauensfrage sagt jetzt, dass sie auch für die Ordner darin gilt.
- CI-Tokens und Anmeldungen gehen nie mehr über unverschlüsseltes http an einen anderen Server als diesen Mac; ein
  Repository, dessen Remote mit `http://` beginnt, wird über https gefragt.
- Ohne gitleaks warten Vorhersagen, bevor Code an einen Anbieter außerhalb dieses Macs geht, bis du ungeprüft senden
  wählst. Die eigenen gitleaks-Einstellungen eines Projekts schalten die Prüfung nicht mehr ab.
- Der eigene Agent lässt sich nicht mehr über einen Link, der nach der Freigabe eines Schreibvorgangs auftaucht, dazu
  bringen, außerhalb des Projekts zu schreiben, und Dateien mit weiteren harten Links bekommen eine eigene Datei,
  statt durch sie hindurch beschrieben zu werden.
- Die Vervollständigungen eines Language Servers können den Editor nicht mehr aufhängen, und sein Umbenennen und
  Löschen einer verlinkten Datei trifft den Link, nicht sein Ziel.
- Bilder, die du an einen Prompt hängst, dekodiert der abgeschottete Dekodierdienst, wie überall sonst.
- Ein Programm im Terminal kann die Zwischenablage nur ersetzen, solange sein Terminal vorn ist, und das Terminal
  sagt es.
- rust-analyzer installiert sich aus einem Release, das samt Prüfsumme festgelegt ist, statt aus dem jeweils neuesten.

## Suche und Navigation
- Schnell öffnen sagt, wenn keine Datei passt, und zeigt, ob es Dateien oder die Symbole des Projekts sucht.
- Die Symbole des Projekts umfassen TypeScript und JavaScript, auch ohne dass eine ihrer Dateien offen ist. Genaue
  Treffer aus dem Projekt und in der Sprache, in der du arbeitest, kommen zuerst; Symbole aus Bibliotheken und
  installierten Paketen sind gekennzeichnet.
- Darstellung › Suche setzt den Fokus ins Suchfeld.

## Editor
- Der Editor behält den Fokus, wenn eine Markdown-Datei mit ihrer Vorschau daneben öffnet, sodass das Menü Format
  sofort funktioniert. Editor-Befehle wirken auch, während der Fokus in der Seitenleiste liegt.
- Ein Vorschlag am Zeilenende oder in einem Kasten darunter sagt, dass Tab ihn übernimmt.
- Gleichnamige Tabs in der Tab-Übersicht sind unterscheidbar, etwa eine Datei und ihre Vorschau im Browser.
- Das Menü Darstellung bietet nicht mehr das „Tableiste ausblenden“ von macOS an, das Dangos Tabs nicht nutzen.

## Sprachen und Debuggen
- Der JSON-Server prüft jetzt Dateien, und Server, die ihre Diagnosen erst nach dem Start anmelden, etwa basedpyright
  und sourcekit-lsp, zeigen sie.
- Wo pip die Installation in Python verweigert, wie bei dem von Homebrew, bietet der Debugger an, debugpy in eine
  virtuelle Umgebung des Projekts zu installieren.
