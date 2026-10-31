# Changelog

Leitmotif is pre-release and unpublished; these notes cover changes that affect
hosts building against the source.

## Unreleased

### Added

- `Model::restore` rebuilds a `State` from a `Snapshot`. `Snapshot` and `Value`
  now implement `Deserialize`, and a snapshot's JSON form is a persisted save
  format.

### Breaking

- `ErrorKind` gains `SnapshotMismatch`. `ErrorKind` is not `#[non_exhaustive]`, so
  exhaustive matches on it need a new arm.
