## Panely AI
- Panel AI, který tři minuty není vidět, ukončí svůj proces Claude, dokud je relace nečinná. Konverzace i rozepsaný text
  v panelu zůstanou a další prompt relaci obnoví. Panel si svůj proces ponechá, dokud Claude pracuje, čeká prompt, je
  otevřený dotaz, ještě běží subagent, úloha na pozadí nebo příkaz, nebo je zapnutý Remote Control.
- Ukončení agenta ukončí i příkazy, které spustil ve vlastní skupině procesů nebo relaci a které ho dříve mohly přežít.
- Časová osa přidává nové položky na konec, místo aby znovu indexovala všechny dřívější řádky.

## Rychlost
- Obnovená okna se otevírají rychleji: panel souboru bez neuložených změn načte svůj soubor, až když se dostane do
  popředí.
- Hledání v projektu, jehož podokno je skryté, se pozastaví a pokračuje, jakmile se podokno zobrazí.
- Značky změn Git v editoru provádějí u každého souboru nejvýš jedno porovnání najednou; úpravy provedené mezitím se
  sloučí do jednoho následného porovnání a text souboru z commitu se znovu načte, jen když se commit změnil.
- Minimapa souboru se sbaleným kódem uchovává jednu položku na každý rozbalený úsek místo jedné na každý řádek.

## Hledání
- Šipka filtrů je zarovnaná se šipkou Nahradit nad ní.
