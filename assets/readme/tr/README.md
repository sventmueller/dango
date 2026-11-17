<p><img src="../../readme-icon.png" alt="Dango uygulama simgesi" width="135" height="135"></p>

# Dango

**Kodlama ajanlarıyla çalışmak için yerel macOS düzenleyicisi.**

Dango kodu, onu değiştiren ajanları ve çevrelerindeki her şeyi her klasör için tek bir pencerede toplar: dil sunucuları ve düzenleme tahminleri olan bir düzenleyici, terminaller, bir tarayıcı, Git, CI, bir hata ayıklayıcı ve Claude, Codex, Antigravity, OpenCode ya da uygulamaya yerleşik bir ajanla AI sohbetleri. Her ajan aynı zaman çizelgesinde çalışır, dosyaları değiştirmeden ya da komut çalıştırmadan önce sorar ve her istemden önce bir kontrol noktası bırakır; böylece yaptıklarını ileti ileti gözden geçirip geri sarabilirsiniz. Bir klasörün yapılandırdığı hiçbir şey, siz o klasöre güvenmeden çalışmaz ve dışarıdan gelen metin ajana güvenilmeyen olarak işaretlenmiş şekilde ulaşır. Düzenleyiciye ve ileti alanına dikte edin; konuşma tanıma Mac’inizde kalabilir. Dango, Electron olmadan gerçek AppKit denetimleri üzerinde Rust ile yazılmıştır, 35 dil konuşur ve telemetri içermez.

