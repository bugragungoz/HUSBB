<p align="center">
  <img src="assets/icon.svg" width="96" alt="Logo de HUSBB: una onda senoidal verde en una pantalla de osciloscopio oscura">
</p>

<h1 align="center">HUSBB</h1>
<p align="center"><b>Hantek DSO2D15 USB Bridge</b> - la pantalla del DSO2D15 en tu PC, los dos canales, en vivo.</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.tr.md">Türkçe</a> |
  <a href="README.zh-CN.md">简体中文</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.de.md">Deutsch</a> |
  <b>Español</b> |
  <a href="README.ru.md">Русский</a>
</p>

<p align="center"><a href="https://github.com/bugragungoz/HUSBB/releases/latest"><b>Descargar HUSBB.exe</b></a></p>

![HUSBB mostrando CH1 y CH2 de un Hantek DSO2D15](docs/screenshot-scope.png)

HUSBB es una aplicación nativa de Windows (Rust, egui sobre wgpu) para el osciloscopio **Hantek
DSO2D15**. Lee los dos canales de la misma adquisición por USB y los dibuja como lo hace el
osciloscopio, con mediciones, cursores que leen muestras reales, marcadores, congelado de pantalla,
exportación con un clic y los controles del osciloscopio, sin el software de PC de Hantek. Está
escrita para el DSO2D15 y solo se ha probado con él.

## Por qué existe

Con el firmware **3.0.1 (2025-04-18)**, el propio software de PC de Hantek para el DSO2D15 no
muestra nada: pide `PRIVate:WAVeform:DATA:ALL?`, a lo que este firmware ya no responde. El comando
documentado `:WAVeform:DATA:ALL?` sí responde, pero con los dos canales encendidos envía un mismo
canal dos veces.

La ingeniería inversa del firmware del DSO2D15 sacó a la luz `:WAVeform:DATA:DISP?`: todos los
canales encendidos, cada uno desde su propio búfer de la misma adquisición, es decir, exactamente el
registro que el osciloscopio está dibujando. HUSBB usa este comando, así que no hay conmutación de
canales, no se pierden muestras (los 3850 puntos en pantalla de cada canal) y la relación temporal
entre CH1 y CH2 es real. El firmware también mostró por qué el panel frontal se bloquea mientras hay
un PC conectado y cómo devolverle el control. Detalles: [docs/PROTOCOL.md](docs/PROTOCOL.md) (en
inglés).

## Funciones

- **Los dos canales, en vivo**, con los colores del osciloscopio (CH1 amarillo, CH2 verde), unos
  1,5 registros nuevos por segundo: el límite propio del DSO2D15 por USB.
- **Interruptor Live link.** Apagado: HUSBB no envía nada y el DSO2D15 desbloquea su panel frontal
  unos 5 s después, para que puedas usar sus teclas. Encendido: HUSBB vuelve a tomar el control.
- **Controles:** run / stop, single, auto setup, canal encendido/apagado, V/div, posición,
  time/div, fuente, flanco, modo y nivel de disparo. Cada cambio se vuelve a leer del osciloscopio.
- **View** por canal: muestra u oculta una traza solo en HUSBB.
- **Mediciones** por canal: frecuencia, periodo, ciclo de trabajo, Vpp, Vmax, Vmin, Vavg, Vrms;
  también se pueden mostrar sobre la pantalla.
- **Cursores Track** que se ajustan a muestras reales y leen los dos canales en A y B (dt, 1/dt,
  dV), y cursores de tensión.
- **Marcadores:** doble clic en una traza para marcar una muestra (tiempo y tensión); clic derecho
  para quitarla.
- **Hold** congela la pantalla mientras el osciloscopio sigue funcionando.
- **Export** con un clic a una carpeta elegida en los ajustes (PNG de la pantalla y/o CSV de los
  datos), o **Save as...** a cualquier lugar.
- Lectura de tiempo y tensión bajo el ratón, reconexión automática y recuerda tus ajustes.

