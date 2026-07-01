# Modding Guide

SimTerm is a framework for building immersive terminal-based games and
experiences. This guide explains how to build a playable SimTerm campaign
without changing Rust code. For the full field reference, see
[CAMPAIGN_FORMAT.md](CAMPAIGN_FORMAT.md).

## 1. Start from the Sample

```bash
cp -r examples/sample_campaign campaigns/my_campaign
```

Edit `campaigns/my_campaign/campaign.ron`, then validate it:

```bash
cargo run -p simterm -- --check --campaign ./campaigns/my_campaign
```

Run it:

```bash
cargo run -p simterm -- --campaign ./campaigns/my_campaign
```

## 2. Build a Completable Experience

The bundled sample campaign currently demonstrates a simulated hacking loop. A
mission using that loop should provide a complete path through the scenario:

1. The target has at least one exposed service.
2. The target has real hidden vulnerabilities tied to those service ports.
3. At least one entry route is reasonable for the player to find.
4. The player can obtain a foothold by exploiting a real finding or by using
   `login` with a matching foothold token.
5. The player can reach root through a deterministic route, usually readable
   `privesc_key` loot.
6. If the mission has an `objective`, the objective path points to a real VFS
   file and is readable after root.

Minimal single-host shape:

```ron
Mission(
    id: "op1",
    name: "FIRST CONTACT",
    briefing: ["Map the host and recover the file."],
    objective: Some("/root/flag.txt"),
    target: (
        hostname: "box-01.lab.local",
        ip: "10.0.0.10",
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
        filesystem: [
            Dir(name: "home", children: [
                Dir(name: "op", children: [
                    File(
                        name: "id_rsa",
                        content: ["local key material"],
                        loot: Some((privesc_key: true)),
                    ),
                ]),
            ]),
            Dir(name: "root", children: [
                File(
                    name: "flag.txt",
                    content: ["FLAG{example}"],
                    root: true,
                ),
            ]),
        ],
    ),
)
```

## 3. Tune the Opening

Use `entry` to change how the mission starts:

- `Active` - the player should begin with `nmap`.
- `Cold(ports: [443])` - the player already knows selected ports.
- `Passive` - the player should prefer `sniff` because active scanning is
  noisier.
- `Pivot(gateway: "bastion")` - the player must `connect` before scanning.

## 4. Use Tool Affinity

Players get better results when they match tools to services:

- Web: `nikto`, `gobuster`, `sqlmap`.
- SMB: `enum4linux`.
- SSH/login: `hydra`, which is intentionally noisy.
- Database: `sqlmap`.
- Unknown services: `probe`.

Avoid making the only practical entry route an SSH brute-force unless the
mission is intentionally high-risk.

## 5. Design for Imperfect Information

Enumeration can produce false positives. Good mission design gives players ways
to reason:

- Use sensible vulnerability names.
- Set `Reliable` for deterministic vectors.
- Set `Unstable` for fragile or timing-dependent vectors.
- Keep early mission difficulty low enough that failed guesses do not exhaust
  the detection budget immediately.
- Include useful VFS notes once the player has a foothold.

## 6. Multi-Host Missions

Use `network` when a mission spans several hosts:

```ron
network: [
    (
        target: (hostname: "edge.lab.local", ...),
        links: ["relay"],
        entry: true,
        objective: None,
    ),
    (
        target: (
            hostname: "relay.lab.local",
            accepts_token: Some("relay-token"),
            ...
        ),
        links: [],
        entry: false,
        objective: Some("/root/blueprints.dat"),
    ),
],
```

A common pattern:

1. The entry host contains loot with `foothold_token: Some("relay-token")`.
2. A linked host has `accepts_token: Some("relay-token")`.
3. The player runs `netmap`, `pivot relay`, then `login`.

## 7. Narrative and Theme

Use campaign data for all story and identity:

- `intro` for campaign opening text.
- `briefing` and `debrief` for mission text.
- file `content` for environmental storytelling.
- `endings` for final choices.
- `theme` for title, prompt, boot text, alerts, stealth grades, defense
  messages, and credits.
- `easter_eggs`, `fortunes`, and `signals` for optional flavor.

Do not add story-specific branches to Rust code.

## 8. Offline Analysis and Reversing Puzzles

For deeper terminal interactions, use the advanced VFS fields documented in
[CAMPAIGN_FORMAT.md](CAMPAIGN_FORMAT.md):

- `Loot.hash` plus `john`/`hashcat` for offline hash cracking.
- `Loot.wordlist` for campaign-gated dictionary attacks.
- `Binary` plus `strings`, `disasm`, and `solve` for reversing-style puzzles.
- `Encoding::Base64` plus `base64` for encoded files.
- `Encoding::Xor` plus `xor <path> <key>` for keyed decoding.
- `TargetNode.local_privesc` plus `linpeas`, `sudo -l`, `suid`, or `sysinfo`
  for discoverable local escalation vectors.

## 9. Validate Often

Run `--check` whenever you change structure:

```bash
cargo run -p simterm -- --check --campaign ./campaigns/my_campaign
```

Common issues:

| Symptom | Likely cause |
|---|---|
| RON parse error | Missing comma, mismatched parenthesis, or malformed `Some(...)`. |
| Objective is not a file | `objective` does not match an actual VFS path. |
| Mission cannot complete | The objective host lacks a deterministic root route. |
| Entry route is too punishing | Only viable vulnerability is too difficult or too noisy. |
| Easter egg does not trigger | Trigger collides with a built-in command or is missing from `triggers`. |

## 10. Publishing a Campaign

Distribute your campaign as a directory containing `campaign.ron` and any
campaign-adjacent assets you choose to support. Players run it with:

```bash
simterm --campaign ./your_campaign
```

The SimTerm framework is MIT licensed. Your campaign content is separate and can
use the license you choose.
