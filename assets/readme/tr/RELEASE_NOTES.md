<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Bu, kodlama ajanlarıyla çalışmak için yerel bir macOS düzenleyicisi olan Dango’nun ilk sürümüdür. Düzenleyici,
terminaller, bir tarayıcı, Git, CI, bir hata ayıklayıcı ve AI sohbetleri her klasör için tek bir pencereyi paylaşır.
Uygulama AppKit üzerinde Rust ile yazılmıştır; WebKit yalnızca tarayıcı sekmesinde kullanılır. Arayüz 35 dilde sunulur.

## Gereksinimler
- macOS 14 veya üstü, Apple silikon ya da Intel işlemcili.
- AI sohbetleri için seçtiğiniz ajan: Claude için giriş yapılmış `claude` komutu gerekir; Codex için Node.js gerekir ve
  uygulama istediğinizde sabitlenmiş paketini kurar; Antigravity ve OpenCode kendi `agy` veya `opencode` komutunuzu ya
  da sabitlenmiş bir indirmeyi kullanır; yerleşik ajan için bir API anahtarı ya da bu Mac’te Ollama veya LM Studio
  gerekir.
- Dil sunucuları ve hata ayıklama bağdaştırıcıları istediğinizde kurulur. gopls ve `dlv` için Go; TypeScript, Python,
  JSON, YAML, CSS, HTML ve kabuk sunucuları ile JavaScript hata ayıklayıcısı için Node.js gerekir; clangd, sourcekit-lsp
  ve `lldb-dap` Xcode Command Line Tools ile gelir.
- Düzenleme tahminleri için bir GitHub Copilot aboneliği (ücretsiz olanı da yeterli) ya da Ollama veya LM Studio’da bir
  fill-in-the-middle modeli gerekir.

## Pencere
- Her dosya, terminal, tarayıcı sayfası, AI sohbeti, diff ve önizleme, klasörün penceresinde bir macOS sekmesidir. Sekme
  çubuğunun sonundaki “+” ve ⌘T yeni bir dosya, bir terminal, tarayıcı ve kurulu her ajanı sunar; bunlar kendi Git
  worktree’sinde de açılabilir.
- Kenar çubuğu Dosyalar, Arama, Ajanlar, Git, Hata Ayıkla ve CI arasında geçiş yapar (⌘1–⌘6); kullanmadıklarınızı sağ
  tıklayarak gizleyebilirsiniz. Araç çubuğunda düzenlediğiniz yerler arasında geri ve ileri gitme (⌃- ve ⌃⇧-), dal,
  Hızlı Aç ve Etkinlik genel görünümü bulunur.
- “Yanda Aç” ikinci bir belgeyi ilkinin yanında gösterir. Yeniden başlatma ya da çökme sonrasında her pencere
  sekmeleriyle geri gelir; kaydedilmemiş metin içeren düzenleyiciler de bu metinle birlikte geri gelir.
- Görüntüler, ses, video, fontlar ve Office belgeleri bir sekmede Quick Look ile, PDF’ler PDFKit ile açılır.

## Düzenleyici
- Sözdizimi renklendirme, satır numaraları, son commit’e göre değişiklik işaretleri, bul ve değiştir, Hızlı Aç (⌘P) ve
  proje araması (⇧⌘F).
- Otomatik girinti, `.editorconfig` dosyasına göre sekme karakteri veya boşluk, ayraç çiftleri, yorum açma/kapama,
  katlama, satıra git, görünür boşluklar ve satır kaydırma.
- Birden fazla imleç: ⌥-tıklama, sütun için ⌥-sürükleme, sonraki eşleşme için ⌘D.
- Dosyalar kodlamalarını, bayt sırası işaretlerini ve satır sonlarını korur. Diskte değişen bir dosya, düzenleyicide
  kaydedilmemiş metin yoksa yeniden yüklenir; aksi hâlde bir çubuk farkları ve birleştirmeyi sunar.
- **Düzenleme tahminleri:** siz yazarken gri renkte bir öneri belirir; Tab onu kabul eder. Öneriler, ayarlarda hangisini
  seçtiyseniz GitHub Copilot’tan ya da Ollama veya LM Studio’daki yerel bir modelden gelir; yerel bir model kodunuzu bu
  Mac’te tutar.

