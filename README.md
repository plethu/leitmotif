<picture>
  <source
    media="(prefers-color-scheme: dark)"
    srcset="assets/identity/leitmotif-wordmark-reversed.svg"
  >
  <img src="assets/identity/leitmotif-wordmark.svg" alt="Leitmotif" width="260">
</picture>

[![CI](https://github.com/plethu/leitmotif/actions/workflows/check.yml/badge.svg?branch=main)](https://github.com/plethu/leitmotif/actions/workflows/check.yml)
[![MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-683e62?style=flat-square&labelColor=241f27)](https://github.com/plethu/leitmotif/blob/main/LICENSE)

Leitmotif is a Rust library for checking how a game's stories fit together.

I'm building it alongside [Recite](https://github.com/plethu/recite) for a game about a
town and the people who live there. Keeping track of who knows what, which events can
happen, and how one person's story changes another's is getting a little beyond what I
can hold in my head.

Models are written in Rust. You can check whether an ending is reachable or a rule can
be broken, and get a sequence of events showing how. I'd like this to make complicated
stories easier to write, including ones where people lie or nobody has the whole
picture.

It's still an early prototype. A compiled Recite guard adapter is available, and a
small harbour example replays checked evidence in headless Bevy, Godot and Unity. Those
are worked examples, not engine integrations you can drop into a game yet. You can
[try the town and mystery examples](https://github.com/plethu/leitmotif/blob/main/docs/guide.md)
from a checkout.

[Contributing](https://github.com/plethu/leitmotif/blob/main/CONTRIBUTING.md) ·
[Milestones](https://github.com/plethu/leitmotif/milestones) ·
[Research and prior art](https://github.com/plethu/leitmotif/blob/main/ATTRIBUTIONS.md)

Licensed under
[MIT OR Apache-2.0](https://github.com/plethu/leitmotif/blob/main/LICENSE), at your
option.
