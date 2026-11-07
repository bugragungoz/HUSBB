<p align="center">
  <img src="assets/icon.svg" width="96" alt="HUSBB logosu: koyu osiloskop ekranında yeşil bir sinüs dalgası">
</p>

<h1 align="center">HUSBB</h1>
<p align="center"><b>Hantek DSO2D15 USB Bridge</b> - DSO2D15'in ekranı bilgisayarınızda, iki kanal birlikte, canlı.</p>

<p align="center">
  <a href="README.md">English</a> |
  <b>Türkçe</b> |
  <a href="README.zh-CN.md">简体中文</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.de.md">Deutsch</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.ru.md">Русский</a>
</p>

<p align="center"><a href="https://github.com/bugragungoz/HUSBB/releases/latest"><b>HUSBB.exe'yi indir</b></a></p>

![Hantek DSO2D15'in CH1 ve CH2 kanallarını gösteren HUSBB](docs/screenshot-scope.png)

HUSBB, **Hantek DSO2D15** osiloskobu için yazılmış yerel bir Windows uygulamasıdır (Rust, wgpu
üzerinde egui). İki kanalı USB üzerinden aynı edinimden okur ve osiloskobun çizdiği gibi çizer;
ölçümler, gerçek örnekleri okuyan imleçler, işaretçiler, ekranı dondurma, tek tıkla dışa aktarma ve
osiloskobun kontrolleriyle birlikte - Hantek'in PC yazılımına gerek kalmadan. Yalnızca DSO2D15 için
yazıldı ve yalnızca DSO2D15 üzerinde denendi.

## Neden var

**3.0.1 (2025-04-18)** firmware'inde Hantek'in DSO2D15 için kendi PC yazılımı hiçbir şey
göstermiyor: `PRIVate:WAVeform:DATA:ALL?` komutunu soruyor, bu firmware ise artık buna cevap
vermiyor. Belgelenmiş `:WAVeform:DATA:ALL?` cevap veriyor, ama iki kanal açıkken tek kanalı iki
kez gönderiyor.

DSO2D15'in firmware'i tersine mühendislikle incelenince `:WAVeform:DATA:DISP?` komutu ortaya çıktı:
açık olan her kanalı, aynı edinimin kendi tamponundan veriyor - tam olarak osiloskobun o an çizdiği
kayıt. HUSBB bunu kullanıyor; bu yüzden kanal açıp kapatma yok, kayıp örnek yok (kanal başına
ekrandaki 3850 noktanın hepsi) ve CH1 ile CH2 arasındaki zamanlama gerçek. Firmware ayrıca PC bağlıyken
ön panelin neden kilitlendiğini ve kilidin nasıl geri verileceğini de gösterdi. Ayrıntılar:
[docs/PROTOCOL.md](docs/PROTOCOL.md) (İngilizce).

## Özellikler

- **İki kanal birlikte, canlı**, osiloskobun kendi renkleriyle (CH1 sarı, CH2 yeşil); saniyede
  yaklaşık 1,5 yeni kayıt - DSO2D15'in USB üzerindeki kendi sınırı.
- **Live link anahtarı.** Kapalı: HUSBB hiçbir şey göndermez, DSO2D15 yaklaşık 5 saniye sonra ön
  panel kilidini açar ve tuşlarını kullanabilirsiniz. Açık: HUSBB kontrolü yeniden alır.
- **Kontroller:** run / stop, single, auto setup, kanal açma/kapama, V/div, konum, time/div, tetik
  kaynağı, kenar, mod ve seviye. Her değişiklik osiloskoptan geri okunur.
- Kanal başına **View**: bir izi yalnızca HUSBB'de göster veya gizle.
- Kanal başına **ölçümler**: frekans, periyot, doluluk oranı, Vpp, Vmax, Vmin, Vavg, Vrms; istenirse
  ekranın üzerine de yazılır.
- Gerçek örneklere oturan ve iki kanalı A ile B noktalarında okuyan **Track imleçleri** (dt, 1/dt,
  dV) ve gerilim imleçleri.
- **İşaretçiler:** bir örneği işaretlemek için ize çift tıklayın (zaman ve gerilim); silmek için sağ
  tıklayın.
- **Hold**, osiloskop çalışmaya devam ederken ekranı dondurur.
- Ayarlarda seçilen klasöre tek tıkla **Export** (ekranın PNG'si ve/veya verinin CSV'si) ya da
  istediğiniz yere **Save as...**.
- Fareyle zaman ve gerilim okuma, kendiliğinden yeniden bağlanma, ayarlarınızı hatırlama.

![Measure sekmesi: ölçüm tablosu ve gerçek örneklere oturan Track imleçleri](docs/screenshot-measure.png)

## İndirme

Releases'ten [**HUSBB.exe**](https://github.com/bugragungoz/HUSBB/releases/latest): tek dosya,
kurulum yok. Yanında yalnızca bir VISA gerekir (aşağıya bakın). Exe kod imzalı olmadığı için
Windows SmartScreen bir kez sorabilir: *Ek bilgi* -> *Yine de çalıştır*.

## Gereksinimler

- Windows 10 veya 11, 64 bit ve bir Hantek DSO2D15 (firmware 3.0.1(250418.00) üzerinde denendi).
- Osiloskobun Aygıt Yöneticisi'nde *USB Test and Measurement Devices* altında görünmesi için
  USBTMC destekli bir VISA: Keysight IO Libraries veya R&S VISA (R&S VISA 7.2 ve IVI `ausbtmc`
  sürücüsüyle denendi). HUSBB `visa64.dll` dosyasını çalışma anında yükler.
- Osiloskobun **arka** USB-B portu.

### DSO2D15'in bağlantısı kopuyorsa

Bütün portları USB 3.x olan bir laptopta DSO2D15, 30 saniye ile 5 dakika arasında bir sürede
USB'den düştü - hiç veri trafiği yokken ve pil gücündeyken bile (Windows, Kernel-PnP 1010 olayını
kaydeder: *"surprise removed as it is reported as missing on the bus"*). **Laptop ile osiloskop
arasına basit bir USB 2.0 hub koymak sorunu çözdü.** `tools/idle_watch.ps1` bu testi tekrarlar.

