# Campaign Format

SimTerm is a framework for building immersive terminal-based games and
experiences. Campaigns are written in [RON](https://github.com/ron-rs/ron). The
fastest way to learn the format is to read this reference alongside
[`examples/sample_campaign/campaign.ron`](../examples/sample_campaign/campaign.ron).

A campaign can be loaded from:

- a directory containing `campaign.ron`, or
- a direct `.ron` file path.

Validate a campaign with:

```bash
# Basic load check (does it parse and have missions?)
cargo run -p simterm -- --check --campaign ./path/to/campaign

# Advanced semantic validation (dangling refs, unreachable content, bad ranges)
cargo run -p simterm -- --doctor --campaign ./path/to/campaign
```

`--doctor` is stricter than `--check`: it reports **errors** (things that break the
campaign) and **warnings** (things that smell wrong but still load). It exits with a
non-zero status when there are errors. See [Validation Invariants](#validation-invariants).

Most fields have defaults. Define only the fields your campaign needs.

## `Campaign`

```ron
Campaign(
    name: "My Campaign",
    language: en,
    stages: ["RECON", "ENUM", "EXPLOIT", "POST"],
    intro: ["Opening line", "..."],
    missions: [ Mission(...) ],
    theme: ( ... ),
    features: ( ... ),
    easter_eggs: [ ( ... ) ],
    fortunes: ["..."],
    signals: ["ALPHA", "BRAVO"],
    achievements: [ ( ... ) ],
    commands: [ ( ... ) ],
    env: { "PATH": "/usr/bin:/bin" },
    processes: ["root 420 /usr/sbin/cron"],
    terminal: [ ( ... ) ],
)
```

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | string | required | Campaign name. |
| `language` | `es` or `en` | `es` | Language for generic engine/UI text. Campaign-authored story text is not translated automatically. |
| `stages` | string list | kill chain | Ordered progression stage names. Default is the pentest kill chain (`RECON/ENUM/EXPLOIT/POST`). Declaring your own marks a non-pentest domain. See [Domains, Stages, and Features](#domains-stages-and-features). |
| `intro` | string list | `[]` | Text shown when the campaign starts. |
| `missions` | `Mission` list | required | Ordered mission sequence. Must not be empty. |
| `theme` | `Theme` | neutral defaults | Branding and cosmetic UI text. |
| `features` | `Features` | heuristic | Toggles for domain mechanics (kill-chain help, trace meter, shell-gated VFS). See [Domains, Stages, and Features](#domains-stages-and-features). |
| `easter_eggs` | `EasterEgg` list | `[]` | Hidden flavor commands (no state change). |
| `fortunes` | string list | generic defaults | Text used by `fortune`. |
| `signals` | string list | generic defaults | Words used by the `signal` minigame. |
| `achievements` | `CampaignAchievement` list | `[]` | Campaign-defined achievements. |
| `commands` | `CampaignCommand` list | `[]` | Declarative commands with simple effects on game state. |
| `env` | string→string map | `{}` | Environment variables for `env`, `export`, and `$VAR` expansion. |
| `processes` | string list | `[]` | Extra `ps` rows, beyond those synthesized from `services`. |
| `terminal` | `TerminalCommand` list | `[]` | Authored realistic shell commands (presentational). |

## Domains, Stages, and Features

SimTerm is not hacking-specific. The pentesting kill chain is just the **default
domain**; the engine core (terminal emulation, campaign progression, meters,
declarative commands, VFS) is domain-agnostic. A campaign becomes a different
domain — forensics, operating a satellite, piloting a ship — purely with data.
See [`examples/demo_orbita`](../examples/demo_orbita/campaign.ron) for a complete
non-hacking campaign (rescue a space probe) driven entirely by data.

### `stages`

`stages` is the ordered list of progression stage names shown in the UI and used
to gate declarative commands. If omitted, it defaults to the pentest kill chain
(`RECON`, `ENUM`, `EXPLOIT`, `POST`).

```ron
stages: ["BOOT", "DIAGNOSE", "ALIGN", "LINK"],
```

Pentest missions advance stages through their built-in verbs (`nmap` reaches
`ENUM`, etc.). A custom domain advances them from data with the
[`ReachStage`](#effects-commandeffect) command effect.

### `features`

Whether a campaign uses **default stages** is the domain signal: a campaign that
keeps the kill chain is treated as pentest; one that declares its own stages is
treated as its own domain (its own commands in help, no kill-chain hints, its own
meters instead of a trace bar, freely explorable VFS…).

`features` lets you override that heuristic per toggle. Each is optional; when
omitted it falls back to the stages heuristic, so you can **mix** — e.g. a
forensics domain that still wants a shell-gated VFS.

```ron
features: (
    kill_chain:    Some(false),  // kill-chain help sections + start hints
    trace:         Some(false),  // detection "trace" meter: sidebar bar, hints, summary
    shell_for_vfs: Some(false),  // require a "shell" (foothold) before ls/cat/cd
),
```

| Toggle | Default | Controls |
|---|---|---|
| `kill_chain` | heuristic | Kill-chain help, start hints, exfil/completion wording, and whether pentest verbs (`nmap`, `exploit`…) exist at all. When off, those verbs are "command not found" and are hidden from help/autocomplete. |
| `trace` | heuristic | The detection/trace meter: the sidebar gauge, trace hints, stealth grade and clean-op achievement. When off, the sidebar shows the mission's own [meters](#meters). |
| `shell_for_vfs` | heuristic | Whether `ls`/`cat`/`cd`/`find` require a "shell" (POST stage). Off = files are freely explorable. |

## `Theme`

```ron
theme: (
    app_title: "TERMINAL",
    boot_header: "T E R M I N A L",
    boot_lines: ["Link established.", "Session opened."],
    overlay_title: " TERMINAL ",
    alert_title: " ALERT ",
    operator_prompt: "operator@console:~$ ",
    stealth_grades: ["GHOST", "CLEAN", "ACTIVE", "EXPOSED"],
    defense_messages: ["stage 1", "stage 2", "stage 3"],
    aborted_lines: ["Trace threshold reached."],
    credits: ["THE END"],
),
```

`stealth_grades` are assigned across the final trace ratio from best to worst.
`defense_messages` are used for active defense stages; if fewer messages are
provided than stages, the last one is reused.

## `EasterEgg`

```ron
(triggers: ["date"], lines: ["Internal clock: t={clock}."]),
(triggers: ["top", "ps"], lines: ["No suspicious processes."]),
```

`triggers` are command verbs. `lines` are printed when the verb is entered.
`{clock}` is replaced with the current mission clock. Easter eggs do not affect
game state.

## `CampaignCommand`

Declarative commands are campaign-defined verbs that **do** change game state,
without writing any Rust. They are the middle ground between built-in commands
(engine code) and easter eggs (flavor only). See
[Command Surface](COMMANDS.md) for how the three tiers compare.

```ron
commands: [
    (
        triggers: ["inspect", "look"],
        lines: ["You inspect the terminal."],
        effects: [
            AddLog("Inspection complete."),
            SetFlag("inspected_terminal"),
            AddTrace(2.0),
            UnlockAchievement("some-achievement-id"),
        ],
    ),
    (
        // Only available after "inspect", and hidden from help/autocomplete.
        triggers: ["wipe-notes"],
        hidden: true,
        conditions: [FlagSet("inspected_terminal")],
        lines: ["You wipe your notes: trace reduced."],
        effects: [AddTrace(-3.0), ClearFlag("inspected_terminal")],
    ),
]
```

| Field | Type | Default | Description |
|---|---|---|---|
| `triggers` | string list | required | Command verbs that run this command. |
| `lines` | string list | `[]` | Lines printed to the log. `{clock}` is substituted. |
| `effects` | `CommandEffect` list | `[]` | Ordered effects applied to game state. |
| `conditions` | `CommandCondition` list | `[]` | All must hold for the command to be available. |
| `hidden` | bool | `false` | If `true`, omitted from `help` and autocomplete (still runnable). |

If a verb's `conditions` are not met, it is treated as unrecognized and falls
through to easter eggs / the unknown-command message, so a command can appear and
disappear with state. Built-in verbs still take priority over campaign commands,
and campaign commands take priority over easter eggs with the same trigger.

### Effects (`CommandEffect`)

| Effect | Description |
|---|---|
| `AddLog("text")` | Print an extra line to the log (`{clock}` is substituted). |
| `SetFlag("name")` | Activate a persistent campaign flag. |
| `ClearFlag("name")` | Deactivate a persistent campaign flag. |
| `AddTrace(f32)` | Add trace (positive) or reduce it (negative). |
| `AddMeter("id", f32)` | Change a mission [meter](#meters) by its id (positive adds, negative subtracts), then evaluate its `on_limit`. |
| `ReachStage("NAME")` | Advance to the named [stage](#stages) (case-insensitive; never goes backward). |
| `UnlockAchievement("id")` | Unlock the `CampaignAchievement` with that `id`. |
| `CompleteMission` | Complete the current mission (as if the objective was met). |

Flags are campaign-scoped and persist across missions and saved progress; `reset`
clears them. Combine `CompleteMission` with a `conditions` guard (e.g. a flag) to
close a mission only when the player has done something specific.

### Conditions (`CommandCondition`)

| Condition | Available when |
|---|---|
| `FlagSet("name")` | The named flag is active. |
| `FlagNotSet("name")` | The named flag is not active. |
| `Mission("mission-id")` | The current mission has that `Mission.id`. |
| `Phase("post")` | The current stage is at least the named one. Names are matched against `Campaign.stages` (case-insensitive); with the default stages that means `recon`, `enum`, `exploit`, or `post`. |

## Terminal Emulation (`env`, `processes`, `TerminalCommand`)

SimTerm emulates a realistic shell. Most system commands are **synthesized** from
the host you already defined, so you rarely author their output. Shell output is
authentic POSIX (English); narrative text still uses the campaign `language`.

### `env` and `$VAR`

```ron
env: {
    "PATH": "/usr/local/bin:/usr/bin:/bin",
    "APP_ENV": "training",
    "APP_SECRET": "s3cr3t",   // a good place to hide clues
},
```

The engine also derives `USER`, `LOGNAME`, `HOME`, `PWD`, `HOSTNAME`, and `SHELL`.
`env` lists them all; `export VAR=value` sets a session variable (reset when you
change host/mission). `$VAR`, `${VAR}`, and `$?` (last exit code) expand in `echo`
and in `TerminalCommand` output.

### `processes`

Extra rows for `ps`, appended to the processes synthesized from the host's
`services`:

```ron
processes: ["root       420  /usr/sbin/cron"],
```

### `TerminalCommand`

For fictional CLIs the engine cannot synthesize. Presentational only (no game
effect — for effects use `commands`).

```ron
terminal: [
    (
        triggers: ["systemctl"],
        args: [
            ("status nginx", ["● nginx.service - active (running)"]),
        ],
        output: ["Usage: systemctl [OPTIONS...] {COMMAND} ..."],
        exit: 1,
    ),
],
```

| Field | Type | Default | Description |
|---|---|---|---|
| `triggers` | string list | required | Command verbs. |
| `output` | string list | `[]` | Default output when no `args` case matches. |
| `args` | `(string, string list)` list | `[]` | Per-argument output; key is the exact arg string after the verb. |
| `exit` | integer | `0` | Exit code left in `$?`. |
| `hidden` | bool | `false` | If `true`, omitted from `help` and autocomplete. |

Output supports templates: `{clock}`, `{user}`, `{host}`, `{ip}`, `{os}`,
`{cwd}`, `{env:NAME}`, plus `$VAR`/`$?`. System built-ins (`uname`, `ps`,
`netstat`, `env`, `grep`…) take priority, so `--doctor` warns if a `terminal`
trigger would be shadowed.

## `CampaignAchievement`

Campaigns can define their own achievements in data, in addition to the generic
engine achievements. They are shown by the `logros` command and saved with
campaign progress.

```ron
achievements: [
    (
        id: "read-secret",
        title: "Secret file",
        description: "Read the hidden dossier.",
        trigger: ReadFile("/srv/secret.txt"),
    ),
    (
        id: "finish-op1",
        title: "First operation closed",
        description: "Complete the first mission.",
        trigger: CompleteMission("op1"),
    ),
    (
        id: "ending-leak",
        title: "Burn it down",
        description: "Choose the leak ending.",
        trigger: ChooseEnding(mission: "final", choice: 3),
    ),
    (
        id: "campaign-clear",
        title: "Case closed",
        description: "Complete the campaign.",
        trigger: CampaignComplete,
    ),
],
```

| Field | Type | Description |
|---|---|---|
| `id` | string | Stable identifier. Must be unique inside the campaign. |
| `title` | string | Visible achievement title. |
| `description` | string | Visible description. May be omitted. |
| `trigger` | `CampaignAchievementTrigger` | Event that unlocks the achievement. |

Triggers:

| Trigger | Unlocks when |
|---|---|
| `ReadFile("/path")` | The player reads or decodes that VFS path. |
| `CompleteMission("mission-id")` | The mission with that `Mission.id` is completed. |
| `ChooseEnding(mission: "mission-id", choice: n)` | The player chooses `choose <n>` in that mission. `choice` is 1-based. |
| `CampaignComplete` | The campaign is completed. |

## `Mission`

```ron
Mission(
    id: "op1",
    name: "FIRST CONTACT",
    briefing: ["Mission text"],
    detection_limit: 120.0,
    meters: [ ( ... ) ],
    time_limit: Some(300),
    reactive: false,
    skill: 0.55,
    root_difficulty: 4,
    objective: Some("/root/flag.txt"),
    debrief: ["Debrief text"],
    entry: Active,
    endings: [ Ending(...) ],
    target: ( ... ),
    network: [ NetHost(...) ],
)
```

| Field | Type | Default | Description |
|---|---|---|---|
| `id` | string | required | Internal mission identifier. |
| `name` | string | required | Visible mission name. |
| `briefing` | string list | `[]` | Text shown at mission start. |
| `detection_limit` | float | `100.0` | Trace threshold for defeat (the pentest trace meter). |
| `meters` | `MeterDef` list | `[]` | Generic mission meters (fuel, oxygen, progress…). See [Meters](#meters). |
| `time_limit` | `Some(u32)` or `None` | `None` | Mission time window in action ticks. |
| `reactive` | bool | `false` | Enables active defense escalation. |
| `skill` | float | `0.5` | Operator skill, normally `0.0..=1.0`. |
| `root_difficulty` | integer | `5` | Local privilege escalation difficulty, `1..=10`. |
| `objective` | optional string | `None` | VFS path to exfiltrate. If absent, root access completes the mission. |
| `debrief` | string list | `[]` | Text shown after mission completion. |
| `entry` | `EntryVector` | `Active` | Opening mission state. |
| `endings` | `Ending` list | `[]` | Branching ending choices, usually on the final mission. |
| `target` | `TargetNode` | empty | Single-host target. |
| `network` | `NetHost` list | `[]` | Multi-host network. If present, `target` is ignored. |
| `music` | optional string | `None` | WAV path (relative to the campaign) for this mission's track. See [Music](#music-optional). |

## `EntryVector`

```ron
entry: Active,
entry: Cold(ports: [443]),
entry: Passive,
entry: Pivot(gateway: "bastion"),
```

- `Active` - classic active scan flow.
- `Cold` - selected ports are known and the mission starts in enumeration.
- `Passive` - passive discovery with `sniff`; active scanning is noisier.
- `Pivot` - requires `connect` before scanning.

## Meters

Meters are generic named resources with a threshold and an optional outcome —
fuel or oxygen that runs out, a progress bar that fills, a battery, integrity.
The pentest "trace" is a privileged built-in meter; these `MeterDef`s are
declared per mission and drive any domain. When the mission's `trace` feature is
off, they replace the trace gauge in the sidebar.

```ron
meters: [
    (
        id: "oxygen",
        label: Some("O₂"),
        start: 100.0,
        limit: 0.0,
        trigger: AtMost,
        on_limit: Fail,
        per_tick: -0.5,
    ),
    (
        id: "progress",
        start: 0.0,
        limit: 100.0,
        trigger: AtLeast,
        on_limit: Win,
    ),
],
```

| Field | Type | Default | Description |
|---|---|---|---|
| `id` | string | required | Stable id, referenced by `AddMeter`. Unique within the mission. |
| `label` | optional string | `id` | Visible label (sidebar gauge). |
| `start` | float | `0.0` | Initial value at mission start. |
| `limit` | float | required | Threshold that fires `on_limit`. |
| `trigger` | `AtLeast` or `AtMost` | `AtLeast` | `AtLeast` fires at `value >= limit` (rising: progress); `AtMost` fires at `value <= limit` (falling: fuel/oxygen). |
| `on_limit` | `None`/`Fail`/`Win` | `None` | Outcome when the threshold is reached: `Fail` loses the mission, `Win` completes it, `None` is just an indicator. |
| `per_tick` | float | `0.0` | Automatic drift per clock tick (e.g. `-0.5` depletes over time). `0.0` = only changes via `AddMeter`. |

Change a meter from data with the [`AddMeter`](#effects-commandeffect) command
effect. The clock only advances through actions that spend time, so a
declarative-only campaign should model cost with `AddMeter` per action rather
than relying on `per_tick`.

## `Ending`

```ron
(title: "Deliver the report", lines: ["Epilogue text"])
```

When endings are present, the player selects one with `choose <n>`.

## `TargetNode`

```ron
(
    hostname: "web-01.lab.local",
    ip: "10.0.0.5",
    os: "Linux 5.x",
    services: [
        (port: 80, name: "http", version: "nginx 1.20"),
    ],
    vulnerabilities: [
        (
            id: "WEB-LFI",
            name: "Local file inclusion",
            affected_service: 80,
            difficulty: 4,
            stealth_cost: 5,
            reliability: Reliable,
        ),
    ],
    filesystem: [ FsNode, ... ],
    accepts_token: Some("relay-token"),
    local_privesc: Some((kind: Sudo, note: "sudo vim can be abused")),
)
```

A `TargetNode` is a generic node (`hostname`/`ip`/`os` + `filesystem`) plus the
pentest payload (`services`, `vulnerabilities`, `accepts_token`, `local_privesc`).
The payload is **optional**: a non-pentest host declares only identity and an
optional `filesystem`, so its files are still explorable (see the `shell_for_vfs`
[feature](#features)).

```ron
// A generic node with no pentest payload.
target: (hostname: "probe-7", ip: "3.2 AU", os: "RTOS-9", filesystem: [ ... ]),
```

Service names determine enumeration categories:

- `http`, `https`, `http-proxy`, `http-alt` -> web.
- `smb`, `netbios`, `netbios-ssn`, `microsoft-ds` -> SMB.
- `ssh` -> SSH/login.
- `mysql`, `pgsql`, `postgresql`, `redis`, `mongodb`, `mssql`, `oracle` ->
  database.
- everything else -> generic.

`Service` supports an optional `requires: Some("token")` field. If present, the
service can be discovered but cannot be enumerated until the player has that
foothold token.

`Vulnerability.reliability` accepts:

- `Reliable` - deterministic once identified, appropriate for credentials,
  no-auth bugs, LFI, SQL injection, and simple bypasses.
- `Unstable` - probabilistic, appropriate for fragile RCE, deserialization,
  memory corruption, SSRF, races, and timing-dependent vectors.

If omitted, reliability defaults to `Unstable`.

## Local Privilege Escalation

```ron
local_privesc: Some((
    kind: Sudo,
    note: "sudo vim can spawn a root shell",
)),
```

`kind` can be:

- `Sudo`
- `Suid`
- `Kernel`
- `Cron`

This data models a local escalation vector. The current frontend exposes it
through `linpeas`, `sudo -l`, `suid`, and `sysinfo`.

## `FsNode`

```ron
Dir(name: "home", children: [
    Dir(name: "op", children: [
        File(
            name: "id_rsa",
            content: ["private key material"],
            root: false,
            loot: Some((privesc_key: true)),
        ),
    ]),
]),
File(
    name: "flag.txt",
    content: ["FLAG{example}"],
    root: true,
),
```

Directories contain child nodes. Files support:

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | string | required | File name. |
| `content` | string list | `[]` | Text shown by `cat`. |
| `root` | bool | `false` | Requires root to read. |
| `loot` | optional `Loot` | `None` | Reward granted the first time the file is read. |
| `binary` | optional `Binary` | `None` | Reversible binary metadata. |
| `encoding` | optional `Encoding` | `None` | Encoded file content metadata. |

## `Loot`

```ron
loot: Some((
    skill: 0.05,
    credential: Some("user:pass"),
    note: Some("Useful operator note"),
    privesc_key: true,
    foothold_token: Some("relay-token"),
    wordlist: false,
))
```

`privesc_key: true` unlocks deterministic privilege escalation. Files with this
flag should be readable before root; they are the path to root.

`foothold_token` can be reused with `login` on hosts whose `accepts_token`
matches.

## Hashes, Binaries, and Encoded Files

The engine data model also supports optional post-exploitation puzzle metadata.
The current frontend exposes these through `john`/`hashcat`, `strings`,
`disasm`/`objdump`/`r2`, `solve`, `base64`, and `xor`.

```ron
hash: Some((
    algo: "sha512crypt",
    strength: 6,
    needs_wordlist: true,
    yields: Token("relay-token"),
)),
binary: Some((
    strings: ["usage: authd", "cmp_key"],
    disasm: ["cmp eax, 0x1f3b"],
    secret: "AX29",
    yields: PrivescKey,
    hint: Some("The key is obfuscated."),
)),
encoding: Some(Base64),
encoding: Some(Xor("key")),
```

`Reward` values are `Skill(f32)`, `Credential(String)`, `Token(String)`, and
`PrivescKey`.

## `NetHost`

```ron
(
    target: ( ... ),
    links: ["db", "app"],
    entry: true,
    objective: Some("/root/data.bin"),
)
```

Use `network` instead of `target` for multi-host missions. Mark at least one
host as `entry: true`, connect hosts with `links`, and place objectives on the
host that should complete the mission.

## Music (optional)

The frontend plays an optional per-mission track. There are two ways to attach
one, checked in this order:

1. **Explicit field** — set `music` on the mission to a WAV path relative to the
   campaign directory. Put the file anywhere you like:

   ```ron
   Mission(
       id: "op1",
       name: "FIRST CONTACT",
       music: Some("music/intro_theme.wav"),
       // ...
   )
   ```

2. **Naming convention** — if `music` is omitted, the frontend looks for
   `music/mission_{N}_theme.wav` next to `campaign.ron` (`N` is the 1-based
   mission number):

   ```text
   my_campaign/
     campaign.ron
     music/
       mission_1_theme.wav
       mission_2_theme.wav
       ...
   ```

Tracks loop with a short fade-in and switch when the mission changes. Missions
with neither an explicit file nor a convention file are silent. If there is no
audio device the game runs silently; `--no-music` disables audio entirely. Only
WAV is decoded. Audio is a frontend feature: the engine never touches it — the
`music` field is just data it carries.

## Validation Invariants

`--check` and the engine tests expect campaigns to stay completable:

- The campaign has at least one mission.
- Any objective path points to a real VFS file.
- **Pentest missions** additionally need each playable host to have services and
  vulnerabilities, and the objective host to provide a deterministic route to
  root (e.g. readable `privesc_key` loot). Non-pentest hosts have no such
  requirement — they complete via a `Win` meter or a `CompleteMission` command.

### `--doctor` semantic checks

`--doctor` runs a deeper analysis than `--check` and prints errors and warnings.
It exits non-zero if there are any **errors**. It reports at least:

**Errors** (break the campaign):

- Duplicate or empty mission IDs.
- A campaign with no missions.
- A `Mission.objective` (or `NetHost.objective`) that points to a path with no
  file in that host's VFS.
- A `Vulnerability.affected_service` port that is not among the host's `services`.
- A `Service.requires` token that can never be obtained anywhere in the campaign.
- Duplicate network hostnames, or `links` (pivot) references to hosts that do not
  exist in the mission's network.
- Duplicate achievement IDs.
- Achievement triggers that reference a missing mission or an out-of-range
  `ChooseEnding` choice.
- Declarative-command effects/conditions that reference a missing achievement or
  mission, a `Phase`/`ReachStage` name not in `stages`, or an `AddMeter` id not
  declared in any mission.
- Duplicate or empty meter ids within a mission.

**Warnings** (smell wrong, still load):

- Out-of-range `skill`, `root_difficulty`, `detection_limit`, `time_limit`, or
  vulnerability `difficulty`.
- `accepts_token` that is never obtainable (`login` will never work there).
- Easter eggs, declarative commands, or terminal commands whose triggers collide
  with built-in/system commands (they would be shadowed) or with each other.
- Achievement `ReadFile` triggers whose path is not in any VFS.
- Declarative `FlagSet(...)` conditions for a flag no command ever sets.
- `TerminalCommand.exit` outside `0..=255`, or `{env:NAME}` templates referencing
  a variable that is neither in `env` nor derived.

The engine exposes this as `simterm_engine::validate_campaign(&Campaign, &reserved_verbs)`,
so other tools can reuse it. The frontend passes its reserved built-in verbs so the
engine stays independent of the terminal command set.
