## AI-flikar
- ”Visa utdata” för ett bakgrundskommando öppnar en flik som följer utdata medan kommandot körs, i stället för en kopia
  av utdata från just det ögonblicket. Fliken öppnas vid slutet av utdata och stannar där medan nya rader kommer, tills
  du rullar uppåt; rullar du tillbaka till slutet följer den med igen. ”Visa utdata” för en flik som redan är öppen
  lägger den längst fram och visar slutet.
  - Ett kommando som självt skriver sina utdata till en fil, till exempel `make > build.log 2>&1`, visar den filen medan
    den växer.
  - När kommandot har avslutats visar fliken alla dess utdata; ”Öppna i redigeraren” öppnar filen som fliken läser från.
  - En flik läser utdata bara medan den ligger längst fram.