## Dil sunucuları
Go, Rust, TypeScript ve JavaScript, Python, C, C++ ve Objective-C, Swift, kabuk betikleri, JSON, YAML, CSS, HTML, TOML,
Markdown ve Lua; ayrıca ayarlardan eklediğiniz kendi sunucularınız. Tanılamalar, üzerine gelince bilgi, tamamlama, imza
yardımı, tanıma, tür tanımına ve uygulamaya git, başvurular, çağrı ve tür hiyerarşileri, yeniden adlandırma,
biçimlendirme, anlamsal renkler, katlama ve ana hat; satırın içinde türler ve parametre adları; bir ampulün arkasında
hızlı düzeltmeler ve yeniden düzenlemeler (⌥⌘A); ⌥⌘O ile ya da Hızlı Aç’ta `#` ile projenin sembolleri. Bir TypeScript
projesinin kendi `tsc`’si dil sunucusu olarak çalışabiliyorsa o kullanılır.

## Hata ayıklama
Koşullu olanlar dahil kesme noktaları, adım adım ilerleme, çağrı yığınlarıyla iş parçacıkları, Denetçi’de (⌥⌘I)
değişkenler ve izlenen ifadeler, program duraklatılmışken kodun yanında değerler ve duraklatılmış çerçevede
değerlendirme yapan bir hata ayıklama konsolu. Rust, C, C++ ve Swift için `lldb-dap`, Go için `dlv`, JavaScript ve
TypeScript için JavaScript hata ayıklayıcısı, Python için debugpy. Projenin `.vscode/launch.json` dosyası okunur. Hata
Ayıkla menüsü Xcode’un klavye kestirmelerini kullanır.

## Terminal
Giriş kabuğunun ortamıyla sekmelerde çalışan, Metal ile çizilen terminaller. Yapıştırılan ya da bırakılan bir görüntü
terminalde çalışan bir kodlama ajanına verilir ya da kaydedilmiş bir kopyanın yolu yapıştırılır. ⌘-tıklama bağlantıları
ve dosya adlarını açar. Terminalin başlattığı bir geliştirme sunucusu tarayıcı sekmesinde açılabilir.

## Markdown
Bir Markdown dosyası, önizlemesi yanında olacak şekilde açılır (⇧⌘V); formüller, Mermaid, Graphviz ve PlantUML
diyagramları ve klasördeki resimler de görünür. Kalın, italik, bağlantılar, başlıklar, listeler, alıntılar ve kod
blokları için bir Biçim menüsü; Word’den veya bir web sayfasından yapıştırılan metin Markdown olarak gelir. **Dosya ›
Dışa Aktar** PDF, tek bir HTML dosyası ya da bir Word belgesi yazar. Splat yüklüyse **Dosya › Splat’te Aç** belgeyi
orada açar.

## Tarayıcı
Adres alanı, geri ve ileri, arama, önce soru soran indirmeler ve Web Denetçisi içeren bir tarayıcı sekmesi.

## Git
Dosyaları ya da tek tek parçaları hazırlama ve atma, commit’ler, commit ve push ya da eşzamanlama, dallar, stash’ler,
birleştirme çakışmaları ve son commit’lerin grafiğini içeren bir Git paneli. Blame, imlecin bulunduğu satırı en son
kimin değiştirdiğini ya da tüm dosya için bir sütun gösterir. Bir düğme commit iletisini AI ile yazar; diff ancak
gitleaks içinde gizli bilgi bulamadıktan sonra gönderilir ve her commit de gizli bilgiler için denetlenir. Bir AI
sohbeti kendi Git worktree’sinde çalışabilir; panel neyi değiştirdiğini gösterir ve işi birleştirme ya da cherry-pick
ile aktarır.

## CI
GitHub Actions ve GitLab CI, GitHub Enterprise Server ve kendi sunucunuzdaki GitLab dahil: geçerli dalın çalıştırmaları,
işleri ve günlükleri; başarısız işleri, tek bir işi ya da (GitHub’da) tüm bir çalıştırmayı yeniden çalıştırma, elle
başlatılan işleri başlatma ya da bekleyen dağıtımları onaylama, yeni bir pipeline ya da elle başlatılabilen bir iş
akışını başlatma ve iptal etme. Açık merge request’ler ya da pull request’ler ve issue’lar aynı panelde listelenir ve
tarayıcı sekmesinde açılır. Başarısız bir işin günlüğü, dışarıdan gelen metin olarak işaretlenip bir ajana
gönderilebilir. Token, Anahtar Zinciri’nden, `gh` ya da `glab`’dan, giriş kabuğunun ortamından gelir ya da siz
girersiniz.

