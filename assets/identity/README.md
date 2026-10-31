# Leitmotif identity

The wordmark sets "leitmotif" in Fraunces with two notes after it: diamond noteheads on
stems under a rising beam, a theme stated and then lifted. The diamonds are Recite's
colon squares, grown to read as noteheads, so the two wordmarks sit together. Letters
are converted to outlines, so the assets need no font installation. Fraunces is licensed
under the SIL Open Font License 1.1, which permits embedding its outlines; the assets
share the repository's MIT OR Apache-2.0 license.

- `leitmotif-wordmark.svg`: the quiet wordmark for the README and docs. Letters take the
  root `fill` (plum) and the notes use `currentColor` (teal), so a renderer can recolour
  each ink.
- `leitmotif-wordmark-reversed.svg`: the same on dark surfaces, with warm-paper letters
  and mint notes.
- `leitmotif-mark.svg`: the notes alone on a grained paper tile, for avatars and other
  square formats.
- `leitmotif-social.svg` and `leitmotif-social.png`: the 1280 by 640 social preview.
  GitHub only accepts the PNG, uploaded under Settings, General, Social preview.

The inks are Recite's: plum `#683e62`, riso teal `#00838a`, paper `#f3e9df` and mint
`#82d8d5`. Print assets set a hard teal copy of the ink 0.02 em right and down beneath
the plum, over paper grain from SVG `feTurbulence` (`fractalNoise`, base frequency 0.9,
two octaves, seed 7) at 7% alpha. The notes are printed once in the wordmark, without
the offset.

## Letterforms

Outlines come from the Fraunces variable font, `Fraunces[SOFT,WONK,opsz,wght].ttf` as
distributed by google/fonts. The instance is `wght` 700, `opsz` 144, `SOFT` 100 and
`WONK` 1, set with −0.04 em letter-spacing and no kerning. The social tagline uses
`wght` 420, `opsz` 36, `SOFT` 100 and `WONK` 0. Each notehead is a square 0.17 em on a
side, turned 45°, standing on the baseline. The second note is 0.40 em right and 0.08 em
higher. Stems are 0.05 em wide and run from the right-hand corner of each head to the
beam, which is 0.08 em thick and whose top edge meets the ascender line. The notes start
0.12 em after the f. To regenerate, instance the font with `fontTools.varLib.instancer`
and draw each glyph through `fontTools.pens.svgPathPen`.
