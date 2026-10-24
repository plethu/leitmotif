# Attributions and prior art

These projects and papers informed Leitmotif. They are research sources and
inspirations, not code incorporated into the library.

## Stories, knowledge, and ambiguity

Isaac Karth, Nic Junius, and Max Kreminski, *Constructing a Catbox: Story Volume
Poetics in Umineko no Naku Koro ni*, ICIDS 2022, LNCS 13762, pp. 455–470.
[Author-hosted paper](https://mkremins.github.io/publications/Umineko_ICIDS2022.pdf) ·
[Publisher](https://doi.org/10.1007/978-3-031-22298-6_29).

The paper considers meaning across alternative storylines and constraints that
preserve or narrow ambiguity. It is literary/design research, not an implemented
solver. Leitmotif's proposed scoped premises and observer-specific queries are
our interpretation of useful ideas from it.

Ryukishi07 / 07th Expansion's *Umineko no Naku Koro ni* is a particularly useful
stress test for the distinction between world truth, testimony, and what an
observer can establish. The example mystery is original; no script, assets, or
solution are copied. Leitmotif does not claim to solve the novel.

## Narrative systems

| Project | What informed this work |
| --- | --- |
| [Prax](https://github.com/AHartNtkn/Prax) | World/belief distinctions, persistent state, social simulation, and explicit actions. Inspected at `5e64873b1d5894a464bbd4da8430a8f1bcc3f78f`. |
| [Sabre](https://github.com/sgware/sabre) | Narrative planning, typed fluents, belief state, and transition graphs. Inspected at `73a128123b10a57843068b69d0b0e0601f37488a` (v0.8.0). |
| [Recite](https://github.com/plethu/recite) | Typed dialogue queries and host-owned effect requests; the sibling integration we want to support first. |
| [Gatekeep](https://github.com/plethu/gatekeep), [Keepsake](https://github.com/plethu/keepsake), and Kumite | Typed Rust authoring, explicit ownership, and small libraries that compose. |

Reusing or extending Prax and Sabre was technically possible. We chose an
independent Rust implementation with permissive licensing and explicit control
over analysis semantics. On 24 October 2026, Prax had no published releases;
its manifests named BSD-3-Clause but the inspected tree lacked a license text.
Sabre's Java/GPL-3.0 distribution did not fit this project's chosen constraints.
Those are adoption decisions, not claims that the projects are incapable or
that their release status cannot change.

## Verification and simulation research

- Christopher J. F. Pickett, Clark Verbrugge, and Félix Martineau (2005),
  *(P)NFG: A Language and Runtime System for Structured Computer Narratives*,
  GAME-ON-NA 2005. [Paper](https://www.sable.mcgill.ca/publications/papers/2005-6/pickett-05-pnfg.pdf).
  Narrative Flow Graphs and NuSMV analysis provide an implemented precedent,
  including reported scaling limits.
- Isamu Hasegawa and Tomoyuki Yokogawa, *Formal Verification for Node-Based Visual
  Scripts Using Symbolic Model Checking*, IEICE E105-D(1), 78–91 (2022).
  [2021 preprint](https://arxiv.org/abs/2103.11618) ·
  [Journal article](https://www.jstage.jst.go.jp/article/transinf/E105.D/1/E105.D_2021EDP7063/_article).
  Their translation of visual scripts to NuSMV includes evaluation on FFXV
  production scripts; it does not establish automatic verification of arbitrary
  game code.
- Richard Evans and Emily Short, *Versu—A Simulationist Storytelling System*,
  IEEE TCIAIG 6(2), 113–130 (2014), DOI 10.1109/TCIAIG.2013.2287297.
  [Paper](https://cs.uky.edu/~sgware/reading/papers/evans2014versu.pdf) ·
  [Short's account of content structure](https://emshort.blog/2013/02/22/versu-content-structure/).
  Composable social practices and agent-selected affordances are useful
  simulation precedents; they are not evidence of exhaustive verification.

## Techniques and libraries to investigate

[petgraph](https://github.com/petgraph/petgraph) provides stored-graph algorithms;
[rsmt2](https://github.com/kino-mc/rsmt2) and [Z3](https://github.com/Z3Prover/z3)
are candidates for constraint solving;
[biodivine-lib-bdd](https://github.com/sybila/biodivine-lib-bdd) for symbolic sets;
[Ascent](https://docs.rs/ascent/latest/ascent/) and
[Soufflé](https://github.com/souffle-lang/souffle) for derived relations.
[SPIN's analysis reference](https://spinroot.com/spin/Man/Quick.html#Analysis)
is useful background for partial-order reduction.

These are candidates, not selected dependencies. The corresponding experiments
live in [issues](https://github.com/plethu/leitmotif/issues); a solver or graph
library does not supply narrative semantics or establish that an abstraction is
sound. Dependency licensing is checked separately by `cargo deny`.
