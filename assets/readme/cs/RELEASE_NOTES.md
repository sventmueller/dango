## Terminál
- Terminál teď pochází z aplikace Tapa, sesterské terminálové aplikace Danga, se vším, co přináší; co terminál Danga uměl dosud, zůstává: vkládání obrázků agentům, vývojové servery v panelu prohlížeče, ⌘-kliknutí do editoru.
- **Zobrazení › Zobrazit/skrýt vstupní pole** (⌘I, když má terminál fokus) promění příkazový řádek v textové pole se složkou a větví Git nad ním. Příkazy jsou obarvené podle svých částí a ten, který váš shell nemůže spustit, dostane červené čárkované podtržení.
- Návrhy se zobrazují šedě už při psaní, ve vstupním poli i bez něj: z vaší historie, ze specifikací pro více než 700 nástrojů příkazového řádku a z projektu, jako jsou větve gitu, skripty `package.json` a cíle make, just a cargo. → návrh přijme; Tab doplňuje a při více kandidátech otevře nabídku, kterou psaní filtruje. Síť nikdy nepoužívají.
- Bloky příkazů: ⌘↑ a ⌘↓ skáčou mezi výzvami, nabídka **Úpravy** vybere, zkopíruje a uloží výstup příkazu a záložky označují řádky. Když je zapnuté **Označovat příkazy jako bloky**, pruh ukazuje každý blok, neúspěšné příkazy jsou označené a tlačítka nad blokem zobrazují jeho stav a dobu trvání.
- **Odeslat blok agentovi** (nabídka Úpravy a místní nabídka terminálu) otevře panel AI s příkazovým řádkem a výstupem aktuálního bloku připojenými jako Cizí obsah.
- **Hledat** (⌘F, když má terminál fokus) prohledává celou historii s regulárními výrazy, rozlišováním velikosti písmen a celými slovy; posuvník označuje výsledky.
- Oznámení programu (OSC 9 a OSC 777) a jeho zvonek dají na panel terminálu červenou tečku, dokud ho nevidíte, a když je vepředu jiná aplikace, jde oznámení do macOS. Průběh, který program hlásí (OSC 9;4), se zobrazí jako kroužek na jeho panelu a jako pruh na ikoně v Docku.
- Dvojité kliknutí vybere celou adresu URL, cestu nebo hash, tažení s ⌥ vybere obdélník, **Kopírovat s formátováním** zachová barvy a styly a **Výběr klávesnicí** posouvá výběr klávesami Vimu.
- Víceřádkové vložení, které by se hned spustilo, se nejprve zobrazí a **Dango › Zabezpečit vstup z klávesnice** brání jiným aplikacím číst, co píšete do terminálu.
- Odkazy jiných druhů než webové adresy, `mailto:` a soubory se z terminálu už neotevírají.

## Nastavení
- **Nastavení › Terminál** přepíná vstupní pole pro nové terminály, jeho klávesy Vim, návrhy, obarvování příkazů, podtrhávání neznámých příkazů, bloky příkazů, okamžité kopírování výběru, zobrazení víceřádkového vložení předem a ⌥ jako meta klávesu, která se sem přesunula z panelu Obecné.