## Derleme ve çalıştırma

[Rust](https://rustup.rs) ile (stable, MSVC araç zinciri):

```bash
cargo build --release
```

```bash
target\release\HUSBB.exe
```

Komut satırı seçenekleri, çoğunlukla otomasyon için:

```
--resource <VISA adı>      ilk bulunan yerine bu osiloskobu kullan
--screenshot <dosya.png>   --after saniye sonra (varsayılan 8) pencereyi kaydet ve çık
--cursors track|voltage    imleçler açık başla
--tab measure              Measure sekmesinde başla
```

Donanım testleri (DSO2D15 bağlıyken):

```bash
cargo run --example probe
```

```bash
cargo run --example link
```

Birim testleri gerçek bir DSO2D15'ten kaydedilmiş paketlerle çalışır (`tests/data`):

```bash
cargo test
```

## Dosya düzeni

| | |
|---|---|
| `src/visa.rs` | çalışma anında yüklenen VISA bağlantısı |
| `src/protocol.rs` | `:WAVeform:DATA:DISP?` akış ayrıştırıcısı |
| `src/scope.rs` | DSO2D15 komutları ve ayarları |
| `src/acquire.rs` | edinim iş parçacığı, live link, yeniden bağlanma |
| `src/measure.rs` | ölçümler, SI biçimlendirme |
| `src/ui/` | pencere, osiloskop ekranı, imleçler, işaretçiler, kontroller ("Bugra" tasarım sistemi) |
| `docs/PROTOCOL.md` | firmware analizinin bulguları |

## Nasıl yapıldı

HUSBB **Claude Opus 5.5** ile geliştirildi. DSO2D15'in protokolü, osiloskobun orijinal firmware
güncellemesi ve Hantek'in PC yazılımı tersine mühendislikle analiz edilerek çıkarıldı - yalnızca
dosyalar üzerinde statik analiz; osiloskoba hiçbir şey yazılmadı veya yüklenmedi.

## Teşekkürler

Önceki protokol çalışmaları: [phmarek/hantek-dso2000](https://github.com/phmarek/hantek-dso2000) ve
[EEVblog DSO2000 başlığı](https://www.eevblog.com/forum/testgear/hantek-dso2x1x-models-master-thread/).
Yazı tipleri: [Geist](https://github.com/vercel/geist-font) ve
[JetBrains Mono](https://github.com/JetBrains/JetBrainsMono), ikisi de SIL Open Font License 1.1
altında (`assets/fonts/OFL-*.txt`). Hantek ile bir bağı yoktur; bu depoda Hantek'e ait hiçbir yazılım
veya firmware bulunmaz.

## Lisans

[MIT](LICENSE)
