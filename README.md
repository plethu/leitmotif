# Leitmotif

Leitmotif is a Rust library for checking how a game's stories fit together.

I'm building it alongside [Recite](https://github.com/plethu/recite) for a game about a
town and the people who live there. Keeping track of who knows what, which events can
happen, and how one person's story changes another's is getting a little beyond what I
can hold in my head.

Models are written in Rust. You can check whether an ending is reachable or a rule can
be broken, and get a sequence of events showing how. I'd like this to make complicated
stories easier to write, including ones where people lie or nobody has the whole
picture.

It's still an early prototype. A compiled Recite guard adapter is available;
engine integrations are planned. For now, you can
[try the town and mystery examples](https://github.com/plethu/leitmotif/blob/main/docs/guide.md)
from a checkout.

[Contributing](https://github.com/plethu/leitmotif/blob/main/CONTRIBUTING.md) ·
[Milestones](https://github.com/plethu/leitmotif/milestones) ·
[Research and prior art](https://github.com/plethu/leitmotif/blob/main/ATTRIBUTIONS.md)

Licensed under
[MIT OR Apache-2.0](https://github.com/plethu/leitmotif/blob/main/LICENSE), at your
option.
