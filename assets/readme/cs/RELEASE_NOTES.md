## Rychlost a paměť
- Vše, co agent spustí, v jakémkoli jazyce, například buildy, testy, shelly a servery MCP, běží ve stavu na pozadí
  systému macOS: s nejnižší prioritou a s omezeným přístupem na disk a do sítě. Samotní agenti včetně subagentů Claude
  běží se střední prioritou. Agenti pod plnou zátěží už Dango nezpomalují; to platí i pro agenty Software Factory.
- Obnovená okna spouštějí jen to, co je vidět: panel AI, který leží za jinými panely, spustí své CLI, až přejde do
  popředí nebo obdrží prompt, a panely prohlížeče, náhledu webu a terminálu načtou svou stránku nebo spustí svůj shell,
  až se poprvé zobrazí.
- Dlouhé hledání v projektu už během běhu znovu nepřekresluje všechny své výsledky. Software Factory, logy CI a podokno
  Testy toho dělají méně, dokud nejsou vidět; testy a konfigurace spuštění se načtou, až se jejich podokno poprvé
  zobrazí, a skrytá podokna CI a Kontejnery nenačítají nic, dokud se nezobrazí.
- Dlouhé relace potřebují méně paměti: dokončený diff si ponechá jen řádky, které ukazuje jeho karta, zodpovězená žádost
  o povolení uvolní svůj vstup, mezipaměť glyfů terminálu už neroste bez omezení a podokna Soubory a Git uvolňují řádky,
  které už neexistují.
- Samotný program je zhruba o 40 % menší.

## Okna a nabídky
- Zavření oblasti už nezavírá její panely: přesunou se do předchozí oblasti, jinak do následující. Spolu se svými panely
  se zavírá jen oblast náhledu a poslední oblast.
- Nabídka Zobrazení uvádí podokna v pořadí ikon bočního panelu.
- „Spravovat agenty…“ už není v nabídce +; zůstává pod „+ Nová relace“ v podokně Agenti.

## Opravy
- „Zahrnout a vyloučit soubory“ v podokně Hledání už vedle své šipky nezobrazuje zbloudilé písmeno.
- Zavření vyskakovacího okna s komentáři ke specifikaci už nezpůsobí pád aplikace.
