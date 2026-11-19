## Hız ve bellek
- Bir ajanın başlattığı her şey, hangi dilde olursa olsun, örneğin derlemeler, testler, kabuklar ve MCP sunucuları,
  macOS’in arka plan durumunda çalışır: en düşük öncelikle ve kısıtlanmış disk ve ağ erişimiyle. Ajanların kendileri,
  Claude’un alt ajanlarıyla birlikte, orta öncelikle çalışır. Tam yük altındaki ajanlar artık Dango’yu yavaşlatmaz; bu,
  Software Factory’nin ajanları için de geçerlidir.
- Geri yüklenen pencereler yalnızca görüneni başlatır: diğerlerinin arkasındaki bir AI sekmesi CLI’ını öne geldiğinde ya
  da bir istem aldığında başlatır; tarayıcı, site önizlemesi ve terminal sekmeleri ise sayfalarını ilk kez
  göründüklerinde yükler ya da kabuklarını o zaman başlatır.
- Uzun süren bir proje araması, çalışırken artık tüm sonuçlarını yeniden çizmez. Software Factory, CI günlükleri ve
  Testler paneli görünmezken daha az iş yapar; testler ve hata ayıklama yapılandırmaları panelleri ilk kez göründüğünde
  okunur, gizli CI ve Container’lar panelleri ise görünene kadar hiçbir şey okumaz.
- Uzun oturumlar daha az bellek kullanır: tamamlanmış bir fark yalnızca kartının gösterdiği satırları tutar, yanıtlanmış
  bir izin isteği girdisini serbest bırakır, terminalin glif önbelleği artık sınırsızca büyümez ve Dosyalar ile Git
  panelleri artık var olmayan satırları serbest bırakır.
- Programın kendisi yaklaşık %40 daha küçük.

## Pencereler ve menüler
- Bir bölmeyi kapatmak artık sekmelerini kapatmaz; sekmeler önceki bölmeye, o yoksa sonraki bölmeye taşınır. Yalnızca
  bir önizlemenin bölmesi ve son bölme sekmeleriyle birlikte kapanır.
- Görüntü menüsü, panelleri kenar çubuğundaki simgelerin sırasıyla listeler.
- “Ajanları Yönet…” artık + menüsünde değil; Ajanlar panelinde “+ Yeni Oturum” altında yer almaya devam ediyor.

## Düzeltmeler
- Arama panelindeki “Dosyaları Dahil Et ve Hariç Tut”, okunun yanında artık yersiz bir harf göstermiyor.
- Bir belirtimin yorum açılır penceresini kapatmak artık çökmeye yol açmıyor.
