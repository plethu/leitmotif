# Changelog

Leitmotif is pre-release and unpublished; these notes cover changes that affect
hosts building against the source.

## Unreleased

### Added

- `Model::restore` rebuilds a `State` from a `Snapshot`. `Snapshot` and `Value`
  now implement `Deserialize`, and a snapshot's JSON form is a persisted save
  format.
- `Model::get` reads a fact's typed value from a `State`. A read allocates nothing
  beyond cloning the returned value.

### Breaking

- `ErrorKind` gains `SnapshotMismatch`. `ErrorKind` is not `#[non_exhaustive]`, so
  exhaustive matches on it need a new arm.
- `FactHandle<F>` keeps its fact's registered domain, so it is `Send`, `Sync`,
  `UnwindSafe` and `RefUnwindSafe` only when `F::Value` is.
