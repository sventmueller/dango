## Hastighed og hukommelse
- Alt, hvad en agent starter, i ethvert sprog, f.eks. builds, tests, shells og MCP-servere, kører i macOS’
  baggrundstilstand: med den laveste prioritet og med begrænset adgang til disk og netværk. Selve agenterne, med Claudes
  subagenter, kører med middel prioritet. Agenter under fuld belastning gør ikke længere Dango langsommere; det gælder
  også for agenterne i Software Factory.
- Gendannede vinduer starter kun det, der vises: en AI-fane bag andre starter sin CLI, når den kommer frem eller får en
  prompt, og browser-, webstedseksempel- og terminalfaner indlæser deres side eller starter deres shell, første gang de
  vises.
- En lang søgning i projektet tegner ikke længere alle sine resultater igen, mens den kører. Software Factory, CI-logs
  og panelet Tests laver mindre, mens de ikke er synlige; tests og startkonfigurationer indlæses, første gang deres
  panel vises, og skjulte paneler CI og Containere læser intet, før de vises.
- Lange sessioner bruger mindre hukommelse: en færdig diff beholder de linjer, dens kort viser, en besvaret anmodning om
  tilladelse giver slip på sit input, terminalens glyf-cache vokser ikke længere ubegrænset, og panelerne Arkiver og Git
  giver slip på rækker, der ikke længere findes.
- Selve programmet er omkring 40 % mindre.

## Vinduer og menuer
- Når en rude lukkes, lukkes dens faner ikke længere; de flyttes til ruden før, ellers til ruden efter. Kun ruden med et
  eksempel og den sidste rude lukkes sammen med deres faner.
- Menuen Oversigt viser panelerne i samme rækkefølge som symbolerne i indholdsoversigten.
- “Administrer agenter…” er fjernet fra menuen +; det findes stadig under “+ Ny session” i panelet Agenter.

## Rettelser
- “Medtag og udelad arkiver” i panelet Søgning viser ikke længere et forvildet bogstav ved siden af sin pil.
- Når du lukker kommentarvinduet til en specifikation, går appen ikke længere ned.
