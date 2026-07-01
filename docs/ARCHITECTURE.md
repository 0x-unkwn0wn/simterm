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
  validate/    semantic campaign validation (--doctor)
  asset/       asset-source abstraction for campaign-adjacent files
```

Important model files:

- `campaign.rs` - campaign root: missions, theme, easter eggs, fortunes,
  signals, achievements, declarative commands.
- `command.rs` - declarative campaign commands: triggers, effects, conditions.
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
  cleanup, netmap, pivot, completion, and declarative/terminal command dispatch.
- `sysemu.rs` - emulated POSIX system commands (`uname`, `ps`, `netstat`, `env`,
  `grep`…) synthesized from host data, plus environment and `$VAR` expansion.
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
- optional audio (`audio.rs`): per-mission WAV playback via `rodio`, resolved
  from `<campaign>/music/`. Like `ratatui`/`crossterm`, audio is a frontend-only
  dependency; the engine has no audio code.

The engine owns game state transitions. Campaign-specific flavor that looks like
a command should be implemented as `easter_eggs` (flavor only) or `commands`
(declarative effects) in campaign data, not as Rust branches in the command
parser.

## Command Registry

Command metadata (canonical name, aliases, category, summary, usage, and kind:
engine-built-in, frontend-only, minigame, or flavor-reserved) lives in a single
frontend registry (`crates/simterm/src/registry.rs`). Autocomplete and help read
from it so there is one source of truth for the built-in command surface.

Because some commands are presentation-only, the registry lives in the frontend,
not the engine. To let the engine validate campaigns without depending on the
frontend, the frontend passes a neutral list of reserved verbs
(`registry::reserved_verbs()`) into `validate_campaign`. The engine never imports
the registry.

## Declarative Command Effects

Declarative campaign commands (`Campaign.commands`) are parsed and dispatched by
the frontend, but their **effects run in the engine runtime**
(`actions::campaign_command`), which mutates `GameState` (flags, trace,
achievements, mission completion). The frontend only routes the verb; it never
implements the effect. This keeps the engine/frontend boundary intact:
presentation and input in the frontend, state transitions in the engine.

## Terminal Emulation

The console emulates a realistic POSIX shell. Two neutral, content-free layers
live in the engine (`runtime/sysemu.rs`), alongside `ls`/`cat`:

- **Synthesized system commands** (`uname`, `id`, `ps`, `netstat`, `ifconfig`,
  `env`, `grep`, `head`/`tail`, `wc`, `file`) render from the existing host model
  (`TargetNode`) and VFS. Authors get a believable box from data they already
  wrote — no per-command authoring.
- **Environment model**: `Campaign.env` plus engine-derived variables
  (`USER`, `HOME`, `PWD`, `HOSTNAME`, `SHELL`), session `export` overrides, and
  `$VAR`/`$?` expansion.

Only the *authored* pieces are campaign data: the `env` map, `processes` (extra
`ps` rows), and `terminal` (authored realistic CLIs, presentational). The frontend
parses these verbs into `Command::Shell` / the unknown path and dispatches to the
engine; it never implements the output. Shell output is authentic POSIX (English)
regardless of the campaign's narrative `language`.

## Semantic Validation

`validate_campaign(&Campaign, &reserved_verbs) -> ValidationReport` lives in the
engine (`validate.rs`) and powers the frontend's `--doctor` mode. It is a pure
data analysis with no UI dependency, so any frontend or tool can reuse it. The
`--check` mode still performs only a basic load.

## Public Repository Rule

The public repository should contain only:

- generic engine and frontend code,
- public documentation,
- neutral examples,
- tests.

Do not add private experience text, private planning documents, unpublished
storylines, proprietary branding, or non-public campaign assets.
