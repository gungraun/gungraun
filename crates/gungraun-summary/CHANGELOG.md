# Changelog

This is the CHANGELOG of the `gungraun-summary`.

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to
[Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Support parsing version 7 summaries through the new `v7` module and the
  version-aware parsing helpers.
- Freeze the version 6 data model from the `gungraun-summary-v6.0.0` release, so
  future runner model changes cannot alter version 6 decoding.

### Changed

- Generate schemas for either supported summary version through the schemagen
  binary.

## [6.0.0] - 2026-06-27

The major version of `gungraun-summary` tracks the latest Gungraun summary
schema version supported by this crate which is currently `v6`.

### Added

- Initial release
