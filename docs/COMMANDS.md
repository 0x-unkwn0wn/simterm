# SimTerm Command Surface

This document lists the commands that the current SimTerm frontend can emulate
inside a campaign.

SimTerm campaigns can use commands in two ways:

- **Built-in commands**: implemented by the framework runtime/frontend. They
  change game state.
- **Campaign flavor commands**: defined in campaign data through `easter_eggs`.
  They print campaign-authored text and do not change game state.

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
  `nmap`, or `quit`.

## Input Quality-of-Life

| Key | Purpose |
|---|---|
| `Tab` | Autocomplete command names, tools, paths, and relevant ids. |
| `Up` / `Down` | Browse command history. |
| `PageUp` / `PageDown` | Scroll the log. |
| `Esc` | Clear the current input line. |

## Extension Notes

To add a new emulated command:

1. Add parsing in `crates/simterm/src/command.rs`.
2. Add runtime behavior in `crates/engine/src/runtime/actions.rs` if it changes
   game state.
3. Add presentation-only behavior in `crates/simterm/src/app.rs` or a related
   frontend module if it only affects UI/log output.
4. Expose configurable story-specific text as campaign data instead of
   hardcoding it.
