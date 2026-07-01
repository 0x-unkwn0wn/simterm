# SimTerm Command Surface

This document lists the commands that the current SimTerm frontend can emulate
inside a campaign.

SimTerm campaigns can use commands in four tiers:

- **Built-in commands** (incl. **system commands**): implemented by the framework
  runtime/frontend. Game-loop verbs (`nmap`, `exploit`…) change state; system
  verbs (`uname`, `ps`, `env`…) synthesize a realistic shell from host data.
  Their metadata lives in a single registry (`crates/simterm/src/registry.rs`)
  that feeds autocomplete, help, and the `--doctor` collision checks.
- **Declarative campaign commands**: defined in campaign data through `commands`.
  They **do** change game state through a fixed set of simple effects (flags,
  trace, achievements, mission completion), without any Rust. See
  [Campaign Format › CampaignCommand](CAMPAIGN_FORMAT.md#campaigncommand).
- **Authored terminal commands**: defined in campaign data through `terminal`.
  They emit realistic, templated output (for fictional CLIs like `systemctl`) and
  do **not** change game state. See
  [Campaign Format › TerminalCommand](CAMPAIGN_FORMAT.md#terminalcommand).
- **Easter eggs**: defined in campaign data through `easter_eggs`. They print
  campaign-authored text and do **not** change game state.

Resolution order for a typed verb: built-in/system first, then declarative
campaign command (if its conditions hold), then authored terminal command, then
easter egg, then a real shell's `bash: <verb>: command not found`. So a campaign
cannot override a built-in verb, and `--doctor` warns when a campaign trigger
would be shadowed by one.

## Shell tone

Command output is authentic POSIX (English): `bash: foo: command not found`,
`uid=0(root)`, `cat: x: No such file or directory`. Narrative text (briefings,
debriefs, mission log, loot) still follows the campaign `language`. A
Spanish-speaking operator still sees English shell output, as on a real box.

## System Commands (synthesized)

Available once you have a shell on the host. Output is generated from the host
definition the author already wrote (`TargetNode`) plus the `env`/`processes`
campaign data — no per-command authoring required.

| Command | Synthesized from |
|---|---|
| `whoami` / `id` | Root vs user session. |
| `uname [-a]` | `os` + hostname. |
| `hostname [-f]` | Host name (short / FQDN). |
| `ps [aux]` | `services` (name→process) + base procs + `processes`. |
| `netstat -tlnp` / `ss` | `services` (listening ports). |
| `ifconfig` / `ip a` | Host `ip` + loopback. |
| `env` | `env` map + derived (`USER`, `HOME`, `PWD`, `HOSTNAME`, `SHELL`). |
| `export VAR=val` | Sets a session variable (reset on mission change). |
| `grep PAT FILE`, `head`/`tail [-n N] FILE`, `wc FILE`, `file FILE` | The VFS. |

Variables expand in `echo` and templates: `$VAR`, `${VAR}`, and `$?` (last exit
code). Unknown variables expand to empty, as in bash.

The bundled sample uses a simulated technical-operation loop. Future frontends
or runtime extensions can add different command sets.

## General Commands

| Command | Aliases | Purpose |
|---|---|---|
| `help` | `h`, `?` | Show in-game command help. |
| `status` | | Show level, phase, shell, clock, detection, and outcome. |
| `logs` | | Jump to the end of the log. |
| `clear` | `cls` | Clear the visible console. |
| `reset` | `newgame` | Restart the campaign and clear saved progress. |
| `quit` | `exit`, `q` | Leave the game. |
| `history` | | Show command history. |
| `echo <text>` | | Print text back to the log. |

## Recon and Discovery

| Command | Aliases | Purpose |
|---|---|---|
| `target` | `host` | Show the current target and discovered services. |
| `nmap` | `scan`, `recon` | Active discovery of services. |
| `sniff` | `intercept`, `listen` | Passive discovery, one service at a time. |
| `connect [host]` | | Establish gateway access for pivot-entry missions. |

## Enumeration Tools

These commands take a port number:

```text
nikto 80
sqlmap 443
probe 8080
```

| Command | Best fit |
|---|---|
| `probe <port>` | Generic services. |
| `nikto <port>` | Web services. |
| `gobuster <port>` | Web paths and files. |
| `enum4linux <port>` | SMB / NetBIOS-style services. |
| `hydra <port>` | SSH/login-style services; intentionally noisy. |
| `sqlmap <port>` | Web/database services. |

## Findings and Actions

| Command | Aliases | Purpose |
|---|---|---|
| `intel` | | List discovered findings and estimated confidence. |
| `searchsploit <id>` | `verify <id>`, `research <id>` | Research a finding. |
| `exploit <id>` | `run <id>` | Attempt exploitation of a finding. |
| `login` | `ssh` | Use a reusable token if the host accepts it. |
| `cleanup` | `covertracks`, `cleanlogs` | Reduce trace with cost/risk. |

## Multi-Host Commands

| Command | Aliases | Purpose |
|---|---|---|
| `netmap` | `lan`, `neighbors` | Discover reachable internal hosts. |
| `pivot <host>` | `jump <host>` | Move context to a reachable host. |

## Virtual Filesystem Commands

These commands are available after the campaign gives the player a foothold.

| Command | Aliases | Purpose |
|---|---|---|
| `ls [path]` | `dir [path]` | List a directory or file. |
| `cd [path]` | | Change current directory. |
| `pwd` | | Show current directory. |
| `cat <path>` | `read <path>`, `type <path>` | Read a file. |
| `find [text]` | | Search file and directory names. |
| `whoami` | `id` | Show current session identity. |
| `privesc` | `escalate`, `root` | Attempt local privilege escalation. |
| `loot` | `creds` | Show collected credentials and notes. |

## Offline Analysis, Reversing, and Decoding

These commands run after the player has a foothold. They spend in-game clock but
do not add network noise, except local enumeration commands that intentionally
add a small amount of trace.

| Command | Aliases | Purpose |
|---|---|---|
| `john <path>` | `hashcat <path>` | Crack a looted hash file. The file must be read with `cat` first. |
| `strings <path>` | | Show printable strings from a reversible binary. |
| `disasm <path>` | `objdump <path>`, `r2 <path>` | Show campaign-authored pseudo-disassembly for a reversible binary. |
| `solve <path> <secret>` | | Submit the secret extracted from a reversible binary. |
| `base64 <path>` | | Decode a Base64-encoded VFS file. |
| `xor <path> <key>` | | Decode an XOR-encoded VFS file with the supplied key. |

Campaign authors configure these through `Loot.hash`, `Loot.wordlist`,
`Binary`, `Encoding::Base64`, and `Encoding::Xor`.

## Local Privilege Escalation Enumeration

These commands reveal `TargetNode.local_privesc` vectors when the host defines
one. Revealing a vector unlocks deterministic `privesc` on that host.

| Command | Purpose |
|---|---|
| `linpeas` | Broad local enumeration; covers every local privesc kind. |
| `sudo -l` | Reveals `LocalKind::Sudo`. |
| `suid` | Reveals `LocalKind::Suid`. |
| `sysinfo` | Reveals `LocalKind::Kernel`. |

`LocalKind::Cron` is intentionally covered by `linpeas`.

## Endings and Choice

| Command | Aliases | Purpose |
|---|---|---|
| `choose <n>` | `deliver <n>` | Select a campaign ending when choices are available. |

## Built-In Mini-Experience Commands

These are generic terminal flavor/minigame commands. Campaign data can influence
some of their content through `fortunes` and `signals`.

| Command | Aliases | Purpose |
|---|---|---|
| `fortune` | | Print a random campaign fortune. |
| `signal` | | Intercept a coded signal from campaign words. |
| `decode <text>` | `decrypt <text>` | Decode the latest signal-style text. |
| `crack <0000-9999>` | | Try a numeric keypad code. |
| `mastermind [guess]` | `bulls [guess]`, `mm [guess]` | Play the mastermind-style minigame. |

## Campaign Flavor Commands

Campaigns can define extra harmless commands in `easter_eggs`:

```ron
easter_eggs: [
    (
        triggers: ["date", "clock"],
        lines: ["Internal clock: t={clock}."],
    ),
    (
        triggers: ["about"],
        lines: ["This terminal belongs to the campaign author."],
    ),
],
```

Rules:

- `triggers` are command verbs.
- `lines` are printed to the log.
- `{clock}` is replaced with the current mission clock.
- Easter eggs do not change game state.
- Built-in command names take priority, so avoid triggers like `help`, `cat`,
  `nmap`, or `quit`. Run `--doctor` to catch collisions.

## Declarative Campaign Commands

When you need a campaign verb that actually affects the run (set a flag, nudge
trace, unlock an achievement, or complete a mission), use `commands` instead of
`easter_eggs`:

```ron
commands: [
    (
        triggers: ["inspect", "look"],
        lines: ["You inspect the terminal."],
        effects: [
            SetFlag("inspected_terminal"),
            AddTrace(2.0),
        ],
    ),
],
```

Non-hidden declarative commands appear in `help` and in Tab autocomplete. Their
effects run in the engine runtime, not the frontend. Full reference:
[Campaign Format › CampaignCommand](CAMPAIGN_FORMAT.md#campaigncommand).

## Authored Terminal Commands

For fictional CLIs the engine cannot synthesize (a service manager, a banner, an
app), use `terminal`: realistic, templated, per-argument output with an exit
code, and no game-state effect.

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

Templates: `{clock}`, `{user}`, `{host}`, `{ip}`, `{os}`, `{cwd}`, `{env:NAME}`,
and `$VAR`/`$?`. Full reference:
[Campaign Format › TerminalCommand](CAMPAIGN_FORMAT.md#terminalcommand).

## Input Quality-of-Life

| Key | Purpose |
|---|---|
| `Tab` | Autocomplete command names, tools, paths, and relevant ids. |
| `Up` / `Down` | Browse command history. |
| `PageUp` / `PageDown` | Scroll the log. |
| `Esc` | Clear the current input line. |

## Extension Notes

To add a new emulated command:

1. Add its metadata (name, aliases, category, kind) to the registry in
   `crates/simterm/src/registry.rs` so autocomplete, help, and `--doctor`
   collision checks stay in sync.
2. Add parsing in `crates/simterm/src/command.rs`.
3. Add runtime behavior in `crates/engine/src/runtime/actions.rs` if it changes
   game state.
4. Add presentation-only behavior in `crates/simterm/src/app.rs` or a related
   frontend module if it only affects UI/log output.
5. Expose configurable story-specific text as campaign data instead of
   hardcoding it.

If the behavior can be expressed with the existing declarative effects, prefer a
`CampaignCommand` (data) over new Rust.
