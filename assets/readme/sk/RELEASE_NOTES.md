## Terminál
- Terminál teraz pochádza z aplikácie Tapa, sesterskej terminálovej aplikácie k aplikácii Dango, so všetkým, čo prináša; čo vedel doterajší terminál, zostáva: vkladanie obrázkov agentom, vývojové servery v tabe prehliadača, ⌘-kliknutie do editora.
- **Zobraziť › Zobraziť/skryť vstupné pole** (⌘I, keď má terminál fokus) premení príkazový riadok na textové pole s priečinkom a vetvou Git nad ním. Príkazy sú zafarbené podľa svojich častí a ten, ktorý váš shell nedokáže spustiť, dostane červené prerušované podčiarknutie.
- Návrhy sa zobrazujú sivo už počas písania, vo vstupnom poli aj bez neho: z vašej histórie, zo špecifikácií pre viac než 700 nástrojov príkazového riadka a z projektu, ako sú vetvy gitu, skripty `package.json` a ciele make, just a cargo. → návrh prijme; Tab dopĺňa a pri viacerých kandidátoch otvorí ponuku, ktorú písanie filtruje. Sieť nikdy nepoužívajú.
- Bloky príkazov: ⌘↑ a ⌘↓ skáču medzi výzvami, ponuka **Upraviť** vyberie, skopíruje a uloží výstup príkazu a záložky označujú riadky. Keď je zapnuté **Označovať príkazy ako bloky**, pruh ukazuje každý blok, neúspešné príkazy sú označené a tlačidlá nad blokom zobrazujú jeho stav a trvanie.
- **Odoslať blok agentovi** (ponuka Upraviť a kontextová ponuka terminálu) otvorí AI tab s príkazovým riadkom a výstupom aktuálneho bloku priloženými ako Externý obsah.
- **Nájsť** (⌘F, keď má terminál fokus) prehľadáva celú históriu s regulárnymi výrazmi, rozlišovaním veľkosti písmen a celými slovami; posuvník označuje výsledky.
- Hlásenie programu (OSC 9 a OSC 777) a jeho zvonček dajú na tab terminálu červenú bodku, kým ho nevidíte, a keď je vpredu iná aplikácia, hlásenie ide do macOS. Priebeh, ktorý program hlási (OSC 9;4), sa zobrazí ako krúžok na jeho tabe a ako pruh na ikone v Docku.
- Dvojité kliknutie vyberie celú adresu URL, cestu alebo hash, ťahanie s ⌥ vyberie obdĺžnik, **Kopírovať s formátovaním** zachová farby a štýly a **Výber klávesnicou** posúva výber klávesmi Vimu.
- Viacriadkové vloženie, ktoré by sa hneď spustilo, sa najprv zobrazí a **Dango › Zabezpečená klávesnica** bráni iným aplikáciám čítať, čo píšete do terminálu.
- Odkazy iných druhov než webové adresy, `mailto:` a súbory terminál už neotvára.

## Nastavenia
- **Nastavenia › Terminál** prepína vstupné pole pre nové terminály, jeho klávesy Vim, návrhy, farebné zvýrazňovanie príkazov, podčiarkovanie neznámych príkazov, bloky príkazov, okamžité kopírovanie výberu, zobrazenie viacriadkového vloženia vopred a ⌥ ako meta kláves, ktorý sa sem presunul z tabu Všeobecné.
