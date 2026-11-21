## Sikkerhed
- Links i en agents svar, planer og rapporter, i Markdown-eksemplet og i PDF’er åbner ikke længere en vilkårlig app med
  et enkelt klik. Webadresser åbnes i en browserfane, arkiver i projektet i en editorfane; alt andet, f.eks. et arkiv et
  andet sted, en netværksdeling eller en anden apps link, spørger først og viser, hvor det fører hen.
- Login- og Remote Control-adresser, som en agent eller en MCP-server videregiver, åbnes kun, når de er webadresser.
- Mapper, der indeholder langt mere end ét projekt, f.eks. din hjemmemappe, Skrivebord, Dokumenter, Overførsler eller
  roden af en diskenhed, kan ikke længere betros som helhed; stol på dem projekt for projekt. Spørgsmålet om tillid
  siger nu, at tilliden også gælder mapperne i den.
- CI-tokens og logins sendes aldrig længere over ukrypteret http til en anden server end denne Mac; et repository, hvis
  remote begynder med `http://`, bliver spurgt over https.
- Uden gitleaks venter forudsigelser, før kode sendes til en udbyder uden for denne Mac, indtil du vælger at sende
  ukontrolleret. Et projekts egne gitleaks-indstillinger slår ikke længere kontrollen fra.
- Appens egen agent kan ikke længere lokkes til at skrive uden for projektet via et link, der dukker op, efter at dens
  skrivning er godkendt, og arkiver med yderligere hårde links får deres eget arkiv i stedet for at blive skrevet
  igennem.
- En sprogservers fuldførelser kan ikke længere få editoren til at hænge, og dens omdøbninger og sletninger af et linket
  arkiv virker på linket, ikke på det, det peger på.
- Billeder, du vedhæfter en prompt, afkodes af den isolerede afkodningstjeneste, som alle andre steder.
- Et program i terminalen kan kun erstatte udklipsholderen, mens dets terminal er forrest, og terminalen siger det.
- rust-analyzer installeres fra en udgivelse, der er fastlåst med sin kontrolsum, i stedet for den, der tilfældigvis var
  nyest.

## Søgning og navigation
- Åbn hurtigt siger, når intet arkiv passer, og viser, om det søger i arkiver eller i projektets symboler.
- Projektets symboler omfatter TypeScript og JavaScript, uden at et af deres arkiver er åbent. Præcise resultater fra
  projektet og i det sprog, du arbejder i, kommer først; symboler fra biblioteker og installerede pakker er markeret.
- Oversigt › Søgning flytter fokus til søgefeltet.

## Editor
- Editoren beholder fokus, når et Markdown-arkiv åbnes med sit eksempel ved siden af, så menuen Format virker med det
  samme. Editorkommandoer virker også, mens fokus er i indholdsoversigten.
- Et forslag ved linjeslut eller i en boks under linjen siger, at Tab accepterer det.
- Faner med samme navn i faneoversigten kan skelnes fra hinanden, f.eks. et arkiv og dets eksempel i browseren.
- Menuen Oversigt tilbyder ikke længere macOS’ egen “Skjul fanelinje”, som Dangos faner ikke bruger.

## Sprog og fejlfinding
- JSON-serveren tjekker nu arkiver, og servere, der først registrerer deres diagnoser efter start, f.eks. basedpyright
  og sourcekit-lsp, viser dem.
- Hvor pip nægter at installere i Python, som i Homebrews, tilbyder fejlfinderen at installere debugpy i et virtuelt
  miljø i projektet.