[![Dango.dmg indir](https://img.shields.io/badge/Dango_indir-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Dango.dmg indir](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: imzalanmış ve Apple tarafından onaylanmış (notarized) disk görüntüsü; Apple Silicon ve Intel üzerinde macOS 14 veya üstü için. Bağlantı ilk sürümle birlikte kullanılabilir olacak.

**Bir hata mı buldunuz?** [Bir issue açın](https://github.com/sventmueller/dango/issues/new) ve hatanın hangi ajanla ve ne tür bir projede ortaya çıktığını belirtin.

[![Rust, 2024 sürümü](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 veya üstü](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Evrensel: Apple Silicon ve Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![objc2 ile AppKit](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 arayüz dili](https://img.shields.io/badge/languages-35-4C9A2A)
![Telemetri yok](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Hardened Runtime ile imzalı](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Lisans: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#lisans)

[Bahasa Indonesia](../id/README.md) ·
[Bahasa Melayu](../ms/README.md) ·
[Català](../ca/README.md) ·
[Čeština](../cs/README.md) ·
[Dansk](../da/README.md) ·
[Deutsch](../de/README.md) ·
[English](../../../README.md) ·
[Español](../es/README.md) ·
[Français](../fr/README.md) ·
[Hrvatski](../hr/README.md) ·
[Italiano](../it/README.md) ·
[Magyar](../hu/README.md) ·
[Nederlands](../nl/README.md) ·
[Norsk](../no/README.md) ·
[Polski](../pl/README.md) ·
[Português (Brasil)](../pt_BR/README.md) ·
[Português (Portugal)](../pt_PT/README.md) ·
[Română](../ro/README.md) ·
[Slovenčina](../sk/README.md) ·
[Slovenščina](../sl/README.md) ·
[Suomi](../fi/README.md) ·
[Svenska](../sv/README.md) ·
[Tiếng Việt](../vi/README.md) ·
**Türkçe** ·
[Ελληνικά](../el/README.md) ·
[Русский](../ru/README.md) ·
[Українська](../uk/README.md) ·
[עברית](../he/README.md)‎ ·
[العربية](../ar/README.md)‎ ·
[हिन्दी](../hi/README.md) ·
[ไทย](../th/README.md) ·
[한국어](../ko/README.md) ·
[日本語](../ja/README.md) ·
[简体中文](../zh-Hans/README.md) ·
[繁體中文](../zh-Hant/README.md)

## İçindekiler

- [Bir bakışta özellikler](#bir-bakışta-özellikler)
- [Gereksinimler](#gereksinimler)
- [Kurulum](#kurulum)
- [Başlarken](#başlarken)
- [AI ajanları](#ai-ajanları)
  - [Zaman çizelgesi](#zaman-çizelgesi)
  - [Ajan izinleri ve modlar](#ajan-izinleri-ve-modlar)
  - [Kontrol noktaları ve worktree’ler](#kontrol-noktaları-ve-worktreeler)
- [Düzenleyici](#düzenleyici)
- [Dil sunucuları](#dil-sunucuları)
- [Testler](#testler)
- [Hata ayıklayıcı](#hata-ayıklayıcı)
- [Terminal](#terminal)
- [Tarayıcı](#tarayıcı)
- [Git](#git)
- [CI](#ci)
- [Container’lar](#containerlar)
- [Markdown](#markdown)
- [Dikte](#dikte)
- [Güven](#güven)
- [Korumalı alan](#korumalı-alan)
- [Görünüm ve erişilebilirlik](#görünüm-ve-erişilebilirlik)
- [Diller](#diller)
- [Klavye kestirmeleri](#klavye-kestirmeleri)
- [Gizlilik ve güvenlik](#gizlilik-ve-güvenlik)
- [Verilerin bulunduğu yerler](#verilerin-bulunduğu-yerler)
- [Bilinen sınırlamalar](#bilinen-sınırlamalar)
- [Kaynaktan derleme](#kaynaktan-derleme)
- [Lisans](#lisans)

## Bir bakışta özellikler

- **Tüm ajanlar tek bir zaman çizelgesinde.** Kendi `claude` komutu üzerinden Claude, Agent Client Protocol üzerinden Codex, Antigravity ve OpenCode, ACP kayıt defterinden daha fazlası ve Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama ve LM Studio için uygulamaya yerleşik bir ajan. Yanıtlar, araç çağrıları, izin istekleri, planlar, yapılacaklar, alt ajanlar ve arka plan görevleri hangi ajan çalışırsa çalışsın aynı görünür.
- **Gözden geçirin ve geri sarın.** Dango her istemden önce bir kontrol noktası tutar. Bir ajanın değiştirdikleri ileti bazında gözden geçirilip geri sarılabilir; bir sohbet kendine ait bir Git worktree’sinde çalışıp işini birleştirme ya da cherry-pick ile teslim edebilir.
- **Her klasör için bir pencere.** Dosyalar, terminaller, tarayıcı sayfaları, AI sohbetleri, diff’ler ve önizlemeler o pencerenin macOS sekmeleridir. Kenar çubuğunda Dosyalar, Arama, Ajanlar, Git, Testler, Hata Ayıkla, CI ve Container’lar bulunur.
- **Gerçek bir kod düzenleyici.** Sözdizimi renkleri, birden fazla imleç, katlama, `.editorconfig`, tamamlama, tanıma git, başvurular, yeniden adlandırma, hızlı düzeltmeler ve hiyerarşilerle 17 dil için dil sunucuları ve GitHub Copilot’tan ya da yerel bir modelden düzenleme tahminleri.
- **Hata ayıklayıcı, Git ve CI yerleşik.** Rust, C, C++, Swift, Go, JavaScript, TypeScript ve Python için kesme noktaları ve adım adım ilerleme; tek tek parçaları hazırlama, dallar, stash’ler, çakışmalar ve bir commit grafiği; GitHub Actions ve GitLab CI’dan çalıştırmalar, işler ve günlükler.
- **Klasörlerinize karşı özenli.** Güvenmediğiniz bir klasör hiçbir şey başlatmaz: Git işlemleri, dil sunucuları, hook’lar ya da ajanlar çalışmaz. Yıkıcı Git komutlarına hiçbir zaman kalıcı olarak izin verilmez ve commit’ler gizli bilgiler için denetlenir.
- **Dikte.** Düzenleyiciye ve ileti alanına konuşun; çevrimiçi olarak OpenAI ya da Groq üzerinden veya yerel olarak Whisper ve Parakeet ile.
- **Özüne kadar yerel.** AppKit pencereleri, sekmeleri, menüleri ve araç çubukları, düzenleyici için TextKit 2, terminal için Metal, WebKit yalnızca tarayıcı sekmesinde. Koyu mod, Karşıtlığı Artır ve VoiceOver her Mac uygulamasında olduğu gibi çalışır.
- **Kendi dilinizde.** Arayüz 35 dilde sunulur: Arapça, Katalanca, Çince (Basitleştirilmiş), Çince (Geleneksel), Hırvatça, Çekçe, Danca, Felemenkçe, İngilizce, Fince, Fransızca, Almanca, Yunanca, İbranice, Hintçe, Macarca, Endonezce, İtalyanca, Japonca, Korece, Malayca, Norveççe, Lehçe, Portekizce (Brezilya), Portekizce (Portekiz), Romence, Rusça, Slovakça, Slovence, İspanyolca, İsveççe, Tayca, Türkçe, Ukraynaca ve Vietnamca.

## Gereksinimler

- Apple Silicon ya da Intel üzerinde macOS 14 Sonoma veya üstü. Dango, macOS 27 üzerinde geliştirilir ve test edilir.
- AI sohbetleri için kullanmak istediğiniz ajan; bkz. [AI ajanları](#ai-ajanları).
- Dango’nun istediğinizde kurduğu dil sunucuları ve hata ayıklama bağdaştırıcıları için: gopls ve `dlv` için Go; TypeScript, Python, JSON, YAML, CSS, HTML ve kabuk sunucuları ile JavaScript hata ayıklayıcısı için Node.js; clangd, sourcekit-lsp ve `lldb-dap` için Xcode Command Line Tools. rust-analyzer, GitHub sürümlerinden indirilir.
- Düzenleme tahminleri için bir GitHub Copilot aboneliği (ücretsiz olanı da yeterli) ya da Ollama veya LM Studio’da bir fill-in-the-middle modeli.

## Kurulum

**Disk görüntüsünden.** Her sürüm, Apple Silicon ve Intel için tek bir evrensel uygulama içeren, Developer ID ile imzalanmış ve Apple tarafından onaylanmış bir disk görüntüsüdür: `Dango-<version>.dmg`. Görüntüyü açın ve Dango’yu Uygulamalar klasörüne sürükleyin. Bir Homebrew Cask aynı görüntüyü kurar. Henüz hiçbir sürüm yayımlanmadı.

**Güncellemeler.** Disk görüntüsünden kurulan Dango kendini [Sparkle](https://sparkle-project.org) ile güncel tutar. İkinci açılışta güncellemeleri kendiliğinden denetleyip denetleyemeyeceğini sorar; siz onay verene kadar hiçbir istek göndermez. **Dango › Güncellemeleri Denetle…** hemen denetler. Güncellemeler uygulamanın kendi anahtarıyla imzalanır ve Sparkle herhangi bir şeyi kurmadan önce bu imzayı denetler. Kendi derlediğiniz bir sürümde güncelleyici yoktur.

**Kaynaktan.** Bkz. [Kaynaktan derleme](#kaynaktan-derleme); ardından `target/release/Dango.app` uygulamasını `/Applications` klasörüne taşıyın.

**Terminal komutu.** **Dango › “dango” Komutunu Yükle…**, yönetici hakları gerektirmeden `~/.local/bin/dango` konumuna küçük bir betik yazar; `~/.local/bin` dizininin `PATH` değişkeninizde olması gerekir. Ardından `dango .` geçerli klasörü, `dango file.rs` ise bir dosyayı açar. Dango orada kendi yazmadığı bir dosyayı asla değiştirmez.

## Başlarken

1. **Bir klasör açın:** ⌘O ile ya da `dango <folder>` ile. Dango ilk seferde o klasöre güvenip güvenmediğinizi sorar (bkz. [Güven](#güven)).
2. **Kenar çubuğu** Dosyalar (⌘1), Arama (⌘2), Ajanlar (⌘3), Git (⌘4), Testler (⌘5), Hata Ayıkla (⌘6), CI (⌘7) ve Container’lar (⌘8) arasında geçiş yapar; sağ tıklama kullanmadıklarınızı gizler, ⌃⌘S de kenar çubuğunu kapatır.
3. **Her belge bir sekmedir;** klasörün penceresinde, macOS’in kendi sekmeleriyle. Sekme çubuğunun sonundaki “+” ve ⌘T yeni bir dosya, bir terminal, tarayıcı ve kurulu her ajanı sunar, kendine ait bir Git worktree’sinde de; “+” düğmesini basılı tutmak aynı seçenekleri menü olarak gösterir.
4. **Hızlı Aç** ⌘P ile dosyaları, `#` ile de projenin sembollerini bulur; ⇧⌘F projede arar ve değiştirir.
5. **Bir ajanla konuşun:** bir AI sekmesinde. ⌘Esc klavye odağını ajanın ileti alanına taşır ve geri getirir.
6. **Ayarlar** (⌘,) Genel, CI, Ajanlar, Yerleşik Ajan, Dil sunucusu, Dikte ve Tahmin sekmelerini içerir.

Araç çubuğunda düzenlediğiniz yerler arasında geri ve ileri gitme (⌃- ve ⌃⇧-), dal, Hızlı Aç ve çalışan her şeyi listeleyen Etkinlik genel görünümü bulunur. Bir dosyanın bağlam menüsündeki **Yanda Aç** ikinci bir belgeyi ilkinin yanında gösterir. Yeniden başlatma ya da çökme sonrasında her pencere sekmeleriyle geri gelir; kaydedilmemiş metin içeren düzenleyiciler de bu metinle birlikte geri gelir. Görüntüler, ses, video, fontlar ve Office belgeleri bir sekmede Quick Look ile, PDF’ler PDFKit ile açılır.

## AI ajanları

| Ajan | Nasıl çalışır | Neye ihtiyaç duyar |
|---|---|---|
| Claude | `claude` komutu, doğrudan stream-json protokolü üzerinden konuşulur | `claude` kurulu ve giriş yapılmış olmalı; Dango giriş kabuğunun `PATH` değişkenine, `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` ve `/usr/local/bin` konumlarına bakar. Giriş yalnızca `claude` içinde yapılır |
| Codex | Agent Client Protocol (ACP) üzerinden `codex-acp` | Node.js ve npm; Dango istediğinizde sabitlenmiş paketi kurar |
| Antigravity | `agy acp` | `PATH` içindeki kendi `agy` komutunuz ya da istediğinizde kurulan sabitlenmiş indirme |
| OpenCode | `opencode acp` | `PATH` içindeki kendi `opencode` komutunuz ya da istediğinizde kurulan sabitlenmiş indirme |
| Diğer ACP ajanları | yalnızca istediğinizde getirilen ACP kayıt defterinden | kayıt defteri girdisinin belirttiği her şey; sağlama toplamı olmayan girdiler ancak siz onayladıktan sonra kurulur |
| Yerleşik ajan | uygulamaya yerleşik; dosyalar, arama, komutlar, tanılamalar ve web istekleri için araçlarla | Anthropic, OpenAI, Google Gemini ya da OpenRouter için Anahtar Zinciri’nde tutulan bir API anahtarı; anahtar olmadan bu Mac’te Ollama ya da LM Studio; OpenAI’ın ya da Anthropic’in API’sini konuşan herhangi bir sunucu |

**Ayarlar › Ajanlar** bu Mac’te hangi ajanların bulunduğunu gösterir, diğerlerini ve güncellemelerini kurar ve ACP kayıt defterini açar. Dango’nun kurduğu ajanlar test edilmiş sürümlere sabitlenir ve yayımlanmış sağlama toplamlarıyla (npm integrity ya da SHA-256) doğrulanır; Dango sürümü belirtilmemiş hiçbir `npx` paketini çalıştırmaz.

### Zaman çizelgesi

Her ajan aynı zaman çizelgesine sahip bir AI sekmesinde çalışır: yanıtlar, çıktılarıyla araç çağrıları, izin istekleri, sorular, onaylanacak ve yorumlanacak planlar (⌥⌘K), yapılacaklar, alt ajanlar ve arka plan görevleri. Sekmenin başlığındaki iki düğme çalışan alt ajanları ve arka plan işlemlerini listeler. İçeriğini ajana vermek için bir dosyadan, `@terminal` ya da `@browser` ifadesinden bahsedin. Sekmedeki yeşil nokta çalışan bir oturumu, kırmızı nokta sizi bekleyen bir oturumu gösterir; siz başka bir yere bakarken oturum bittiğinde ya da beklediğinde bir bildirim sizi uyarır. Claude için durum satırı bağlamı ve kullanımı gösterir.

### Ajan izinleri ve modlar

Yeni sohbetler “Elle” modunda başlar: ajan dosyaları değiştirmeden ya da komut çalıştırmadan önce sorar. Yıkıcı Git komutları (`push --force`, `reset --hard`, `clean -fd`) için Dango “Her Zaman İzin Ver” seçeneği sunmaz. Yerleşik ajan “Her Zaman İzin Ver” kurallarını proje başına tutar, bunları **Ayarlar › Yerleşik Ajan** içinde listeler ve kayıtlı olan ne olursa olsun yıkıcı Git komutları için her seferinde sorar; Claude ve ACP ajanları bu tür kuralları kendi yapılandırmalarında tutar. “Her Şeye İzin Ver” seçiliyken ve korumalı alan kapalıyken sohbet kalıcı bir uyarı gösterir.

### Kontrol noktaları ve worktree’ler

Dango her istemden önce bir kontrol noktası tutar: projenin deposunda `refs/ide-checkpoints/` altında bir commit olarak ya da Git kullanmayan bir proje için kendi klasöründe; normal bir push ya da fetch bunları asla taşımaz. Bir ajanın değiştirdikleri ileti bazında gözden geçirilip geri sarılabilir. Bir sohbet kendine ait bir Git worktree’sinde çalışabilir: Git paneli neyi değiştirdiğini gösterir ve işi birleştirme ya da cherry-pick ile aktarır.

## Düzenleyici

- **Sözdizimi renkleri, satır numaraları ve değişiklik işaretleri** (son commit’e göre), bul ve değiştir, Hızlı Aç (⌘P) ve proje araması (⇧⌘F).
- **Girinti** dosyayı ve `.editorconfig` dosyasını izler: sekme karakteri ya da boşluk, otomatik girinti, ayraç çiftleri.
- **Birden fazla imleç:** ⌥-tıklama, sütun için ⌥-sürükleme, ⌃⇧↑ ve ⌃⇧↓, sonraki eşleşme için ⌘D.
- **Katlama** (⌥⌘← ve ⌥⌘→, hepsi için ⇧ ile), yorum açma/kapama (⌘/), satıra git (⌃G), görünür boşluklar ve satır kaydırma (⌥Z).
- **Dosyalar kodlamalarını,** bayt sırası işaretlerini ve satır sonlarını korur. Diskte değişen bir dosya, düzenleyicide kaydedilmemiş metin yoksa yeniden yüklenir; aksi hâlde bir çubuk farkları ve birleştirmeyi sunar.
- **Düzenleme tahminleri.** Siz yazarken gri renkte bir öneri belirir ve Tab onu kabul eder. Öneri, **Ayarlar › Tahmin** içinde seçtiğinize göre GitHub Copilot’tan ya da Ollama veya LM Studio’daki yerel bir modelden gelir; yerel bir model kodunuzu bu Mac’te tutar.
- **Mini Harita ve Yapışkan Kaydırma** (Düzenleyici menüsü): metnin yanında tüm dosyanın dar bir resmi ve en üst satırın içinde bulunduğu blokların başlık satırları, bu satırın üstünde sabitlenmiş olarak.
- **Satır Sonunda Tanılamalar** (Görüntü menüsü): her satırın en ciddi iletisi, bir saniyelik duraklamadan sonra, önem derecesini gösteren bir simgeyle.
- **Statik sayfaların canlı önizlemesi:** Düzenleyici › Tarayıcı Sekmesinde Aç, güvenilen bir klasörün sayfalarını doğrudan Dango’nun kendisinden sunar ve bir dosya kaydedildiğinde sekmeyi yeniden yükler.
- **Prettier**, projenin yapılandırdığı yerlerde JavaScript, TypeScript, CSS, HTML, Markdown ve daha fazlasını projenin kendi `node_modules/.bin/prettier` dosyasıyla biçimlendirir; `formatter` ayarı her dil için biçimlendiriciyi seçer.
- **Vim Modu** (Düzenleyici › Vim Modu): Normal, Ekle, Görsel ve Değiştir modları, tekrar sayıları, hareketler ve metin nesneleriyle operatörler, `.`, yazmaçlar, imler ve makrolar. `u` ve ⌃R düzenleyicinin geri alma özelliğini kullanır ve bir ekleme turu tek bir adımdır. `"+` ve `"*` panodur; `vimClipboard` ayarı adsız yazmacı da pano yapar. `/`, `?`, `n`, `N`, `*`, `:s` ve `:%s`, Vim’in desenleriyle değil, Rust’ın düzenli ifadeleriyle arar: `\v` yoktur ve `\<` ile `\>` yerine `\b` kullanılır. Ex komutları `:w`, `:q`, `:wq`, `:x`, `:noh` ve `:<line>` şeklindedir. Normal modda ⌃G kestirmesini satıra git yerine Vim kullanır.
- **Ayar eşzamanlama**, iCloud Drive’daki bir klasör üzerinden (**Ayarlar › Genel**): ayarlarınız ve kestirmeleriniz diğer Mac’lerinize de sizinle gelir; yollar, yerel adresler ve onaylar her Mac’te ayrı kalır.

## Dil sunucuları

Go, Rust, TypeScript ve JavaScript, Python, C, C++ ve Objective-C, Swift, kabuk betikleri, JSON, YAML, CSS, HTML, TOML, Markdown ve Lua; ayrıca **Ayarlar › Dil sunucusu** içinden eklediğiniz kendi sunucularınız. Dango bir sunucuyu istediğinizde kendi araçlar klasörüne kurar ve bir TypeScript projesinin kendi `tsc`’si dil sunucusu olarak çalışabiliyorsa onu kullanır.

Getirdikleri: tanılamalar (⌘' ve ⇧⌘' aralarında gezinir), üzerine gelince bilgi (⌃⌘I), tamamlama (⌥Esc), imza yardımı, tanıma git (⌃⌘J), tür tanımı ve uygulama, başvurular (⌃⇧⌘F), çağrı ve tür hiyerarşileri (⌃⇧⌘H), yeniden adlandırma (⌃⌘E), biçimlendirme (⌥⇧F), anlamsal renkler, katlama ve ana hat (⇧⌘O); satırın içinde türler ve parametre adları; bir ampulün arkasında hızlı düzeltmeler ve yeniden düzenlemeler (⌥⌘A); ⌥⌘O ile projenin sembolleri.

Bir dosyanın kendi sunucusunun yanında ESLint, onu kullanan projelerin JavaScript ve TypeScript kodunu hızlı düzeltmeleriyle birlikte denetler, Emmet’in kısaltmaları da HTML, CSS ve JSX için tamamlamada görünür. Dango ikisini de istediğinizde kurar; **Ayarlar › Genel** içinde isterseniz ESLint’in sorunları kaydederken düzeltilir.

## Testler

Testler paneli bir klasörün testlerini paket ve dosyaya göre durumlarıyla birlikte listeler: `cargo test` ya da cargo-nextest, Go, pytest, Vitest ve Jest. Kenar boşluğunda her testin yanındaki simge testi çalıştırır, ⌥-tıklama testte hata ayıklar ve başarısız bir assert kendi satırını işaretler. Hata Ayıkla menüsü imleç konumundaki testi (⌃⌥⌘U), son testleri yeniden (⌃⌥⌘G) ya da tümünü çalıştırır ve imleç konumundaki testte hata ayıklar. Testler projenin kodunu çalıştırdığından yalnızca güvenilen klasörlerde çalışır; listeleme her test programını derlediğinden `cargo test` testlerini yalnızca siz istediğinizde listeler.

## Hata ayıklayıcı

Koşullu olanlar ve günlük noktaları dahil kesme noktaları, adım adım ilerleme, çağrı yığınlarıyla iş parçacıkları, Denetçi’de (⌥⌘I) değişkenler ve izlenen ifadeler, program duraklatılmışken kodun yanında değerler ve duraklatılmış çerçevede değerlendirme yapan bir hata ayıklama konsolu.

| Dil | Hata ayıklama bağdaştırıcısı |
|---|---|
| Rust, C, C++, Swift | Xcode Command Line Tools’taki `lldb-dap` |
| Go | `dlv` |
| JavaScript, TypeScript | Node.js üzerinde JavaScript hata ayıklayıcısı |
| Python | debugpy |

Projenin `.vscode/launch.json` dosyası okunur. Hata Ayıkla menüsü Xcode’un kestirmelerini kullanır: ⌘\\ bir kesme noktası koyar, ⌘Y hepsini açar ya da kapatır, ⌃⌘Y devam eder, F6, F7 ve F8 üzerinden, içine ve dışına adımlar, ⌘. durdurur.

## Terminal

Giriş kabuğunun ortamıyla sekmelerde (⌃\`) çalışan, Metal ile çizilen terminaller. ⌘K ekranı temizler. Yapıştırılan ya da bırakılan bir görüntü terminalde çalışan bir kodlama ajanına verilir ya da kaydedilmiş bir kopyanın yolu yapıştırılır. ⌘-tıklama bağlantıları ve dosya adlarını açar. Terminalin başlattığı bir geliştirme sunucusu tarayıcı sekmesinde açılabilir.

## Tarayıcı

Adres alanı (⌘L), geri ve ileri (⌘[ ve ⌘]), yeniden yükleme (⌘R), arama, önce soru soran indirmeler ve Web Denetçisi içeren bir tarayıcı sekmesi. `@browser` sayfayı dışarıdan gelen metin olarak işaretleyip bir ajana verir.

## Git

Git paneli dosyaları ya da tek tek parçaları hazırlar ve atar, commit eder, commit edip push eder ya da eşzamanlar, dalları değiştirir ve oluşturur, stash’leri tutar, birleştirme çakışmalarını çözer ve son commit’lerin grafiğini çizer. Blame, imlecin bulunduğu satırı en son kimin değiştirdiğini ya da tüm dosya için bir sütun gösterir. Bir düğme commit iletisini AI ile yazar; diff ancak gitleaks içinde gizli bilgi bulamadıktan sonra gönderilir ve her commit de ayrıca gizli bilgiler için denetlenir.

## CI

GitHub Actions ve GitLab CI, GitHub Enterprise Server ve kendi sunucunuzdaki GitLab dahil:

- geçerli dalın çalıştırmaları, işleri ve günlükleri;
- başarısız işleri, tek bir işi ya da GitHub’da tüm bir çalıştırmayı yeniden çalıştırma; elle başlatılan işleri başlatma ya da bekleyen dağıtımları onaylama; yeni bir pipeline ya da elle başlatılabilen bir iş akışını başlatma; iptal etme;
- tarayıcı sekmesinde açılan açık pull request’ler ya da merge request’ler ve issue’lar;
- dışarıdan gelen metin olarak işaretlenip bir ajana verilen, başarısız bir işin günlüğü.

Token, Anahtar Zinciri’nden, `gh` ya da `glab`’dan, giriş kabuğunun ortamından gelir ya da **Ayarlar › CI** içinde sizin girdiğiniz token’dır.

## Container’lar

Container’lar paneli Docker’ın Compose projelerini servisleriyle birlikte, tek başına container’ları, imajları ve volume’ları gösterir ve Docker’ın olaylarını gerçekleştikleri anda izler. Klasördeki bir Compose dosyası, container’larından hiçbiri henüz yokken bile proje olarak görünür. Container’lar başlatılır, durdurulur ve yeniden başlatılır; günlükleri ve bir kabuk terminal sekmelerinde, yayımlanan bir port ise tarayıcı sekmesinde açılır. Bir container’ı, imajı ya da volume’u silmek ve kullanılmayan verileri temizlemek önce onay ister. Compose, projenin tarif ettiğini derleyip çalıştırdığından Compose’un up, down ve pull komutları yalnızca güvenilen klasörlerde çalışır. Panelin menüsü, kabuklarınızın kullandığı bağlamı değiştirmeden pencere için Docker bağlamını seçer; aynı komutlar Container’lar menüsünde de bulunur. Panel için `docker` komut satırı ve Docker 24.0.6 veya daha yenisi gerekir.

## Markdown

- **Önizleme.** Bir Markdown dosyası, önizlemesi yanında olacak şekilde açılır; **Görüntü › Önizlemeyi Yanda Göster/Gizle** (⇧⌘V) onu kapatır ve açar, **Ayarlar › Genel** de otomatik önizlemeyi kapatır. Önizleme formülleri, Mermaid, Graphviz ve PlantUML diyagramlarını ve klasördeki resimleri çizer; web’deki resimler yalnızca güvenilen bir klasörde ya da siz önizlemede istedikten sonra yüklenir.
- **Biçim menüsü:** kalın (⌘B), italik (⌘I), üstü çizili (⇧⌘X), kod (⌥⇧⌘C), bağlantı (⇧⌘K), başlık 1–3 (⌥⌘1–⌥⌘3), madde işaretli ve numaralı listeler, görevler, alıntılar ve kod blokları. Her komut seçimde ya da satırlarda açılıp kapanır ve tek bir geri alma adımıdır.
- **Yapıştırma.** Word’den ya da bir web sayfasından gelen metin Markdown olarak, resimleri de belgenin yanında dosyalar olarak gelir; ⌥⇧⌘V düz metin yapıştırır.
- **Dışa aktarma.** **Dosya › Dışa Aktar** PDF, resimleri içine gömülü tek bir HTML dosyası ya da bir Word belgesi yazar. Web’deki resimler yalnızca güvenilen bir klasörden dışa aktarılır.
- **Splat’te açma.** [Splat](https://github.com/sventmueller/splat) kuruluysa, **Dosya › Splat’te Aç** ile dosya ağacının ve sekmelerin bağlam menüleri belgeyi kaydettikten sonra orada açar.

## Dikte

Bir AI sekmesinin ileti alanına ya da düzenleyiciye dikte edin: oradaki mikrofon düğmesiyle, **Düzen › Dikteyi Başlat** (⌃⌥D) ile ya da fn tuşuyla (kayıt için basılı tutun, kayda devam etmek için iki kez dokunun). Konuşma tanıma [Lowspeak](https://github.com/sventmueller/lowspeak)’inkidir; Lowspeak’in kendisinin kurulu olması gerekmez.

- **Ayarlar › Dikte** modu ve modeli (OpenAI ya da Groq ile çevrimiçi, Whisper ya da Parakeet ile yerel), dili, son işlemeyi, mikrofonu ve kendi sözcük dağarcığınızı belirler.
- **Yerel modeller,** birini seçtiğinizde Hugging Face’ten ilerleme gösterilerek ve iptal edilebilir şekilde indirilir. Lowspeak’in kullandığı klasörlere kaydedilir; böylece her iki uygulama da diğerinin indirdiğini kullanır. Yerel bir model kaydı bu Mac’te tutar.
- **Çevrimiçi yazıya dökme** bir OpenAI ya da Groq anahtarı gerektirir; Dango bunu kendi Anahtar Zinciri öğesinde tutar.
- **Lowspeak’ten.** Lowspeak varsayılanlardan farklı ayarlanmışsa, Dikte sekmesi ayarlarını bir kez aktarmayı, isterseniz anahtarlarını da almayı önerir; macOS, Dango bir anahtarı okumadan önce sorar (“Her Zaman İzin Ver” yerine “İzin Ver” düğmesini seçin).
- **İzinler.** macOS ilk dikte sırasında mikrofon için izin ister. fn tuşu, Dango etkin uygulamayken ve Lowspeak çalışmıyorken çalışır; Erişilebilirlik izni gerektirir ve Dango bu izni açılışta asla değil, ilk dikteden sonra ister.

## Güven

Bir klasör, içinde bir şey başlatılır başlatılmaz programların çalışmasına yol açabilir: Git hook’ları ve `core.fsmonitor` gibi depo ayarları, `node_modules` içinden dil sunucuları, ajan hook’ları, `.mcp.json`. Bir klasörü ilk açtığınızda Dango o klasöre güvenip güvenmediğinizi sorar.

- **Güvenilen:** Git, dil sunucuları, hata ayıklayıcılar, ajanlar ve CI bu klasör ve altındaki her şey için çalışır.
- **Kısıtlı:** okuyabilir ve düzenleyebilirsiniz; düzenleyicideki değişiklik işaretleri doğrudan `.git` okunarak elde edilir. Klasörün yapılandırdığı hiçbir şey başlatılmaz. Git, CI ve AI panelleri bunu belirtir ve klasöre güvenmeyi önerir.

Sizden gelmeyen metin, bir ajanı yönlendirmeye yönelik talimatlar taşıyabilir (istem enjeksiyonu). Bu yüzden bir terminalin çıktısı (`@terminal`), bir tarayıcı sayfası (`@browser`), bir ajana verilen CI günlüğü, proje dışındaki bir dosya ve bir web sayfasından yapıştırılan uzun metin işaretlenerek gönderilir: etikette “Dış İçerik” yazar ve ajan metni, talimatlarına uymaması gereken güvenilmeyen veri olduğu notuyla alır. Yerleşik ajanın web sayfaları da aynı notla gelir. Yapıştırılan metin, tarayıcı bunu panoda belirttiğinde web sayfasından gelmiş sayılır; Safari, Chrome ve Firefox bunu yapar. Başka yerlerden gelen metin ve birkaç sözcüklük kısa bir ifade sizin metniniz olarak kalır.

## Korumalı alan

Dango’nun kendisi App Sandbox olmadan çalışır, çünkü başlattığı her program onu devralırdı: terminaller, Git, dil sunucuları ve ajanlar çalışamazdı. Bunun yerine her AI sohbetinin varsayılan olarak kapalı bir korumalı alan anahtarı vardır. Neyi sınırladığı ajana bağlıdır ve anahtarın araç ipucu bunu söyler:

| Ajan | Korumalı alan açıkken |
|---|---|
| Claude | kabuk komutları Claude’un kendi korumalı alanında çalışır; dosya ve web araçları ise orada çalışmaz |
| Codex | komutlar ve değişiklikler yalnızca projeye, ağ olmadan yazar |
| Yerleşik ajan | dosyalar, web istekleri ve komutlar proje ve izin verilen klasörler içinde, ağ olmadan kalır (`sandbox-exec`) |
| Antigravity, OpenCode | Dango’nun açabileceği bir korumalı alan yok |

## Görünüm ve erişilebilirlik

- **Koyu mod** sistemi izler; **Karşıtlığı Artır** ve **Hareketi Azalt** dikkate alınır.
- **VoiceOver** kenar çubuğunu, sekmeleri, AI zaman çizelgesinin kartlarını, Git ve CI satırlarını, hata ayıklayıcıyı ve terminal metnini adlandırır. Kesme noktaları hiçbir zaman yalnızca renkle ayırt edilmez.
- **Klavye.** Her komutun bir kestirmesi vardır ve hepsi `keybindings.json` içinde değiştirilebilir; ⌥⌘\` sonraki panele geçer.
- **Sağdan sola.** Arapça ve İbranicede pencere aynalanır, kod görünümleri ise soldan sağa kalır.

Dango’nun AppKit’in üzerine alan alan neler eklediği [ACCESSIBILITY.md](../../../ACCESSIBILITY.md) dosyasındadır.

## Diller

Arayüz 35 dile çevrilmiştir: Arapça, Katalanca, Çince (Basitleştirilmiş), Çince (Geleneksel), Hırvatça, Çekçe, Danca, Felemenkçe, İngilizce, Fince, Fransızca, Almanca, Yunanca, İbranice, Hintçe, Macarca, Endonezce, İtalyanca, Japonca, Korece, Malayca, Norveççe, Lehçe, Portekizce (Brezilya), Portekizce (Portekiz), Romence, Rusça, Slovakça, Slovence, İspanyolca, İsveççe, Tayca, Türkçe, Ukraynaca ve Vietnamca.

Dango, **Sistem Ayarları › Genel › Dil ve Bölge** içindeki dil sırasını izler; orada **Uygulamalar** altından yalnızca Dango için de bir dil seçebilirsiniz. Dilinizde çeviri yoksa İngilizceyi kullanır. Terimler tek bir sözlüğü, [docs/glossary.md](../../../docs/glossary.md) dosyasını, ve Apple’ın kendi çevirilerini izler; böylece Dango Mac’inizin geri kalanı gibi okunur.

## Klavye kestirmeleri

| Eylem | Kestirme |
|---|---|
| Klasör aç, yeni pencere, pencereyi kapat | ⌘O, ⇧⌘N, ⇧⌘W |
| Yeni dosya, kaydet, farklı kaydet | ⌘N, ⌘S, ⇧⌘S |
| Yeni sekme, sekmeyi kapat, sonraki ve önceki sekme | ⌘T, ⌘W, ⇧⌘] ve ⇧⌘[ |
| Hızlı Aç, projede ara | ⌘P, ⇧⌘F |
| Dosyalar, Arama, Ajanlar, Git, Testler, Hata Ayıkla, CI, Container’lar | ⌘1–⌘8 |
| Kenar çubuğunu göster ya da gizle, sonraki panel | ⌃⌘S, ⌥⌘\` |
| Düzenlenen yerler arasında geri ve ileri | ⌃- ve ⌃⇧- |
| Yorumu aç/kapat, satıra git | ⌘/, ⌃G |
| Sonraki eşleşme, yukarıda ve aşağıda imleç | ⌘D, ⌃⇧↑ ve ⌃⇧↓ |
| Katla, aç, tümünü katla, tümünü aç | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Satır kaydırma, Denetçi | ⌥Z, ⌥⌘I |
| Tamamlama, üzerine gelince bilgi, tanıma git | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Başvurular, çağrı hiyerarşisi, yeniden adlandır | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Dosyadaki, projedeki semboller | ⇧⌘O, ⌥⌘O |
| Hızlı düzeltme, biçimlendir | ⌥⌘A, ⌥⇧F |
| Sonraki ve önceki sorun | ⌘' ve ⇧⌘' |
| Markdown önizlemesi, düz metin olarak yapıştır | ⇧⌘V, ⌥⇧⌘V |
| Kalın, italik, üstü çizili, kod, bağlantı | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Başlık 1–3 | ⌥⌘1–⌥⌘3 |
| Yeni terminal, terminali temizle | ⌃\`, ⌘K |
| Ajanın ileti alanına odaklan, bir plana yorum yap | ⌘Esc, ⌥⌘K |
| Adres alanı, yeniden yükle, geri, ileri (tarayıcı) | ⌘L, ⌘R, ⌘[, ⌘] |
| Dikteyi başlat ya da durdur | ⌃⌥D |
| Kesme noktası, tüm kesme noktalarını aç ya da kapat | ⌘\\, ⌘Y |
| İmleç konumundaki testi çalıştır, son testleri yeniden çalıştır | ⌃⌥⌘U, ⌃⌥⌘G |
| Devam et, üzerinden, içine, dışına adımla, durdur | ⌃⌘Y, F6, F7, F8, ⌘. |
| Ayarlar | ⌘, |

Uygulamanın veri klasöründeki `keybindings.json` (bkz. [Verilerin bulunduğu yerler](#verilerin-bulunduğu-yerler)) bunların herhangi birini değiştirir.

## Gizlilik ve güvenlik

- **Telemetri yok;** analiz yok, hiçbir yere çökme raporu gönderilmez. Hatalar sistem günlüğüne gider (Console.app, uygulamanın adı altında).
- **Ağ yalnızca sizin başlattıklarınız için:** kullandığınız ajanlar, her biri kendi hizmetiyle konuşur; istediğinizde ACP kayıt defteri ve ajanların, dil sunucularının, hata ayıklama bağdaştırıcılarının ve dikte modellerinin indirmeleri; giriş yaptığınız CI sunucuları; seçtiyseniz Copilot’tan düzenleme tahminleri; yerleşik ajanın sağlayıcıları; seçtiyseniz çevrimiçi yazıya dökme; tarayıcı sekmesinde açtığınız sayfalar; ve izin verdikten sonra güncelleme denetimi.
- **Kod Mac’ten** yalnızca sizin bir hizmet seçtiğiniz yerde çıkar: bir ajan, Copilot’un tahminleri, AI ile yazılan bir commit iletisi. Commit iletisi için diff ancak gitleaks içinde gizli bilgi bulamadıktan sonra gönderilir.
- **Anahtarlar ve token’lar** macOS Anahtar Zinciri’nde kalır, asla bir ayar dosyasında değil. Lowspeak’inki gibi başka uygulamaların anahtarları yalnızca siz kabul ettiğinizde ve yalnızca bir kez okunur.
- **Sabitlenmiş indirmeler.** Dango’nun kurduğu ajanlar, dil sunucuları ve hata ayıklama bağdaştırıcıları test edilmiş sürümlere sabitlenir ve sağlama toplamlarıyla doğrulanır.
- **Kod imzalama** Hardened Runtime ile; güncellemeler yalnızca uygulamanın kendi imzasıyla kurulur.

## Verilerin bulunduğu yerler

`<id>` paket kimliğidir (bundle identifier): sürüm için `local.agentide` (hata ayıklama derlemeleri için `local.agentide.debug`). Görünen adı içermez; bu yüzden ad değişikliğinde tüm veriler korunur.

| Yer | İçerik |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (AI panelinin listesi), `agent-threads/` (yerleşik ajan konuşmaları), `layouts/` (proje başına sekmeler), `recovery/` (çökme sonrasında kaydedilmemiş düzenleyiciler), `checkpoints/` (Git kullanmayan klasörler için), `worktrees/`, `agents/` ve `tools/` (kurulu ajanlar, dil sunucuları ve hata ayıklama bağdaştırıcıları), `task-output/` |
| `~/Library/Caches/<id>/` | yapıştırılan görüntüler, kabuk entegrasyonu betikleri, ACP kayıt defteri; silinebilir |
| `~/Library/Logs/<id>/dictation/` | diktenin günlük dosyaları |
| `~/Library/Application Support/Lowspeak` ve `FluidAudio` | Lowspeak ile paylaşılan yerel dikte modelleri |
| `~/Library/WebKit/<id>/` | tarayıcı sekmesinin çerezleri ve web sitesi verileri |
| `<project>/.dango/settings.json` | projenin kendi ayarları |
| projenin Git deposu | `refs/ide-checkpoints/` altında commit’ler olarak kontrol noktaları; normal bir push ya da fetch bunları asla taşımaz |
| Anahtar Zinciri, hizmet `<id>` | yerleşik ajanın ve diktenin API anahtarları, CI token’ları |
| `~/.local/bin/dango` | kurduysanız terminal komutu |

Claude kendi oturumlarını `~/.claude/projects` altında tutar; Dango bir oturumu sürdürmek için bunları okur ama değiştirmez.

## Bilinen sınırlamalar

- **Uygulamanın kendisi için App Sandbox yok,** çünkü terminaller, Git, dil sunucuları ve ajanlar onu devralırdı; bir klasörün yapılandırdığı herhangi bir şey çalışmadan önce o klasöre güvenilmesi gerekir. Antigravity ve OpenCode’un, Dango’nun açabileceği bir korumalı alanı yoktur.
- **Claude’un kullanım rakamları ve bağlam dökümü,** protokolünün deneysel olarak işaretlenmiş kısımlarına dayanır ve bunlar yoksa gizlenir.
- **Konuşmayı geri sarma** yalnızca Claude’da çalışır; diğer ajanlarda yalnızca kod geri sarılır.
- **Tarayıcı üzerinden CI’ya giriş** kapalıdır; `gh`, `glab`, bir token ya da ortamı kullanın.
- **Gemini CLI sunulmuyor:** Google kişisel hesapla girişi sonlandırdı ve Antigravity’ye yönlendiriyor. Önceki Gemini sohbetleri okunabilir kalır.

## Kaynaktan derleme

Gerekenler:

- Xcode Command Line Tools (`xcode-select --install`) ile macOS 14 veya üstü; Xcode’un kendisi gerekmez.
- `rust-toolchain.toml` dosyasının belirttiği Rust. Homebrew’un `rust`’ı bu Mac’in işlemcisi için derler; bir sürümün evrensel derlemesi `aarch64-apple-darwin` ve `x86_64-apple-darwin` hedefleriyle rustup gerektirir.
- Bu deponun yanında [splat](https://github.com/sventmueller/splat) ve [lowspeak](https://github.com/sventmueller/lowspeak) kopyaları (`../splat`, `../lowspeak`), ikisi de `main` dallarında; çalışma alanı onlardan crate’leri yol üzerinden alır.
- Dikte içeren her derleme (`make run`, `make build`, `make release`) için whisper.cpp’yi derleyen cmake. `make check` onsuz derler.

| Komut | Ne yapar |
|---|---|
| `make run` | hata ayıklama uygulamasını (`target/debug/Dango Debug.app`) derler ve açar |
| `make build` | sürüm uygulamasını (`target/release/Dango.app`) derler; ilk derlemenin giriş anahtar zincirinde oluşturduğu yerel bir sertifikayla imzalanır; `VERSION` değerini değiştirmez |
| `make lint` | biçimlendirme ve clippy; uyarılar başarısızlığa yol açar |
| `make check` | `lint`, ardından tüm testler; anahtar zinciri, pencere ya da ağ gerektirmez |
| `make test` | `check`, ardından gerçek uygulamada soldan sağa ve sağdan sola `probe` öz testi ve erişilebilirlik denetimi |
| `make perf` | bir sürüm derlemesinde zamanlama bütçeleri; yalnızca başka bir iş yapmayan bir Mac’te anlamlıdır |
| `make release` | yerel sertifikayla imzalanmış, `target/release-dry-run/` içine sürümün bir deneme çalıştırması; hiçbir yere bir şey göndermez |
| `make release-notarized` | `check`, ardından `dist/` içine sürümün kendisi: Developer ID imzası, onaylama (notarization), uygulamanın kendi anahtarıyla imzalanmış güncelleme akışı; `VERSION` değerini değiştirmez |
| `make deploy` | temiz ve yeşil CI mührüne sahip `main` üzerinde sonraki sürümü onaylatır, `VERSION` değerini güncelleyip commit eder, sürüm dosyalarını yükler ve kodu, CI mührünü ve sürüm etiketini push eder; GitHub sürümü otomatik olarak yayımlar |

Hata ayıklama ve sürüm derlemeleri kendi ayarları ve verileri olan ayrı uygulamalardır. Derleme, test etme, öz testler, sürüm yayımlama ve kodun izlediği kurallar [docs/development.md](../../../docs/development.md) dosyasında, arkasındaki kararlar [docs/plan.md](../../../docs/plan.md) dosyasında anlatılır. Sürüm notları [RELEASE_NOTES.md](../../../RELEASE_NOTES.md) dosyasındadır ve her README’nin yanında `assets/readme/<language>/` içinde çevrilmiştir.

## Lisans

Dango, aşağıdakilerden herhangi birinin altında lisanslanmıştır

- Apache Lisansı, Sürüm 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- MIT Lisansı ([LICENSE-MIT](../../../LICENSE-MIT))

tercihinize bağlı olarak.

Siz açıkça başka türlü belirtmedikçe, Apache-2.0 lisansında tanımlandığı üzere Dango’ya dahil edilmek üzere sizin tarafınızdan kasıtlı olarak sunulan her katkı, ek hüküm ya da koşul olmaksızın yukarıdaki gibi çift lisanslı olacaktır.
