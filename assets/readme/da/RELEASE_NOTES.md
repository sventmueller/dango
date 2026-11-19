## AI-faner
- “Vis output” for en baggrundskommando åbner en fane, der følger outputtet, mens kommandoen kører, i stedet for en kopi
  fra det øjeblik. Fanen åbner ved slutningen af outputtet og bliver der, efterhånden som nye linjer kommer, indtil du
  ruller op; ruller du tilbage til slutningen, følger den igen. Er fanen allerede åben, bringer “Vis output” den frem
  ved slutningen.
  - En kommando, der selv skriver sit output til en fil, f.eks. `make > build.log 2>&1`, viser den fil, mens den vokser.
  - Når kommandoen er afsluttet, viser fanen hele dens output; “Åbn i editor” åbner filen bag fanen.
  - En fane læser kun outputtet, mens den er forrest.
