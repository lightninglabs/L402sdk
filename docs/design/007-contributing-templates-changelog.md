# Design Doc 007: CONTRIBUTING.md, Issue Templates, and CHANGELOG.md

**Issue:** #16
**Author:** Dario Anongba Varela
**Date:** 2026-03-16

## Problem

L402sdk lacks standard open-source project scaffolding files. Without a CONTRIBUTING.md, new contributors don't know how to set up the project, run tests, or submit changes. Without issue templates, bug reports and feature requests arrive in inconsistent formats. Without a CHANGELOG, users can't track what changed between releases.

These are the last scaffolding items before the project can accept external contributors.

## Proposed Design

### CONTRIBUTING.md

A comprehensive guide covering:

1. **Prerequisites**: Rust (stable, MSRV 1.85), Node.js 22+ (for TypeScript packages), protobuf-compiler (for LND gRPC)
2. **Development setup**: Clone, build, test commands
3. **Project structure**: High-level overview with links to AGENTS.md for architecture
4. **Coding standards**: Formatting, linting, doc comments, error handling — references CLAUDE.md
5. **PR workflow**: Branch naming (`type/description`), conventional commits, squash merge, CI requirements
6. **Testing requirements**: Rust (cargo test), TypeScript (vitest), integration tests with l402-mock
7. **Issue workflow**: File an issue first, get confirmation, then implement

### Issue Templates

Two GitHub issue templates using YAML forms (`.github/ISSUE_TEMPLATE/`):

1. **Bug report** (`bug_report.yml`): Description, steps to reproduce, expected vs actual behavior, environment (Rust version, OS, crate versions)
2. **Feature request** (`feature_request.yml`): Problem description, proposed solution, alternatives considered, additional context

Plus a `config.yml` to add a link to discussions for general questions.

### CHANGELOG.md

Following [Keep a Changelog](https://keepachangelog.com/) format:

- Header with format description and link conventions
- `[Unreleased]` section for tracking current changes
- Retroactive entries for work done so far (initial release content)
- Categories: Added, Changed, Deprecated, Removed, Fixed, Security

## Key Decisions

1. **YAML issue templates** over markdown templates: YAML forms provide structured fields, dropdowns, and validation. Better UX than freeform markdown.
2. **Keep a Changelog format**: Industry standard, human-readable, and compatible with future automation from conventional commits.
3. **Single CONTRIBUTING.md** for both Rust and TypeScript: The project is a unified workspace, so one file covers both.
4. **No automated changelog generation yet**: Manual for now; will automate with `git-cliff` or similar when we have releases.

## Alternatives Considered

- **Separate contributing guides per language**: Rejected — too much duplication, project is a single workspace.
- **GitHub Discussions instead of issue templates**: We want structured issue tracking, not forum-style discussion. Can add discussions later.
- **Automated changelog from day one**: Premature — we don't have releases yet. Manual is fine.

## Testing Plan

- Verify issue templates render correctly on GitHub (create and cancel a test issue)
- Verify CONTRIBUTING.md links all work
- Verify CHANGELOG.md follows the specification
- CI remains green (no code changes, docs only)

## Files Created/Modified

- `CONTRIBUTING.md` (new)
- `.github/ISSUE_TEMPLATE/bug_report.yml` (new)
- `.github/ISSUE_TEMPLATE/feature_request.yml` (new)
- `.github/ISSUE_TEMPLATE/config.yml` (new)
- `CHANGELOG.md` (new)
- `README.md` (update Contributing section to link to CONTRIBUTING.md)
- `PROJECT.md` (update status)
