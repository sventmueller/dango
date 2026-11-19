## Hastighet og minne
- Alt en agent starter, uansett språk, for eksempel bygg, tester, skall og MCP-servere, kjører i bakgrunnstilstanden til
  macOS: med lavest prioritet og med strupet disk- og nettverkstilgang. Selve agentene, med underagentene til Claude,
  kjører med middels prioritet. Agenter under full belastning gjør ikke lenger Dango tregere; det gjelder også agentene
  i Software Factory.
- Gjenopprettede vinduer starter bare det som vises: en KI-fane bak andre starter CLI-en sin når den kommer frem eller
  får en prompt, og nettleser-, nettstedforhåndsvisnings- og terminalfaner laster inn siden sin eller starter skallet
  sitt første gang de vises.
- Et langt prosjektsøk tegner ikke lenger alle treffene sine på nytt mens det pågår. Software Factory, CI-logger og
  panelet Tester gjør mindre mens de ikke er synlige; tester og feilsøkingskonfigurasjoner leses første gang panelet
  deres vises, og panelene CI og Containere leser ingenting mens de er skjult, før de vises.
- Lange økter trenger mindre minne: en ferdig diff beholder bare linjene kortet dens viser, en besvart
  tillatelsesforespørsel slipper inndataene sine, glyfbufferen i terminalen vokser ikke lenger uten grense, og panelene
  Filer og Git slipper rader som er borte.
- Selve programmet er rundt 40 % mindre.

## Vinduer og menyer
- Når du lukker en rute, flyttes fanene dens til ruten foran, ellers til ruten etter, i stedet for å lukkes. Bare ruten
  til en forhåndsvisning og den siste ruten lukkes sammen med fanene sine.
- Menyen Vis viser panelene i samme rekkefølge som symbolene i sidepanelet.
- «Administrer agenter…» er fjernet fra +-menyen; den blir værende under «+ Ny økt» i panelet Agenter.

## Rettelser
- «Inkluder og ekskluder filer» i søkepanelet viser ikke lenger en bortkommen bokstav ved siden av pilen.
- Å lukke kommentarpopoveren til en spesifikasjon fører ikke lenger til krasj.
