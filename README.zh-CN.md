<p align="center">
  <img src="assets/icon.svg" width="96" alt="HUSBB 标志：深色示波器屏幕上的绿色正弦波">
</p>

<h1 align="center">HUSBB</h1>
<p align="center"><b>Hantek DSO2D15 USB Bridge</b> - 在电脑上实时显示 DSO2D15 的屏幕，双通道同时显示。</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.tr.md">Türkçe</a> |
  <b>简体中文</b> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.de.md">Deutsch</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.ru.md">Русский</a>
</p>

<p align="center"><a href="https://github.com/bugragungoz/HUSBB/releases/latest"><b>下载 HUSBB.exe</b></a></p>

![HUSBB 显示 Hantek DSO2D15 的 CH1 和 CH2](docs/screenshot-scope.png)

HUSBB 是一款面向 **Hantek（汉泰）DSO2D15** 示波器的原生 Windows 应用（Rust，基于 wgpu 的 egui）。它通过 USB 从同一次采集中读取两个通道，并按示波器自身的方式绘制波形，同时提供测量、读取真实采样点的光标、标记、画面冻结、一键导出以及示波器的各项控制 - 无需 Hantek 的 PC 软件。本软件专为 DSO2D15 编写，也只在 DSO2D15 上测试过。

## 为什么要做它

在 **3.0.1（2025-04-18）** 固件上，Hantek 为 DSO2D15 提供的 PC 软件什么也显示不出来：它发送 `PRIVate:WAVeform:DATA:ALL?`，而该固件已不再响应这条命令。文档中的 `:WAVeform:DATA:ALL?` 虽然有响应，但两个通道都打开时，它会把同一个通道的数据发送两次。

通过对 DSO2D15 固件进行逆向工程，找到了 `:WAVeform:DATA:DISP?`：它返回所有已打开的通道，每个通道都来自同一次采集中各自的缓冲区 - 正是示波器当前正在绘制的那条记录。HUSBB 使用的就是这条命令，因此无需来回切换通道、不丢失采样点（每个通道屏幕上的全部 3850 个点），CH1 与 CH2 之间的时间关系也是真实的。固件分析还揭示了电脑连接时前面板按键为何会被锁定，以及如何把控制权交还给前面板。详情见 [docs/PROTOCOL.md](docs/PROTOCOL.md)（英文）。

## 功能

- **双通道实时显示**，使用示波器自身的颜色（CH1 黄色，CH2 绿色），每秒约 1.5 条新记录 -这是 DSO2D15 在 USB 上的自身上限。
- **Live link 开关。** 关闭时 HUSBB 不发送任何命令，约 5 秒后 DSO2D15 会解锁前面板，可以直接使用示波器按键；打开后 HUSBB 重新接管。
- **控制：** 运行 / 停止、单次、自动设置、通道开关、V/div、垂直位置、时基（time/div）、触发源、触发沿、触发方式和触发电平。每次修改后都会从示波器回读确认。
- 每个通道的 **View**：仅在 HUSBB 中显示或隐藏某条波形。
- 每个通道的 **测量**：频率、周期、占空比、Vpp、Vmax、Vmin、Vavg、Vrms，也可以叠加显示在屏幕上。
- **跟踪光标（Track）** 吸附到真实采样点，在 A、B 两处同时读取两个通道（dt、1/dt、dV）；另有电压光标。
- **标记：** 双击波形可标记一个采样点（时间和电压）；右键删除。
- **Hold** 冻结画面，示波器本身继续运行。
- **Export** 一键导出到设置中选定的文件夹（屏幕 PNG 和/或数据 CSV），或用 **Save as...** 保存到任意位置。
- 鼠标悬停读取时间和电压，自动重连，记住你的设置。

![Measure 选项卡：测量表格与吸附到真实采样点的跟踪光标](docs/screenshot-measure.png)

## 下载

从 Releases 下载 [**HUSBB.exe**](https://github.com/bugragungoz/HUSBB/releases/latest)：单个文件，无需安装。除此之外只需要一个 VISA（见下文）。该 exe 没有代码签名，Windows SmartScreen 可能会提示一次：点击 *更多信息* -> *仍要运行*。

## 运行要求

- Windows 10 或 11（64 位），以及一台 Hantek DSO2D15（已在固件 3.0.1(250418.00) 上测试）。
- 支持 USBTMC 的 VISA，使示波器出现在设备管理器的 *USB Test and Measurement Devices* 下：Keysight IO Libraries 或 R&S VISA（已用 R&S VISA 7.2 和 IVI `ausbtmc` 驱动测试）。HUSBB 在运行时加载 `visa64.dll`。
- 使用示波器 **背面** 的 USB-B 接口。

### 如果 DSO2D15 频繁断开连接

在所有端口都是 USB 3.x 的笔记本电脑上，DSO2D15 会在 30 秒到 5 分钟之间从总线上掉线 - 即使完全没有数据传输、使用电池供电时也一样（Windows 会记录 Kernel-PnP 事件 1010：*"surprise removed as it is reported as missing on the bus"*）。**在笔记本和示波器之间加一个普通的 USB 2.0 集线器（Hub）即可解决。** `tools/idle_watch.ps1` 可以复现这个测试。

## 编译与运行

需要 [Rust](https://rustup.rs)（stable，MSVC 工具链）：

```bash
cargo build --release
```

```bash
target\release\HUSBB.exe
```

命令行参数，主要用于自动化：

```
--resource <VISA 名称>     使用指定的示波器，而不是找到的第一台
--screenshot <文件.png>    在 --after 秒后（默认 8 秒）保存窗口截图并退出
--cursors track|voltage    启动时显示光标
--tab measure              启动时打开 Measure 选项卡
```

硬件检查（连接 DSO2D15 时）：

```bash
cargo run --example probe
```

```bash
cargo run --example link
```

单元测试使用从真实 DSO2D15 录制的数据包（`tests/data`）：

```bash
cargo test
```

## 目录结构

| | |
|---|---|
| `src/visa.rs` | VISA 绑定，运行时加载 |
| `src/protocol.rs` | `:WAVeform:DATA:DISP?` 数据流解析 |
| `src/scope.rs` | DSO2D15 命令与设置 |
| `src/acquire.rs` | 采集线程、live link、自动重连 |
| `src/measure.rs` | 测量与 SI 单位格式化 |
| `src/ui/` | 窗口、示波器屏幕、光标、标记、控件（"Bugra" 设计系统） |
| `docs/PROTOCOL.md` | 固件分析的结果 |

## 如何做出来的

HUSBB 由 **Claude Opus 5.5** 构建。DSO2D15 的通信协议是通过对示波器的原始固件升级包和 Hantek 的 PC 软件进行逆向工程得到的 - 仅对文件做静态分析；没有向示波器写入或刷写任何内容。

## 致谢

此前的协议研究：[phmarek/hantek-dso2000](https://github.com/phmarek/hantek-dso2000) 以及 [EEVblog DSO2000 讨论帖](https://www.eevblog.com/forum/testgear/hantek-dso2x1x-models-master-thread/)。字体：[Geist](https://github.com/vercel/geist-font) 和 [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono)，均采用 SIL Open Font License 1.1（`assets/fonts/OFL-*.txt`）。本项目与 Hantek 无关；本仓库不包含任何 Hantek 的软件或固件。

## 许可证

[MIT](LICENSE)
