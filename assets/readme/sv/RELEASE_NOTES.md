## Terminal
- Terminalen är nu Tapas, Dangos systerapp för terminalen, med allt den tillför, medan Dango behåller det som dess terminal kunde förut: inklistring av bilder till agenter, utvecklingsservrar i webbläsarfliken, ⌘-klick in i redigeraren.
- **Innehåll › Visa/göm inmatningsfält** (⌘I medan en terminal har fokus) gör kommandoraden till ett textfält med mappen och Git-grenen ovanför. Kommandon färgläggs efter sina delar, och ett som ditt skal inte kan köra får en streckad röd understrykning.
- Förslag visas i grått medan du skriver, i fältet och utan det: från din historik, från beskrivningar av fler än 700 kommandoradsverktyg och från projektet, som git-grenar, `package.json`-skript och mål för make, just och cargo. → tar ett; Tabb kompletterar och öppnar vid flera kandidater en meny som filtreras när du skriver. De använder aldrig nätverket.
- Kommandoblock: ⌘↑ och ⌘↓ hoppar mellan prompter, menyn **Redigera** markerar, kopierar och sparar ett kommandos utdata, och bokmärken markerar rader. Med **Markera kommandon som block** påslaget visar en stapel varje block, misslyckade kommandon markeras och knappar över ett block visar dess status och varaktighet.
- **Skicka block till agent** (menyn Redigera och terminalens kontextmeny) öppnar en AI-flik med det aktuella blockets kommandorad och utdata bifogade som Externt innehåll.
- **Sök** (⌘F medan en terminal har fokus) söker i hela historiken med reguljära uttryck, skiftlägeskänsligt och på hela ord; rullningslisten markerar träffarna.
- Ett programs notis (OSC 9 och OSC 777) och dess klocka sätter den röda pricken på terminalens flik medan du inte ser den, och notisen går till macOS när en annan app ligger längst fram. Förlopp som ett program rapporterar (OSC 9;4) visas som en ring på dess flik och som en stapel på Dock-symbolen.
- Ett dubbelklick markerar en hel URL, sökväg eller hash, att dra med ⌥ markerar en rektangel, **Kopiera med formatering** behåller färger och stilar och **Markera med tangentbordet** flyttar en markering med Vims tangenter.
- Inklistring av flera rader som skulle köras direkt visas först, och **Dango › Säker tangentbordsinmatning** hindrar andra appar från att läsa det du skriver i en terminal.
- Länkar av andra slag än webbadresser, `mailto:` och filer öppnas inte längre från terminalen.

## Inställningar
- **Inställningar › Terminal** slår på och av inmatningsfältet för nya terminaler, dess Vim-tangenter, förslagen, färgläggningen av kommandon, understrykningen av okända kommandon, kommandoblock, att markeringen kopieras direkt, att inklistring av flera rader visas först och ⌥ som metatangent, som har flyttat hit från Allmänt.
