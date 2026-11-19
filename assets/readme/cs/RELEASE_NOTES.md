## Panely AI
- „Zobrazit výstup“ u příkazu na pozadí otevře panel, který výstup sleduje po celou dobu běhu příkazu, místo kopie z
  okamžiku kliknutí. Panel se otevře na konci výstupu a zůstává tam, zatímco přibývají nové řádky, dokud neposunete
  zobrazení nahoru; když ho posunete zpět na konec, začne výstup opět sledovat. Je-li panel už otevřený, „Zobrazit
  výstup“ ho přenese do popředí, na konec výstupu.
  - Příkaz, který svůj výstup sám zapisuje do souboru, například `make > build.log 2>&1`, zobrazí tento soubor tak, jak
    postupně roste.
  - Jakmile příkaz skončí, panel zobrazí celý jeho výstup; „Otevřít v editoru“ otevře soubor, ze kterého panel čte.
  - Panel čte výstup, jen dokud je v popředí.
