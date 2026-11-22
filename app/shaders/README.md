# CRT shaders

mpv loads these during playback (`glsl-shaders`). The app ships this folder as a resource, so the
Rust side finds it with Tauri's resource path.

## crt-lottes.glsl

- **Original:** Timothy Lottes' CRT shader, released into the public domain.
- **mpv port:** [mpv-retro-shaders](https://github.com/hhirtz/mpv-retro-shaders) by the
  mpv-retro-shaders contributors, under an ISC-style licence (the notice is kept at the top of
  the file).
- **Our change:** six settings that were `#define`s are now mpv `PARAM` blocks. That lets the app
  switch presets live with `glsl-shader-opts`, without reloading the file.

## Presets

Set in `crt_preset_options` in `src-tauri/src/lib.rs`.

| Preset | Mask | Look |
| --- | --- | --- |
| Trinitron | 2, aperture grille | The shader's defaults: crisp scanlines, gentle curve |
| Portable | 1, TV shadow mask | Softer scanlines, more curve and glow, like a small portable set |
| Arcade | 3, stretched VGA | Hard scanlines, flatter screen |
| Clean | none | No shader |

## Settings

| Setting | What it does |
| --- | --- |
| `HARD_SCAN` | Scanline hardness. More negative means sharper, darker gaps |
| `CURVATURE_X`, `CURVATURE_Y` | How much the picture bends at the edges |
| `MASK_DARK`, `MASK_LIGHT` | How strongly the phosphor mask darkens and brightens |
| `BLOOM_AMOUNT` | Glow around bright areas |
| `SHADOW_MASK` | Mask type: 0 none, 1 TV shadow mask, 2 aperture grille, 3 stretched VGA, 4 VGA |
