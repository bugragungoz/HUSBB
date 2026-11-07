<p align="center">
  <img src="assets/icon.svg" width="96" alt="HUSBB-Logo: eine grüne Sinuswelle auf einem dunklen Oszilloskop-Bildschirm">
</p>

<h1 align="center">HUSBB</h1>
<p align="center"><b>Hantek DSO2D15 USB Bridge</b> - der Bildschirm des DSO2D15 auf dem PC, beide Kanäle, live.</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.tr.md">Türkçe</a> |
  <a href="README.zh-CN.md">简体中文</a> |
  <a href="README.ja.md">日本語</a> |
  <b>Deutsch</b> |
  <a href="README.es.md">Español</a> |
  <a href="README.ru.md">Русский</a>
</p>

<p align="center"><a href="https://github.com/bugragungoz/HUSBB/releases/latest"><b>HUSBB.exe herunterladen</b></a></p>

![HUSBB zeigt CH1 und CH2 eines Hantek DSO2D15](docs/screenshot-scope.png)

HUSBB ist eine native Windows-App (Rust, egui auf wgpu) für das Oszilloskop **Hantek DSO2D15**.
Sie liest beide Kanäle aus derselben Erfassung über USB und zeichnet sie wie das Oszilloskop
selbst, mit Messungen, Cursorn auf echten Abtastwerten, Markern, Hold, Export per Klick und den
Bedienelementen des Oszilloskops - ganz ohne Hantek-PC-Software. Geschrieben für und getestet nur mit
dem DSO2D15.

## Warum es das gibt

Mit der Firmware **3.0.1 (2025-04-18)** zeigt die PC-Software von Hantek für das DSO2D15 nichts an:
Sie fragt `PRIVate:WAVeform:DATA:ALL?` ab, worauf diese Firmware nicht mehr antwortet. Das
dokumentierte `:WAVeform:DATA:ALL?` antwortet zwar, schickt aber bei zwei eingeschalteten Kanälen
einen Kanal doppelt.

Das Reverse Engineering der DSO2D15-Firmware brachte `:WAVeform:DATA:DISP?` zutage: jeder
eingeschaltete Kanal, jeweils aus seinem eigenen Puffer derselben Erfassung - genau der Datensatz,
den das Oszilloskop gerade zeichnet. HUSBB nutzt diesen Befehl, deshalb gibt es kein Umschalten
zwischen Kanälen, keine verlorenen Abtastwerte (alle 3850 Bildschirmpunkte pro Kanal) und ein echtes
Zeitverhältnis zwischen CH1 und CH2. Die Firmware zeigte außerdem, warum die Frontplatte gesperrt
ist, solange ein PC verbunden ist, und wie man sie wieder freigibt. Details:
[docs/PROTOCOL.md](docs/PROTOCOL.md) (Englisch).

## Funktionen

- **Beide Kanäle, live**, in den Farben des Oszilloskops (CH1 gelb, CH2 grün), etwa 1,5 neue
  Datensätze pro Sekunde - das eigene USB-Limit des DSO2D15.
- **Live-link-Schalter.** Aus: HUSBB sendet nichts, und das DSO2D15 entsperrt seine Frontplatte nach
  etwa 5 s, sodass die Tasten wieder benutzbar sind. Ein: HUSBB übernimmt wieder.
- **Bedienung:** Run / Stop, Single, Auto Setup, Kanal ein/aus, V/div, Position, Time/div,
  Triggerquelle, Flanke, Modus und Pegel. Jede Änderung wird vom Oszilloskop zurückgelesen.
- **View** pro Kanal: eine Kurve nur in HUSBB ein- oder ausblenden.
- **Messungen** pro Kanal: Frequenz, Periode, Tastgrad, Vpp, Vmax, Vmin, Vavg, Vrms, auf Wunsch
  auch direkt im Bild.
- **Track-Cursor**, die auf echten Abtastwerten einrasten und beide Kanäle an A und B ablesen (dt,
  1/dt, dV), sowie Spannungscursor.
- **Marker:** Doppelklick auf eine Kurve markiert einen Abtastwert (Zeit und Spannung);
  Rechtsklick entfernt ihn.
- **Hold** friert das Bild ein, während das Oszilloskop weiterläuft.
- **Export** per Klick in einen in den Einstellungen gewählten Ordner (PNG des Bildschirms und/oder
  CSV der Daten) oder **Save as...** an einen beliebigen Ort.
