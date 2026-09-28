# IR Mixer Pro — Audio Processing

This document is the internal contract for sample rates, convolution, mix
semantics, monitoring, and export. It explains which representation is
authoritative at each stage and prevents the real-time engine from accidentally
constraining export quality.

## Audio representation

Decoded audio uses planar `f32` channels with an explicit sample rate. Import is
limited to mono and stereo WAV. Mono sources are duplicated when a stereo path
is required; stereo sources retain their two channels.

Every loaded IR has two in-memory forms:

1. **Native source** — the immutable decoded WAV at its original sample rate.
2. **Prepared engine copy** — a direct resample of the native source to the
   current monitoring rate, partitioned for real-time convolution.

The native source supplies displayed file metadata, fingerprints, analysis and
export inputs, and every later engine rebuild. A prepared copy is never used as
the source for another resample.

## IR metadata and duration

The IR rack reports the WAV's native rate and frame count, not the engine rate.
Frame count alone does not describe time:

```text
duration_seconds = frames / sample_rate
```

At 48 kHz, 1,000 frames are about 20.8 ms and 24,000 frames are 500 ms. A long
IR may intentionally contain cabinet decay, early reflections, or a room/reverb
tail. Loading such an IR is valid, but it consumes more preparation time and
convolution work than a short close-mic cabinet IR.

## Standalone monitoring clock

The selected Windows output endpoint's shared-mode default format is the master
clock. Selecting a different output device causes the backend to:

1. Query its current default sample rate.
2. Stop the existing stream.
3. Invalidate the previous preparation generation.
4. Resample loaded IRs and the preview directly from native sources.
5. Build new partitions and start the stream after current-generation work is
   ready.

The app displays this rate but does not offer a competing monitoring-rate
selector. To change it, use Windows Sound settings or the interface control
panel, then restart/select the endpoint so the stream is rebuilt.

Preview mode opens only an output stream. Live mode opens the selected input
stream while monitoring is enabled. The current shared-mode implementation does
not resample live capture, so input and output default sample rates must match.
The backend reports a configuration error when they do not.

## Buffering and latency

Selectable engine blocks are 64, 128, and 256 frames. CPAL devices may return
callbacks of different sizes; the output adapter bridges them to fixed DSP
blocks with preallocated buffers.

Displayed latency estimates two blocks of buffering plus limiter lookahead:

```text
latency_ms ≈ block_frames × 2 / sample_rate × 1000 + limiter_lookahead_ms
```

Actual hardware and operating-system latency may be higher. Small buffers reduce
latency but increase deadline pressure. Release builds should be used for audio
evaluation because debug builds can miss deadlines and produce glitches.

## Real-time convolution and controls

The engine uses one uniform partitioned FFT convolver per channel of each active
IR. It reserves 16 real-time slots. Additional project slots may remain stored
but disabled.

The following updates do not rebuild convolvers:

- Gain or balance
- Mute, solo, and enable state
- Pan
- Polarity
- Positive delay
- Output gain and bypass
- Limiter enable
- Per-IR EQ parameters, topology, bypass, and EQ output gain

Gain, pan, polarity, audibility, output gain, and bypass transitions are smoothed
over 10 ms. Delay uses preallocated lines up to 4096 samples.

Each IR owns up to 16 dynamic `biquad` `DirectForm1<f32>` sections per channel.
The cascade runs after convolution and before delay, polarity, rack gain, and
pan. Replacement chains preserve matching stable-ID histories, crossfade for
10 ms, and retire completed state outside the callback.

If every IR is disabled, muted, or excluded by solo logic, monitoring passes the
source through. This prevents an empty rack from unexpectedly silencing input.

## Mix gain and Balance Mode

Ordinary mode stores gain in dB:

```text
linear_gain = 10^(gain_db / 20)
```

Balance Mode stores a percentage budget totaling 100% and synchronizes each
percentage to its linear amplitude gain. It is not equal-power crossfading and
does not attempt loudness compensation.

Pan uses a constant-power stereo law. Mono export is produced only after the
stereo mix:

```text
mono = 0.5 × (left + right)
```

## Normalization

Normalization is explicit at three different stages:

- **Preview normalization** scales the loaded preview peak to 0 dBFS before the
  input gain.
- **Per-IR normalization** scales that IR to 0 dBFS before its delay, polarity,
  gain, and pan are applied.
- **Final export normalization** scales the completed output to -1 dBFS.

None of these options is enabled implicitly by the processing layer. Relative
mix intent is preserved unless the corresponding control is enabled.

## Monitoring limiter and bypass

The optional monitoring limiter is stereo-linked and uses:

- -0.3 dBFS ceiling
- 1 ms lookahead
- 50 ms release

The lookahead delay is included in the latency estimate. Disabling the limiter
does not remove its fixed lookahead delay from the real-time path, which avoids
a timing jump when toggling it.

Bypass crossfades between the processed mix and the dry source. Neither bypass
nor limiter gain reduction is written into exported IRs.

## Mixed-rate IRs and export

IRs with different native rates may be mixed safely. Export has its own target
rate and does not reuse the monitoring engine's prepared buffers.

For every export, each active native source is resampled directly to the chosen
export rate with the offline 64-tap windowed-sinc resampler. Therefore:

- A 96 kHz source exported at 96 kHz is not first reduced to the monitoring rate.
- A 44.1 kHz source in a 96 kHz export is upsampled but cannot gain information
  above its original Nyquist limit.
- Choosing a lower export rate intentionally band-limits higher-rate sources.
- Repeated output-device changes do not accumulate resampling loss.

Export then applies per-IR normalization, sample-rate-specific EQ and EQ output
gain, delay, polarity, gain, pan, sum,
output gain, optional length trim/pad, channel conversion, optional final
normalization, and WAV encoding.

## Export length and encoding

When Trim to Length is enabled, export writes exactly 1024, 2048, or 4096 frames
at the chosen export rate. A shorter natural response is zero-padded; a longer
response, including a room tail, is truncated. When trimming is disabled, the
longest active resampled IR plus its delay determines the output length. Active
IIR tails render until both channels remain below −120 dBFS for 256 frames, with
a two-second cap.

Encoding choices are:

- 16-bit PCM with deterministic triangular dither
- 24-bit PCM with deterministic triangular dither
- 32-bit float

Integer PCM export fails rather than silently clipping when final normalization
is disabled and the peak exceeds full scale. Float32 may retain values above
0 dBFS for later processing.

## Analysis relationship

Frequency, phase, and combined impulse analysis use the active native sources
rendered at the current engine rate. They follow the same slot transforms and
output gain as export, but do not apply final normalization, monitoring bypass,
or limiter behavior.

The real-time spectrum and meters describe monitored output rather than the
stored IR alone.