## AI
Her ajan aynı zaman çizelgesine sahip bir AI sekmesinde çalışır: yanıtlar, çıktılarıyla araç çağrıları, izin istekleri,
sorular, onaylanacak ve yorumlanacak planlar, yapılacaklar, alt ajanlar ve arka plan görevleri; sekmenin başlığındaki
iki düğme çalışan alt ajanları ve arka plan işlemlerini listeler. Uygulama her istemden önce bir kontrol noktası tutar;
değişiklikler ileti bazında gözden geçirilip geri sarılabilir. Yeni sohbetler “Elle” modunda başlar ve yıkıcı Git
komutlarına hiçbir zaman kalıcı olarak izin verilmez. Etkinlik genel görünümü çalışan her şeyi listeler. Sekmedeki yeşil
nokta çalışan bir oturumu, kırmızı nokta sizi bekleyen bir oturumu gösterir; siz başka bir yere bakarken oturum
bittiğinde ya da beklediğinde bir bildirim sizi uyarır.
- **Claude**, değiştirilmemiş `claude` komutu üzerinden; oturumlar, alt ajanlar, arka plan görevleri, bağlam ve kullanım
  bilgisiyle.
- **ACP ajanları:** Codex, Antigravity ve OpenCode, istediğinizde ACP kayıt defterinden daha fazlası; kurulu ajanlar
  test edilmiş sürümlere sabitlenir ve sağlama toplamlarıyla doğrulanır.
- **Yerleşik ajan:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio ve OpenAI’ın ya da Anthropic’in
  API’sini konuşan sunucular; dosyalar, arama, komutlar, tanılamalar ve web istekleri için araçlarla; anahtarlar Anahtar
  Zinciri’nde kalır.
- Her sohbet için bir korumalı alan anahtarı, ajanın desteklediği ölçüde, ajanın komutlarının neler yapabileceğini
  sınırlar.
- Terminal çıktısı, bir web sayfası ya da bir CI günlüğü gibi dışarıdan gelen metin, ajana güvenilmeyen olarak
  işaretlenmiş şekilde gider.

## Dikte
AI ileti alanına ve düzenleyiciye Lowspeak’in konuşma tanımasıyla dikte: mikrofon düğmesi, ⌃⌥D ya da Dango etkin
uygulamayken fn tuşu. Lowspeak’in ayarlarını ve modellerini kullanır; çevrimiçi yazıya dökme için bir OpenAI veya Groq
anahtarı gerekir.

## Güncellemeler
Uygulama kendini Sparkle ile günceller. İkinci açılışta güncellemeleri kendiliğinden denetleyip denetleyemeyeceğini
sorar; siz onay verene kadar hiçbir istek göndermez. **Dango › Güncellemeleri Denetle…** hemen denetler.

## Erişilebilirlik
Kenar çubuğu, sekmeler, AI zaman çizelgesinin kartları, Git ve CI satırları, hata ayıklayıcı ve terminal metni için
VoiceOver adları; `keybindings.json` içinde değiştirebileceğiniz kestirmelerle tam klavye denetimi; Karşıtlığı Artır ve
Hareketi Azalt; sağdan sola arayüzlerde kod görünümleri soldan sağa kalır. Ayrıntılar ACCESSIBILITY.md dosyasındadır.

## Gizlilik
Telemetri, analiz ve çökme raporu yok. Uygulama yalnızca sizin başlattıklarınız için bağlanır: kullandığınız ajanlar,
istediğiniz indirmeler, giriş yaptığınız CI sunucuları, seçtiyseniz Copilot’tan tahminler, seçtiyseniz çevrimiçi yazıya
dökme, açtığınız sayfalar ve izin verdikten sonra güncelleme denetimi. Anahtarlar ve token’lar macOS Anahtar Zinciri’nde
kalır.

## Bilinen sınırlamalar
- Tarayıcı üzerinden CI’ya giriş kapalı; `gh`, `glab`, bir token ya da ortamı kullanın.
- Uygulama App Sandbox olmadan çalışır, çünkü terminaller, Git, dil sunucuları ve ajanlar onu devralırdı; bir klasörün
  yapılandırdığı herhangi bir şey çalışmadan önce o klasöre güvenmeniz gerekir. Antigravity ve OpenCode’un, uygulamanın
  açabileceği bir korumalı alanı yoktur.
- Claude’un kullanım rakamları ve bağlam dökümü, protokolünün deneysel olarak işaretlenmiş kısımlarına dayanır ve bunlar
  yoksa gizlenir. Konuşmayı geri sarma yalnızca Claude’da çalışır; diğer ajanlarda yalnızca kod geri sarılır.
- Gemini CLI sunulmuyor: Google kişisel hesapla girişi sonlandırdı ve Antigravity’ye yönlendiriyor. Önceki Gemini
  sohbetleri okunabilir kalır.
