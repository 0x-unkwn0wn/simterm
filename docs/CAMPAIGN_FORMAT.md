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
cargo run -p simterm -- --check --campaign ./path/to/campaign
```

Most fields have defaults. Define only the fields your campaign needs.

## `Campaign`

```ron
Campaign(
    name: "My Campaign",
    language: en,
    intro: ["Opening line", "..."],
    missions: [ Mission(...) ],
    theme: ( ... ),
    easter_eggs: [ ( ... ) ],
    fortunes: ["..."],
    signals: ["ALPHA", "BRAVO"],
    achievements: [ ( ... ) ],
)
```

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | string | required | Campaign name. |
| `language` | `es` or `en` | `es` | Language for generic engine/UI text. Campaign-authored story text is not translated automatically. |
| `intro` | string list | `[]` | Text shown when the campaign starts. |
| `missions` | `Mission` list | required | Ordered mission sequence. Must not be empty. |
| `theme` | `Theme` | neutral defaults | Branding and cosmetic UI text. |
| `easter_eggs` | `EasterEgg` list | `[]` | Hidden flavor commands. |
| `fortunes` | string list | generic defaults | Text used by `fortune`. |
| `signals` | string list | generic defaults | Words used by the `signal` minigame. |
| `achievements` | `CampaignAchievement` list | `[]` | Campaign-defined achievements. |

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
| `detection_limit` | float | `100.0` | Trace threshold for defeat. |
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

## Validation Invariants

`--check` and tests expect campaigns to stay completable:

- The campaign has at least one mission.
- Each playable host has services and vulnerabilities.
- Any objective path points to a real VFS file.
- The host that contains the objective provides a deterministic route to root,
  such as readable `privesc_key` loot.
