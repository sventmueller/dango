## Güvenlik
- Bir ajanın yanıtlarındaki, planlarındaki ve raporlarındaki, Markdown önizlemesindeki ve PDF’lerdeki bağlantılar artık
  tek tıklamayla herhangi bir uygulamayı açmıyor. Web adresleri bir tarayıcı sekmesinde, proje içindeki dosyalar bir
  düzenleyici sekmesinde açılır; başka bir yerdeki bir dosya, bir ağ paylaşımı ya da başka bir uygulamanın bağlantısı
  gibi diğer her şey önce sorar ve nereye gittiğini gösterir.
- Bir ajanın ya da bir MCP sunucusunun ilettiği giriş ve Remote Control adresleri yalnızca web adresiyse açılır.
- Kullanıcı klasörünüz, Masaüstü, Belgeler, İndirilenler ya da bir birimin kökü gibi bir projeden çok daha fazlasını
  içeren klasörlere artık bir bütün olarak güvenilemez; bunlara proje proje güvenin. Güven sorusu artık güvenin içindeki
  klasörleri de kapsadığını söylüyor.
- CI token’ları ve girişleri hiçbir zaman şifrelenmemiş http üzerinden bu Mac dışındaki bir sunucuya gitmez; uzak deposu
  `http://` ile başlayan bir depoya https üzerinden sorulur.
- gitleaks olmadan düzenleme tahminleri, kod bu Mac’in dışındaki bir sağlayıcıya gitmeden önce, siz denetlemeden
  göndermeyi seçene kadar bekler. Bir projenin kendi gitleaks ayarları artık denetimi kapatmıyor.
- Yerleşik ajan, yazması onaylandıktan sonra ortaya çıkan bir bağlantı aracılığıyla artık proje dışına yazmaya
  yönlendirilemez; başka sabit bağlantıları olan dosyalar da üzerlerinden yazılmak yerine kendi dosyalarını alır.
- Bir dil sunucusunun tamamlamaları artık düzenleyiciyi kilitleyemez; sunucunun bağlantı olan bir dosyayı yeniden
  adlandırması ve silmesi, bağlantının gösterdiği yeri değil bağlantının kendisini etkiler.
- Bir isteme eklediğiniz resimler, her yerde olduğu gibi korumalı alanda çalışan kod çözme hizmeti tarafından çözülür.
- Terminaldeki bir program panoyu yalnızca terminali öndeyken değiştirebilir ve terminal bunu bildirir.
- rust-analyzer, o an en yeni olan sürümden değil, sağlama toplamıyla sabitlenmiş bir sürümden yüklenir.

## Arama ve gezinme
- “Hızlı Aç”, hiçbir dosya eşleşmediğinde bunu söyler ve dosyalarda mı yoksa projenin simgelerinde mi aradığını
  gösterir.
- Projenin simgeleri, hiçbir dosyaları açık olmadan da TypeScript ve JavaScript’i içerir. Projeden ve çalıştığınız
  dildeki tam eşleşmeler önce gelir; kitaplıklardan ve yüklü paketlerden gelen simgeler işaretlenir.
- Görüntü › Arama, odağı arama alanına taşır.

## Düzenleyici
- Bir Markdown dosyası önizlemesi yanında açıldığında düzenleyici odağı korur, böylece Biçim menüsü hemen çalışır.
  Düzenleyici komutları, odak kenar çubuğundayken de çalışır.
- Satır sonundaki ya da satırın altındaki bir kutudaki öneri, Tab ile kabul edildiğini söyler.
- Sekme genel görünümündeki aynı adlı sekmeler artık birbirinden ayırt edilebiliyor, örneğin bir dosya ve tarayıcıdaki
  önizlemesi.
- Görüntü menüsü artık macOS’in kendi “Sekme Çubuğunu Gizle” komutunu sunmuyor; Dango’nun sekmeleri bu komutu kullanmaz.

## Diller ve hata ayıklama
- JSON sunucusu artık dosyaları denetliyor; tanılamalarını başladıktan sonra kaydeden basedpyright ve sourcekit-lsp gibi
  sunucular da bunları gösteriyor.
- pip’in Python’a yüklemeyi reddettiği yerlerde, Homebrew’un Python’unda olduğu gibi, hata ayıklayıcı debugpy’ı projenin
  bir sanal ortamına yüklemeyi önerir.
