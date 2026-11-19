## File IA
- „Afișați ieșirea” la o comandă în fundal deschide o filă care urmărește ieșirea cât timp comanda rulează, în loc de o
  copie a ei din acel moment. Fila se deschide la sfârșitul ieșirii și rămâne acolo pe măsură ce apar rânduri noi, până
  când derulați în sus; dacă derulați înapoi până la sfârșit, o urmărește din nou. „Afișați ieșirea” la o filă deja
  deschisă o aduce în față, la sfârșitul ieșirii.
  - O comandă care își scrie singură ieșirea într-un fișier, cum ar fi `make > build.log 2>&1`, afișează acel fișier pe
    măsură ce crește.
  - După ce comanda se termină, fila afișează întreaga ieșire; „Deschideți în editor” deschide fișierul din spatele ei.
  - O filă citește ieșirea doar cât timp se află în față.
