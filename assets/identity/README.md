# Leitmotif identity

The wordmark sets "leitmotif" in Fraunces with two linked diamonds after it: one shape
stated and restated, each loop crossing the other twice. The diamonds are Recite's colon
squares drawn as outlines, so the two wordmarks sit together. Where the strokes cross
they overprint, as two inks would on a riso plate. Letters are converted to outlines, so
the assets need no font installation. Fraunces is licensed under the SIL Open Font
License 1.1, which permits embedding its outlines; the assets share the repository's
MIT OR Apache-2.0 license.

- `leitmotif-wordmark.svg`: the quiet wordmark for the README and docs. Letters are
  plum; one link is plum and the other teal.
- `leitmotif-wordmark-reversed.svg`: the same on dark surfaces. Letters and one link are
  warm paper, the other link mint, and the crossings are a bright screened tone, since
  light inks add where dark inks multiply.
- `leitmotif-mark.svg`: the two links alone on a grained paper tile, for avatars and
  other square formats.
- `leitmotif-social.svg` and `leitmotif-social.png`: the 1280 by 640 social preview.
  GitHub only accepts the PNG, uploaded under Settings, General, Social preview.

The inks are Recite's: plum `#683e62`, riso teal `#00838a`, paper `#f3e9df`, mint
`#82d8d5` and overprint `#002035`, plum multiplied by teal, which fills each crossing.
The social preview sets a hard teal copy of the letters 0.02 em right and down beneath
the plum, over paper grain from SVG `feTurbulence` (`fractalNoise`, base frequency 0.9,
two octaves, seed 7) at 7% alpha. The links are printed once.

## Letterforms

Outlines come from the Fraunces variable font, `Fraunces[SOFT,WONK,opsz,wght].ttf` as
distributed by google/fonts. The instance is `wght` 700, `opsz` 144, `SOFT` 100 and
`WONK` 1, set with −0.04 em letter-spacing and no kerning. The social tagline uses
`wght` 420, `opsz` 36, `SOFT` 100 and `WONK` 0. Each link is a square turned 45°, as
tall as the x-height, stroked 0.062 em with mitred corners (0.09 em in the mark, so it
holds at avatar sizes). Their centres are 1.2 half-diagonals apart, so the strokes cross
at right angles, and the pair starts 0.10 em after the f and stands on the baseline. To
regenerate, instance the font with `fontTools.varLib.instancer` and draw each glyph
through `fontTools.pens.svgPathPen`.
