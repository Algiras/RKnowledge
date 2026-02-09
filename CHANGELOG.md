# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-02-09

### Added
- **Tenant Isolation**: Support for property-based knowledge graph isolation within Neo4j. Added `--tenant` global flag.
- **Manual Relation Insertion**: New `rknowledge add` command for manual concept and relationship entry.
    - Supports interactive mode (`--interactive`).
    - Supports batch import from JSON (`--file`).
- **Domain Context Customization**: Enhanced LLM prompt customization for specialized domains.
    - Added `--domain` and `--context` flags to the `build` command.
    - Added `--context-file` flag to load prompt context from a file.
    - Support for `[domain]` and `[tenant]` sections in `config.toml`.
- **Knowledge Graph Statistics**: Added detailed reporting of node and edge counts in `stats` command.
- **Redesigned Visualization Dashboard**: Complete UI overhaul for `rknowledge viz`.
    - Interactive sidebar for concept filtering by entity type.
    - Proximity-aware search with neighborhood highlighting.
    - Detailed concept property cards with relationship exploration.
    - Dark-mode glassmorphism design.

### Fixed
- Fixed regression where `build` command was not respecting the tenant scoping.
- Fixed visualization rendering issues and unhandled empty states.
- Improved error handling for Neo4j connectivity and missing configuration files.
- Optimized parallel processing in `BatchProcessor` to handle large codebases more reliably.

### Usage Guide for v0.2.0
- **Manual Add**: `rknowledge add "Node A" "Node B" --relation "connected" --tenant myproject`
- **Domain Focus**: `rknowledge build ./docs --domain software --context "extract API calls"`
- **Filtered View**: Open `rknowledge viz`, use the sidebar to filter out clutter (like "concept" or "document" types).

## [0.1.0] - 2026-01-20

### Added
- Initial release with document processing and Neo4j integration.
- Support for PDF, Markdown, HTML, and Text files.
- Visualization functionality via `rknowledge viz`.
- Community detection and path finding.
