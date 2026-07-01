# Architecture

SimTerm is a framework for building immersive terminal-based games and
experiences. It is split into engine code and campaign content. The engine
interprets a `Campaign` value loaded from disk; it does not know about any
specific story, mission, host, IP address, organization, ending, or brand.

## Framework vs. Content

| Area | Framework and frontend | Campaign or experience |
|---|---|---|
| Location | `crates/` | external `.ron` files |
| Contains | rules, loader, runtime state, TUI, CLI | missions, hosts, text, endings, theme |
| License | MIT | chosen by the campaign author |
| Coupling | generic data model only | no Rust code required |

This boundary allows the public repository to remain open source while private
or commercial experiences are shipped separately.

## Workspace

```text
crates/
  engine/     simterm-engine library
  simterm/   terminal frontend binary
```

`simterm-engine` contains the reusable model, campaign loader, and runtime
rules. It has no `ratatui` or `crossterm` dependency, so another frontend could
reuse it.

`simterm` is the playable terminal application. It parses CLI arguments, loads
a campaign with `--campaign`, owns terminal setup/teardown, and dispatches
player input to the engine.

## Engine Modules

```text
crates/engine/src/
  model/       immutable campaign data structures
  runtime/     mutable game state and player actions
  loader/      campaign loading from directories or .ron files
  asset/       asset-source abstraction for campaign-adjacent files
```

Important model files:

- `campaign.rs` - campaign root: missions, theme, easter eggs, fortunes,
  signals.
- `mission.rs` - mission settings, entry vectors, endings, multi-host networks.
- `target.rs` - target hosts, services, vulnerabilities, local privilege
  escalation metadata.
- `filesystem.rs` - virtual filesystem, loot, hashes, reversible binaries, and
  encoded files.
- `toolbox.rs` - generic enumeration tools and service categories.
- `theme.rs` - neutral UI text defaults and campaign branding fields.

Important runtime files:

- `state.rs` - all mutable campaign and mission state.
- `actions.rs` - recon, enumeration, research, exploit, login, privesc, VFS,
  cleanup, netmap, pivot, and completion logic.
- `detection.rs` - trace accumulation.
- `balance.rs` - engine-level tuning constants.
- `probability.rs` - randomness helpers for imperfect information.

## Campaign Loading

`load_campaign(path)` accepts either:

- a directory containing `campaign.ron`, or
- a direct `.ron` file path.

The loader deserializes RON into `Campaign` and rejects empty campaigns. The
frontend's `--check` mode uses this path to validate that a campaign can be
loaded without opening the TUI.

## Runtime Flow

The current sample campaign follows a terminal hacking loop:

```text
RECON -> ENUM -> EXPLOIT -> POST -> COMPLETE
```

The exact opening step depends on `EntryVector`:

- `Active` starts before active scanning.
- `Cold` starts in enumeration with selected ports already known.
- `Passive` encourages `sniff` instead of noisy active scanning.
- `Pivot` requires `connect` before the target can be scanned.

That loop is one concrete experience built on top of the framework. Future
campaigns can model different terminal-native interactions while reusing the
same loader, runtime state, frontend, logs, campaign data, and presentation
boundaries.

## Frontend Boundary

The terminal app owns:

- CLI parsing.
- loading the selected campaign path.
- terminal rendering and input handling.
- command parsing and dispatch.
- presentation-only commands such as help, logs, status, fortunes, and
  minigames.

The engine owns game state transitions. Campaign-specific flavor that looks like
a command should be implemented as `easter_eggs` in campaign data, not as Rust
branches in the command parser.

## Public Repository Rule

The public repository should contain only:

- generic engine and frontend code,
- public documentation,
- neutral examples,
- tests.

Do not add private experience text, private planning documents, unpublished
storylines, proprietary branding, or non-public campaign assets.
