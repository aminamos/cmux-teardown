# cmux-teardown

Teardown notes for building a **Windows-first Rust reverse-engineering app**:
what cmux Cloud VMs actually give you vs raw [freestyle.sh](https://www.freestyle.sh/pricing),
plus tool details on [knife](https://github.com/bl4ckr0ss3/knife) and
[REA](https://github.com/morluto/rea) to reuse when designing the app
(Rust core, [iroh](https://www.iroh.computer/) networking, cloud VMs for heavy lifting).

## TL;DR — the question that started this

**cmux Founders Edition ($30/mo) vs freestyle.sh tiers:**

- **Founders Edition = cmux Pro tier.** Up to **5 active Cloud VMs** sharing one
  pool of **20 vCPUs / 40 GB RAM**. Biggest single VM: 16 vCPU / 32 GB RAM.
  Disk is per-VM (up to 256 GB), not pooled. Only `provisioning`/`running` VMs
  count; paused/stopped are free. Confirmed: cmux runs its Cloud VMs on
  `api.freestyle.sh` (the public Freestyle platform) — so Founders is a
  flat-rate wrapper over Freestyle infra.
- **Freestyle Free ($0):** up to 10 concurrent / 10 saved VMs, 200 vCPU-h +
  400 GiB-h + 60,000 GiB-h storage + 50 GB transfer included monthly, then
  **hard stop** (no overage). Max 4 vCPU / 8 GiB / 32 GiB disk per VM,
  no persistent VMs/snapshots, no custom sizing.
- **Freestyle Hobby ($50/mo + usage over $50):** 40 concurrent / 200 saved,
  500 GB transfer, persistent VMs/snapshots, custom sizing, max 8 vCPU /
  16 GiB / 64 GiB per VM. Same monthly compute allowances as Free; the $50
  covers overage dollar-for-dollar.
- **Freestyle Pro ($500/mo + usage):** 400 concurrent / 4,000 saved, max
  32 vCPU / 64 GiB / 256 GiB per VM.

So: Founders $30/mo buys you a **flat 20 vCPU / 40 GB pool with 16vCPU-max VMs
and persistence** — roughly between Freestyle Free (capped, non-persistent,
4vCPU-max) and Hobby (persistent but 8vCPU-max per VM), at a lower flat price
than Hobby, but locked to cmux's images/limits instead of raw Freestyle API access.

Full tables + sources: [docs/cmux-founders-vs-freestyle.md](docs/cmux-founders-vs-freestyle.md).

## Docs

| Doc | What |
| --- | ---- |
| [docs/cmux-founders-vs-freestyle.md](docs/cmux-founders-vs-freestyle.md) | Founders $30/mo entitlements vs Freestyle Free/Hobby/Pro, with sources |
| [docs/knife-tool.md](docs/knife-tool.md) | knife: Rust static-RE toolkit (CLI/TUI/MCP), full command + MCP tool inventory |
| [docs/rea-tool.md](docs/rea-tool.md) | REA: agent-first RE over MCP (providers, evidence contract, target matrix) |
| [docs/windows-rust-app-plan.md](docs/windows-rust-app-plan.md) | App plan: Windows Rust core + iroh + cloud VMs, reusing knife/REA patterns |

## Sources (checked 2026-10-14)

- Freestyle pricing page: <https://www.freestyle.sh/pricing>
- cmux Founder's Edition: <https://github.com/manaflow-ai/cmux/issues/542>
  (mirrored in cmux README `#founder's-edition`)
- cmux billing runbook (`skills/cmux-billing/SKILL.md` in supermux mirror):
  pool sizes, machine rows, grandfathered $30 price key `cmux-pro-monthly`
- cmux Cloud VM backend doc: `docs/cloud-vm-backend-rollout-todo.md` in
  manaflow-ai/cmux — "Freestyle on the public platform (`api.freestyle.sh`)
  is the only active Cloud VM provider" (2026-09-02)
- knife: <https://github.com/bl4ckr0ss3/knife> (`reknife` on crates.io, MIT)
- REA: <https://github.com/morluto/rea> (`rea-agents` on npm, MIT)
