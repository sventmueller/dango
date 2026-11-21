## Bezpieczeństwo
- Łącza w odpowiedziach, planach i raportach agenta, w podglądzie Markdown i w plikach PDF nie otwierają już żadnej
  aplikacji jednym kliknięciem. Adresy internetowe otwierają się w karcie przeglądarki, pliki w projekcie w karcie
  edytora; wszystko inne, na przykład plik w innym miejscu, udział sieciowy lub łącze innej aplikacji, najpierw pyta i
  pokazuje, dokąd prowadzi.
- Adresy logowania i Remote Control przekazywane przez agenta lub serwer MCP otwierają się tylko wtedy, gdy są adresami
  internetowymi.
- Folderom, które zawierają o wiele więcej niż jeden projekt, takim jak folder domowy, Biurko, Dokumenty, Pobrane lub
  katalog główny woluminu, nie można już zaufać jako całości; ufaj im projekt po projekcie. Pytanie o zaufanie mówi
  teraz, że obejmuje ono także foldery wewnątrz.
- Tokeny CI i dane logowania nigdy nie trafiają przez zwykłe http do serwera innego niż ten Mac; repozytorium, którego
  repozytorium zdalne zaczyna się od `http://`, jest odpytywane przez https.
- Bez gitleaks przewidywanie edycji czeka, zanim kod trafi do dostawcy spoza tego Maca, aż zdecydujesz się wysłać go
  bez sprawdzania. Własne ustawienia gitleaks projektu nie wyłączają już sprawdzania.
- Wbudowanego agenta nie da się już nakłonić do zapisu poza projektem przez łącze, które pojawia się po zatwierdzeniu
  zapisu, a pliki z kolejnymi dowiązaniami twardymi dostają własny plik, zamiast być zapisywane przez dowiązanie.
- Uzupełnienia serwera języka nie mogą już zawiesić edytora, a zmiana nazwy i usunięcie dowiązanego pliku przez serwer
  działają na dowiązaniu, a nie na tym, na co ono wskazuje.
- Obrazy dołączane do promptu są dekodowane przez usługę dekodowania w piaskownicy, tak jak wszędzie indziej.
- Program w terminalu może zastąpić zawartość schowka tylko wtedy, gdy jego terminal jest na wierzchu, a terminal o tym
  informuje.
- rust-analyzer instaluje się z wydania przypiętego jego sumą kontrolną, a nie z tego, które akurat było najnowsze.

## Wyszukiwanie i nawigacja
- Szybko otwórz informuje, gdy żaden plik nie pasuje, i pokazuje, czy przeszukuje pliki, czy symbole projektu.
- Symbole projektu obejmują TypeScript i JavaScript bez otwierania któregoś z ich plików. Dokładne dopasowania z
  projektu i w języku, w którym pracujesz, są pierwsze; symbole z bibliotek i zainstalowanych pakietów są oznaczone.
- Widok › Wyszukiwanie przenosi fokus do pola wyszukiwania.

## Edytor
- Edytor zachowuje fokus, gdy plik Markdown otwiera się z podglądem obok, więc menu Format działa od razu. Polecenia
  edytora działają też wtedy, gdy fokus jest na pasku bocznym.
- Propozycja na końcu wiersza lub w polu pod nim informuje, że Tab ją przyjmuje.
- Karty o tej samej nazwie w przeglądzie kart można od siebie odróżnić, na przykład plik i jego podgląd w przeglądarce.
- Menu Widok nie oferuje już systemowego polecenia macOS „Ukryj pasek kart”, którego karty Dango nie używają.

## Języki i debugowanie
- Serwer JSON sprawdza teraz pliki, a serwery, które rejestrują swoją diagnostykę po uruchomieniu, takie jak
  basedpyright i sourcekit-lsp, ją pokazują.
- Tam, gdzie pip odmawia instalacji w Pythonie, jak w Pythonie z Homebrew, debuger proponuje zainstalowanie debugpy w
  środowisku wirtualnym projektu.
