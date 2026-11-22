# Mockups

Source of the design canvas at https://claude.ai/artifact/DGM4unNGpV9nqAQaW4tfB5. Open the canvas to see them rendered and clickable.

| File | What it shows | Status |
|---|---|---|
| `Main.dc.html` | Home screen: FONY TV, FONY VCR in a black-ash unit, two poster areas, Dymo tabs, a shelf of handwritten tapes. Clicking a tape switches the TV to AV1 ▶ PLAY. | **Current look**: build to this |
| `Boot.dc.html` | Power-on screen (full-size CRT) | **Current look** |
| `Shelf.dc.html` | Shelf wall + pulled-out tape panel | **Layout only**: still the older, cartoonier styling; restyle it to match Main |

## Reading these files
They're "Design Component" pages for the canvas editor, not plain HTML:
- They need the editor's `support.js`, so they won't render if opened directly.
- Inline `style="…"` attributes hold the real CSS values: reuse them.
- `{{ … }}`, `<sc-if>` and `<sc-for>` are the editor's template syntax (if/loop). Port them to React (`{cond && …}`, `.map()`).
- The `<script data-dc-script>` block holds the sample data and click logic.
- In `Main.dc.html` the tape photos point at `vhs-stack.png` / `vhs-single.png` in this folder.
