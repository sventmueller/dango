## Bezpečnosť
- Odkazy v odpovediach, plánoch a hláseniach agenta, v náhľade Markdownu a v PDF už jedným kliknutím neotvoria žiadnu
  aplikáciu. Webové adresy sa otvoria v tabe prehliadača, súbory v projekte v tabe editora; všetko ostatné, napríklad
  súbor inde, sieťové zdieľanie alebo odkaz inej aplikácie, sa najprv opýta a ukáže, kam vedie.
- Adresy na prihlásenie a pre Remote Control, ktoré odovzdá agent alebo MCP server, sa otvoria, len ak sú to webové
  adresy.
- Priečinkom, ktoré obsahujú oveľa viac než jeden projekt, napríklad domovskému priečinku, Ploche, Dokumentom,
  Stiahnutým súborom alebo koreňu zväzku, už nemožno dôverovať ako celku; dôverujte im projekt po projekte. Otázka na
  dôveru teraz uvádza, že dôvera platí aj pre priečinky v ňom.
- Tokeny a prihlásenia CI nikdy neodchádzajú cez nešifrované http na iný server než tento Mac; repozitár, ktorého
  vzdialený repozitár sa začína na `http://`, sa dopytuje cez https.
- Bez gitleaks predpovede pri úpravách pred odoslaním kódu poskytovateľovi mimo tohto Macu počkajú, kým sa nerozhodnete
  odoslať ho bez kontroly. Vlastné nastavenia gitleaks v projekte už kontrolu nevypnú.
- Vlastného agenta už nemožno naviesť, aby zapisoval mimo projektu cez odkaz, ktorý sa objaví až po schválení jeho
  zápisu, a súbory s ďalšími pevnými odkazmi dostanú vlastný súbor namiesto toho, aby sa cez ne zapisovalo.
- Dopĺňanie jazykového servera už nemôže zaseknúť editor a jeho premenovania a odstránenia súboru, ktorý je odkazom,
  pôsobia na samotný odkaz, nie na to, kam ukazuje.
- Obrázky, ktoré pripojíte k promptu, dekóduje dekódovacia služba v sandboxe, tak ako všade inde.
- Program v termináli môže nahradiť schránku len vtedy, keď je jeho terminál v popredí, a terminál to oznámi.
- rust-analyzer sa inštaluje z vydania pripnutého s jeho kontrolným súčtom namiesto toho, ktoré bolo práve najnovšie.

## Vyhľadávanie a navigácia
- „Rýchlo otvoriť“ oznámi, keď sa nenájde žiadny súbor, a ukáže, či vyhľadáva súbory, alebo symboly projektu.
- Symboly projektu zahŕňajú TypeScript a JavaScript, aj keď nie je otvorený žiadny ich súbor. Ako prvé sa zobrazia
  presné zhody z projektu a v jazyku, v ktorom pracujete; symboly z knižníc a nainštalovaných balíkov sú označené.
- Zobraziť › Vyhľadávanie presunie fokus do vyhľadávacieho poľa.

## Editor
- Editor si ponechá fokus, keď sa súbor Markdown otvorí s náhľadom vedľa seba, takže ponuka Formát funguje hneď. Príkazy
  editora fungujú aj vtedy, keď je fokus na postrannom paneli.
- Návrh na konci riadka alebo v poli pod ním oznamuje, že ho prijmete klávesom Tab.
- Taby s rovnakým názvom v prehľade tabov sa dajú rozlíšiť, napríklad súbor a jeho náhľad v prehliadači.
- Ponuka Zobraziť už neponúka vlastnú položku macOS „Skryť panel tabov“, ktorú taby Dango nepoužívajú.

## Jazyky a ladenie
- JSON server teraz kontroluje súbory a servery, ktoré zaregistrujú svoju diagnostiku až po spustení, napríklad
  basedpyright a sourcekit-lsp, ju zobrazujú.
- Tam, kde pip odmieta inštalovať do Pythonu, ako to robí Python z Homebrew, ponúkne nástroj na ladenie inštaláciu
  debugpy do virtuálneho prostredia projektu.