- Zeit- und Spannungsanzeige unter dem Mauszeiger, automatische Wiederverbindung, gespeicherte
  Einstellungen.

![Measure-Reiter: Messtabelle und Track-Cursor auf echten Abtastwerten](docs/screenshot-measure.png)

## Download

[**HUSBB.exe**](https://github.com/bugragungoz/HUSBB/releases/latest) unter Releases: eine Datei,
kein Installer. Zusätzlich wird nur ein VISA benötigt (siehe unten). Die exe ist nicht signiert,
deshalb fragt Windows SmartScreen eventuell einmal nach: *Weitere Informationen* -> *Trotzdem
ausführen*.

## Voraussetzungen

- Windows 10 oder 11, 64 Bit, und ein Hantek DSO2D15 (getestet mit Firmware 3.0.1(250418.00)).
- Ein VISA mit USBTMC-Unterstützung, damit das Oszilloskop im Geräte-Manager unter *USB Test and
  Measurement Devices* erscheint: Keysight IO Libraries oder R&S VISA (getestet mit R&S VISA 7.2
  und dem IVI-Treiber `ausbtmc`). HUSBB lädt `visa64.dll` zur Laufzeit.
- Der **hintere** USB-B-Anschluss des Oszilloskops.

### Wenn das DSO2D15 immer wieder die Verbindung verliert

An einem Laptop, dessen Anschlüsse alle USB 3.x sind, fiel das DSO2D15 nach 30 s bis 5 min vom Bus -
ganz ohne Datenverkehr und auch im Akkubetrieb (Windows protokolliert Kernel-PnP-Ereignis 1010,
*"surprise removed as it is reported as missing on the bus"*). **Ein einfacher USB-2.0-Hub zwischen
Laptop und Oszilloskop hat das behoben.** `tools/idle_watch.ps1` reproduziert den Test.

## Bauen und starten

Mit [Rust](https://rustup.rs) (stable, MSVC-Toolchain):

```bash
cargo build --release
```

```bash
target\release\HUSBB.exe
```

Kommandozeilenoptionen, vor allem für Automatisierung:

```
--resource <VISA-Name>     dieses Oszilloskop statt des ersten gefundenen verwenden
--screenshot <datei.png>   Fenster nach --after Sekunden (Standard 8) speichern und beenden
--cursors track|voltage    mit eingeblendeten Cursorn starten
--tab measure              im Measure-Reiter starten
```

Hardwaretests (mit angeschlossenem DSO2D15):

```bash
cargo run --example probe
```

```bash
cargo run --example link
```

Die Unit-Tests laufen mit Paketen, die von einem echten DSO2D15 aufgezeichnet wurden (`tests/data`):

```bash
cargo test
```

## Aufbau

| | |
|---|---|
| `src/visa.rs` | VISA-Anbindung, zur Laufzeit geladen |
| `src/protocol.rs` | Parser für den `:WAVeform:DATA:DISP?`-Datenstrom |
| `src/scope.rs` | DSO2D15-Befehle und -Einstellungen |
| `src/acquire.rs` | Erfassungs-Thread, Live link, Wiederverbindung |
| `src/measure.rs` | Messungen, SI-Formatierung |
| `src/ui/` | Fenster, Oszilloskopbild, Cursor, Marker, Bedienelemente ("Bugra"-Designsystem) |
| `docs/PROTOCOL.md` | Ergebnisse der Firmware-Analyse |

## Wie es entstanden ist

HUSBB wurde mit **Claude Opus 5.5** gebaut. Das Protokoll des DSO2D15 wurde durch Reverse
Engineering des originalen Firmware-Updates und der Hantek-PC-Software ermittelt - ausschließlich
statische Analyse der Dateien; auf das Oszilloskop wurde nichts geschrieben oder geflasht.

## Danksagung

Frühere Protokollarbeit: [phmarek/hantek-dso2000](https://github.com/phmarek/hantek-dso2000) und der
[EEVblog-Thread zur DSO2000-Serie](https://www.eevblog.com/forum/testgear/hantek-dso2x1x-models-master-thread/).
Schriften: [Geist](https://github.com/vercel/geist-font) und
[JetBrains Mono](https://github.com/JetBrains/JetBrainsMono), beide unter der SIL Open Font License
1.1 (`assets/fonts/OFL-*.txt`). Keine Verbindung zu Hantek; dieses Repository enthält keine Software
oder Firmware von Hantek.

## Lizenz

[MIT](LICENSE)
