# Fony Media Centre — Design Direction (v3)

_Updated 13 Nov 2026. Read alongside spec.md. Working name: **Fony Media Centre**._

## Brief in one line
A realistic UK bedroom from 1985–1993: a black Trinitron-style set on a black-ash TV unit, the VCR in the cubby below, home-taped cartoons on the shelf, and posters on the wall. Realism over cartoon: real materials, textures, gloss and shadows. **No vaporwave or neon.**

## Hardware look (references: Sony KV-20S90 TV, Sony SLV-series VCR)
- **TV: "FONY Trinitone".**
  - Matte black boxy cabinet with chamfered lower corners and perforated speaker grilles down both sides.
  - A small "Trinitone" badge top-left.
  - A chrome serif FONY wordmark centred under the screen.
  - A control strip with tiny labels (SET UP · TV/VIDEO · VOLUME · CHANNEL · POWER in green) and a flap.
  - **CRT effect:** aperture grille, scanlines, the two faint damper-wire lines, vignette and glass glare.
  - Power-on screen: dark blue, white FONY / MEDIA CENTRE, green "AV1" and a blinking "INSERT TAPE".
- **VCR: FONY front loader.**
  - On the **left**: chrome FONY badge, power button, display window (smoked glass with cyan digits, 12:00 flashing), eject.
  - On the **right**: the larger **cassette slot flap** with gold italic "Hi-Fi Stereo / DIGITAL AUTO TRACKING / AUTO HEAD CLEANER", and a smoked transport panel below it.
- **Furniture:** a black-ash TV unit with a VCR cubby and a tape cubby below; tapes stand spine-out.
- **Wall:** a warm painted woodchip wall with grain and a lamp-glow vignette.
  - **Two large 2:3 poster areas** either side of the TV, blu-tacked, for Glitchy's own artwork.

## Tapes
- **Realistic VHS spines:** black plastic with fine ribbing and a cream paper label inset.
- **For now:** handwritten labels in biro/felt-tip in mixed blue, red, black and green ink (Kalam).
- **Later:** real spine and cover artwork, made from TMDB posters or downloaded/scanned spines, dropped in through the editor.
- Glitchy's transparent blank-tape PNGs (`mockups/vhs-*.png`) are used as props on the unit top.

## UI furniture
- Shelf tabs and buttons are **Dymo embossed label tape** (black, with red for the selected tab).
- On-screen text uses the TV's OSD font (VT323).

## Tokens
- **Fonts:**
  - VT323 (OSD/VFD)
  - Alfa Slab One with `scaleX(1.18)` (FONY wordmark, chrome gradient fill)
  - Inter (small labels, gold italic VCR lettering)
  - Kalam 700 (handwritten labels)
  - Roboto Mono 700 (Dymo)
- **Colours:**
  - wall `#4a4740`→`#2c2a26`
  - black ash `#2a2724`→`#1d1b19`
  - TV plastic `#2b2b2d`→`#1b1b1d`
  - VFD cyan `#5cf2d6`
  - OSD green `#5dff6a`
  - AV blue `#1630c4`
  - VCR gold `#d8c48f`
  - tape label cream `#efe9d8`
  - Dymo `#141414` / `#b3201f` / `#1c3f8f`

## Image use
Personal use only, never distributed.
- Glitchy can drop in real product photos, transparent PNGs, posters and spine scans through the editor.
- Built-in hardware stays FONY-branded, with no real Sony marks recreated.

## Parked for later
Toys and stickers (robot, puzzle cube, gross-out stickers, joystick, handheld) are removed for now, to revisit once the realistic base is right.

## Sound & motion
- VCR clunks, CRT degauss, rewind whine.
- Tracking noise when seeking.
- An idle "INSERT TAPE" screen.
- Everything must be keyboard/remote navigable (P3).