![Pestaña Measure: tabla de mediciones y cursores Track sobre muestras reales](docs/screenshot-measure.png)

## Descarga

[**HUSBB.exe**](https://github.com/bugragungoz/HUSBB/releases/latest) en Releases: un solo archivo,
sin instalador. Aparte solo necesitas un VISA (ver abajo). El exe no está firmado, así que Windows
SmartScreen puede preguntar una vez: *Más información* -> *Ejecutar de todas formas*.

## Requisitos

- Windows 10 u 11 de 64 bits y un Hantek DSO2D15 (probado con el firmware 3.0.1(250418.00)).
- Un VISA compatible con USBTMC, para que el osciloscopio aparezca en el Administrador de
  dispositivos en *USB Test and Measurement Devices*: Keysight IO Libraries o R&S VISA (probado con
  R&S VISA 7.2 y el controlador IVI `ausbtmc`). HUSBB carga `visa64.dll` en tiempo de ejecución.
- El puerto USB-B **trasero** del osciloscopio.

### Si el DSO2D15 se desconecta una y otra vez

En un portátil con todos los puertos USB 3.x, el DSO2D15 desaparecía del bus al cabo de entre 30 s y
5 min, sin ningún tráfico e incluso funcionando con batería (Windows registra el evento Kernel-PnP
1010, *"surprise removed as it is reported as missing on the bus"*). **Un simple hub USB 2.0 entre
el portátil y el osciloscopio lo solucionó.** `tools/idle_watch.ps1` reproduce la prueba.

## Compilar y ejecutar

Con [Rust](https://rustup.rs) (stable, toolchain MSVC):

```bash
cargo build --release
```

```bash
target\release\HUSBB.exe
```

Opciones de línea de comandos, sobre todo para automatización:

```
--resource <nombre VISA>   usar este osciloscopio en lugar del primero encontrado
--screenshot <archivo.png> guardar la ventana tras --after segundos (8 por defecto) y salir
--cursors track|voltage    iniciar con los cursores visibles
--tab measure              iniciar en la pestaña Measure
```

Pruebas de hardware (con el DSO2D15 conectado):

```bash
cargo run --example probe
```

```bash
cargo run --example link
```

Las pruebas unitarias usan paquetes grabados de un DSO2D15 real (`tests/data`):

```bash
cargo test
```

## Estructura

| | |
|---|---|
| `src/visa.rs` | enlace con VISA, cargado en tiempo de ejecución |
| `src/protocol.rs` | analizador del flujo `:WAVeform:DATA:DISP?` |
| `src/scope.rs` | comandos y ajustes del DSO2D15 |
| `src/acquire.rs` | hilo de adquisición, live link, reconexión |
| `src/measure.rs` | mediciones, formato SI |
| `src/ui/` | ventana, pantalla del osciloscopio, cursores, marcadores, controles (sistema de diseño "Bugra") |
| `docs/PROTOCOL.md` | lo que encontró el análisis del firmware |

## Cómo se hizo

HUSBB se construyó con **Claude Opus 5.5**. El protocolo del DSO2D15 se obtuvo mediante ingeniería
inversa de la actualización de firmware original del osciloscopio y del software de PC de Hantek,
solo con análisis estático de los archivos; no se escribió ni se flasheó nada en el osciloscopio.

## Créditos

Trabajo previo sobre el protocolo: [phmarek/hantek-dso2000](https://github.com/phmarek/hantek-dso2000)
y el [hilo de EEVblog sobre la serie DSO2000](https://www.eevblog.com/forum/testgear/hantek-dso2x1x-models-master-thread/).
Fuentes: [Geist](https://github.com/vercel/geist-font) y
[JetBrains Mono](https://github.com/JetBrains/JetBrainsMono), ambas bajo la SIL Open Font License 1.1
(`assets/fonts/OFL-*.txt`). Sin relación con Hantek; este repositorio no incluye software ni firmware
de Hantek.

## Licencia

[MIT](LICENSE)
