# Hantek DSO2D15 USB protocol notes (firmware 3.0.1, 2025-04-18)

Everything here was measured on a DSO2D15 (`*IDN?` -> `undefined, DSO2D15, ..., 3.0.1(250418.00)`)
and, where noted, read from the scope's own firmware (`phoenix`, the ARM Linux application in
the official `dso3kb_20250418.upk` update, the same build the scope runs). The analysis was static,
on the files only (with Claude Opus 5.5); no firmware was modified or uploaded to the scope.

## Transport

USBTMC, VID `0x049F` PID `0x505E`, through any VISA (tested: IVI `ausbtmc` driver with
R&S VISA 7.2 behind the IVI router `visa64.dll`). Inside the scope `phoenix` reads and writes
`/dev/usb_tmc` (`/dso/etc/usbtmc.ko`).

The SCPI parser is a table of `{name, handler, 0}` entries (`0xcda..`–`0xcdc..` in `phoenix`); a
query must match an entry including its `?`. Relevant entries:

| Command | Handler | Notes |
|---|---|---|
| `WAVeform:DATA:DISP?` | `0x848d0` | **use this**: every channel that is on, one acquisition |
| `WAVeform:DATA:ALL?` | `0x90738` | both channels on → one channel's record twice |
| `PRIVate:WAVeform:DATA:ALL?` | `0x93630` | what Hantek's PC software v1.0.3 sends; no answer on 3.0.1 |
| `WAVeform:DATA` | `0x8e8e8` | registered without `?`; `WAVeform:DATA?` times out |
| `WAVeform:SOURce`, `:FORMat`, `:POINts`, `:STARt`, `:STOP`, `:XINCrement?`, `:XORigin?`, … | | present |
| `WAVeform:PREamble?` | none | registered with a null handler: times out |
| `DISPlay:DATA?` | `0x843d8` | screen image |
| `SYSTem:LOCKed` | | accepted, but the panel stays locked while a session is open |

## `:WAVeform:DATA:DISP?`

A state machine: each query returns the next packet of the current stream.

1. **Header**, 128 bytes, format string at `0xce740`:
   `#9%09d%01d%01d%09d%09d%04d×4%.1e×4%01d×4%.3e%06d%+.2e%+.2e%03d%03d`

   | bytes | field |
   |---|---|
   | 0–1 | `#9` |
   | 2–10 | packet length (128) |
   | 11 | running |
   | 12 | triggered |
   | 13–21 | stream length: 99 + samples of all channels |
   | 22–30 | position (0) |
   | 31–46 | 4 × channel offset in screen counts (`%04d`, e.g. `0050`, `-050`) |
   | 47–74 | 4 × volts/div at the probe tip (`%.1e`, e.g. `5.0e-01`) |
   | 75–78 | 4 × channel on |
   | 79–87 | sample rate (`%.3e`) |

2. **One packet per channel that is on**, in channel order: `#9%09d%09d%09d` (length, stream
   length, position) followed by the samples, copied from that channel's own buffer
   (`snapshot + 0x24 + 4·ch`).

3. `#9000000000` (11 bytes) when there is no display snapshot to send; ask again later.

Samples are signed 8-bit screen positions: 0 is mid-screen, 25 counts per division, channel
offset included (`V = (s − offset) / 25 × V/div`). At 4K memory depth each channel has 3850 points
(the on-screen window); `:WAVeform:XINCrement?` and `:WAVeform:XORigin?` give the time axis
(`t = i·XINC − XORIGIN`, trigger at 0).

Checked against the scope's own `:MEASure` results on the 1 kHz probe-compensation output:
Vmax, Vpp and frequency agree. Rate on USB 2.0 HS: ~1.7 complete two-channel records per second
(about 190 ms per packet on the scope side), nearly every one a new acquisition.

## `:WAVeform:DATA:ALL?` and why it shows CH1 twice

Same idea, different header (format at `0xcee38`: length, total, position, running, triggered,
offsets, then volts/div printed from an *integer* microvolt value with `%.1e`, which comes out as
a denormal such as `9.9e-319` = 200 000 µV). The handler walks CH1…CH4 and asks the acquisition
layer (`0x5a95c`) for each enabled channel's data, but with both channels on the record that comes
back is the same channel twice: CH1 in steady state, CH2 for a while after CH2 had been on alone.
`:WAVeform:SOURce`, a `CHANnel2` argument, trigger or measure source, memory depth, acquisition
type, STOP and timebase changes make no difference. With one channel on it works, and the stream
also contains a bare `#9000000000` between header and data.

## Other observations

* The scope refreshes the record it hands out about once a second; a reader that joins mid-stream
  must resynchronise on a header, since the cursor is kept between queries (and between clients).
* `:SINGle` is accepted and ignored; `:TRIGger:SWEep SINGle` works.
* **Front panel lock.** Every USB command locks the keys: the USBTMC read loop calls
  `anolis_is_keypad_locked` / `anolis_lock_keypad` (around 0x8dcf8). A periodic check (0x9428c)
  compares the time of the last USB command with now and, after more than 4999 ms, jumps to
  0x80bec, which calls `anolis_unlock_keypad`. So the panel comes back about 5 s after the PC stops
  sending; `:SYSTem:LOCKed OFF` does not help while commands keep coming. HUSBB's live link switch
  relies on this.
* **Trace colours.** Each trace's colour is `table[index[ch]]` (0x71d78), with an ARGB table at
  0xcb1e8: red, orange, yellow `#FFFF00` (CH1), green `#00FF00` (CH2), cyan, blue, magenta, pink,
  grey, white. HUSBB uses the same yellow and green.
* Each SCPI query takes about 200 ms to answer, so settings are refreshed one per record.
* On a laptop whose ports are all USB 3.x (AMD xHCI), the scope dropped off the bus after 30 s to
  5 min — even with no traffic and on battery power (Windows: Kernel-PnP event 1010 "surprise
  removed … missing on the bus"); only a re-plug brought it back. A plain USB 2.0 hub in between
  fixed it (5 min of continuous reading, no drop). `tools/idle_watch.ps1` reproduces the idle test.
