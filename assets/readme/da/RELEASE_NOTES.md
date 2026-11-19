## Hukommelse
- Lange agentsessioner bruger langt mindre hukommelse:
  - Når en tidligere session åbnes, læses dens udskrift én linje ad gangen; en session med en udskrift på 100 MB topper
    nu ved omkring 20 MB i stedet for 65 MB.
  - Et afsluttet værktøjskald beholder kun det, dets kort viser: de første 400 linjer af outputtet, og ikke længere det
    rå input, når kortet viser sine felter. “N linjer mere” tæller stadig hver linje.
  - En AI-fane, der er ude af syne i 30 sekunder, slipper sine kort, og når den vises igen, bygger den dem, der er
    synlige, på det sted, hvor den var.
  - Når en fane lukkes, eller en tidligere session åbnes, giver Dango den frigjorte hukommelse tilbage til macOS.
- Lister vokser ikke længere i hukommelsen, når de opdateres. På macOS 27 efterlod hver opdatering af en liste med
  rækker, der kan udvides, en udvidelsestrekant for altid, og hver opdatering efterlod lidt af hver række:
  - Listerne CI, Containere, Software Factory, Tests, Fejlfind, Git og Søgning samt kaldhierarkiet tegner deres egne
    pile.
  - Lister, der ikke er synlige, opdateres ikke længere; de indhenter det, når de vises.
  - Listerne Sessioner, CI, Software Factory og Containere beholder de rækker, der ikke er ændret, og Git opdaterer kun
    rækkerne for filer, hvis tilstand er ændret.
- Panelerne i et lukket vindue frigives sammen med det.

## AI-faner
- “Vis output” for en baggrundskommando, der selv skriver sit output til en fil, f.eks. `make > build.log 2>&1`, viser
  den fil, når CLI ikke har noget af outputtet.
