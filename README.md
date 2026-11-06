<p align="center">
  <img src="assets/icon.svg" width="96" alt="HUSBB logo: a green sine wave on a dark scope screen">
</p>

<h1 align="center">HUSBB</h1>
<p align="center"><b>Hantek DSO2D15 USB Bridge</b> - the DSO2D15's screen on your PC, both channels, live.</p>

![HUSBB showing CH1 and CH2 of a Hantek DSO2D15](docs/screenshot-scope.png)

> Türkçe özet aşağıda.

HUSBB is a native Windows app (Rust, egui on wgpu) for the **Hantek DSO2D15** oscilloscope. It
reads both channels from the same acquisition over USB and draws them like the scope does, with
measurements, cursors that read real samples, markers, hold, one-click export and the scope's
controls - without Hantek's PC software. It was written for and tested on the DSO2D15 only.

## Why it exists

On firmware **3.0.1 (2025-04-18)** Hantek's own PC software for the DSO2D15 shows nothing: it asks
for `PRIVate:WAVeform:DATA:ALL?`, which this firmware no longer answers. The documented
`:WAVeform:DATA:ALL?` answers, but with both channels on it sends one channel twice.

Reverse engineering the DSO2D15's firmware turned up `:WAVeform:DATA:DISP?`: every channel that is
on, each from its own buffer of the same acquisition - exactly the record the scope is drawing.
HUSBB uses it, so there is no channel switching, no lost samples (all 3850 on-screen points per
channel) and real timing between CH1 and CH2. The firmware also showed why the front panel locks
while a PC is connected, and how to give it back. Details: [docs/PROTOCOL.md](docs/PROTOCOL.md).

## Features

- **Both channels, live**, in the scope's own colours (CH1 yellow, CH2 green), about 1.5 new
  records per second - the DSO2D15's own limit on USB.
- **Live link switch.** Off: HUSBB sends nothing and the DSO2D15 unlocks its front panel about 5 s
  later, so you can use its keys. On: HUSBB takes over again.
- **Controls:** run / stop, single, auto setup, channel on/off, V/div, position, time/div, trigger
  source, slope, mode and level. Every change is read back from the scope.
- **View** per channel: show or hide a trace in HUSBB only.
- **Measurements** per channel: frequency, period, duty, Vpp, Vmax, Vmin, Vavg, Vrms, also written
  on the screen if you like.
- **Track cursors** that snap to real samples and read both channels at A and B (dt, 1/dt, dV), and
  voltage cursors.
- **Markers:** double-click a trace to mark a sample (time and voltage); right-click to remove.
- **Hold** freezes the screen while the scope keeps running.
- **Export** in one click to a folder chosen in Settings (PNG of the screen and/or CSV of the
  data), or **Save as...** anywhere.
- Hover readout of time and voltage, reconnects by itself, remembers your settings.

![Measure tab: measurement table and track cursors on real samples](docs/screenshot-measure.png)

## Requirements

- Windows 10 or 11, 64-bit, and a Hantek DSO2D15 (tested on firmware 3.0.1(250418.00)).
- A VISA that handles USBTMC, so the scope shows up in Device Manager under *USB Test and
  Measurement Devices*: Keysight IO Libraries or R&S VISA (tested with R&S VISA 7.2 and the IVI
  `ausbtmc` driver). HUSBB loads `visa64.dll` at run time.
- The scope's **rear** USB-B port.

### If the DSO2D15 keeps disconnecting

On a laptop whose ports are all USB 3.x the DSO2D15 dropped off the bus after anything from 30 s
to 5 min - with no traffic at all, and on battery power too (Windows logs Kernel-PnP event 1010,
*"surprise removed as it is reported as missing on the bus"*). **A plain USB 2.0 hub between
laptop and scope fixed it.** `tools/idle_watch.ps1` reproduces the test.

## Build and run

```bash
cargo build --release
```

```bash
target\release\HUSBB.exe
```

Command-line options, mainly for automation:

```
--resource <VISA name>     use this scope instead of the first one found
--screenshot <file.png>    save the window after --after seconds (default 8) and quit
--cursors track|voltage    start with cursors shown
--tab measure              start on the Measure tab
```

Hardware checks (with the DSO2D15 connected):

```bash
cargo run --example probe
```

```bash
cargo run --example link
```

Unit tests run against packets recorded from a real DSO2D15 (`tests/data`):

```bash
cargo test
```

## Layout

| | |
|---|---|
| `src/visa.rs` | VISA binding, loaded at run time |
| `src/protocol.rs` | `:WAVeform:DATA:DISP?` stream parser |
| `src/scope.rs` | DSO2D15 commands and settings |
| `src/acquire.rs` | acquisition thread, live link, reconnects |
| `src/measure.rs` | measurements, SI formatting |
| `src/ui/` | window, scope screen, cursors, markers, controls ("Bugra" design system) |
| `docs/PROTOCOL.md` | what the firmware analysis found |

## How it was made

HUSBB was built with **Claude Opus 5.5**. The DSO2D15's protocol was recovered by reverse
engineering the scope's original firmware update and Hantek's PC software - static analysis of the
files only; nothing was written to or flashed onto the scope.

## Türkçe özet

HUSBB (Hantek DSO2D15 USB Bridge), Hantek DSO2D15 osiloskobunun ekranını USB üzerinden
bilgisayarda canlı gösteren yerel bir Windows uygulamasıdır (Rust + egui/wgpu). Firmware 3.0.1'de
Hantek'in kendi programı hiçbir şey göstermiyor, belgelenmiş `:WAVeform:DATA:ALL?` komutu da iki
kanal açıkken tek kanalı iki kez gönderiyor. **Claude Opus 5.5 ile osiloskobun orijinal firmware'i
ve Hantek'in PC yazılımı tersine mühendislikle analiz edilerek** (yalnızca dosyalar üzerinde
statik analiz; osiloskoba hiçbir şey yazılmadı) bulunan `:WAVeform:DATA:DISP?` komutu iki kanalı
aynı edinimden, kayıpsız ve kanal açıp kapatmadan veriyor; HUSBB bunu kullanıyor.

Özellikler: iki kanal birlikte (osiloskobun kendi renkleriyle), Live link anahtarı (kapatınca
osiloskobun tuş kilidi ~5 saniyede açılır), kanal başına göster/gizle, V/div, zaman tabanı ve tetik
ayarları, ölçümler, gerçek örneklere oturan imleçler, işaretçiler, ekranı dondurma (Hold), ayarlarda
seçilen klasöre tek tıkla dışa aktarma (PNG/CSV). Laptopta bütün portlar USB 3.x ise osiloskop
USB'den düşebiliyor; araya basit bir USB 2.0 hub koymak sorunu çözdü.

## Credits

Earlier protocol work: [phmarek/hantek-dso2000](https://github.com/phmarek/hantek-dso2000) and the
[EEVblog DSO2000 thread](https://www.eevblog.com/forum/testgear/hantek-dso2x1x-models-master-thread/).
Fonts: [Geist](https://github.com/vercel/geist-font) and
[JetBrains Mono](https://github.com/JetBrains/JetBrainsMono), both under the SIL Open Font License
1.1 (`assets/fonts/OFL-*.txt`). Not affiliated with Hantek; no Hantek software or firmware is
included in this repository.

## License

MIT
