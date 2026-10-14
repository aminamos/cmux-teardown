# cmux Founders Edition ($30/mo) vs freestyle.sh tiers

> Yes — cmux Cloud VMs run on freestyle.sh. Per cmux's own backend doc
> (`docs/cloud-vm-backend-rollout-todo.md`): *"Freestyle on the public platform
> (`api.freestyle.sh`) is the only active Cloud VM provider"* (2026-09-02).
> (Blaxel was removed as a provider; the Freestyle driver was collapsed onto
> the public platform in cmux 0.2.9.)

## What Founders Edition $30/mo gets you

From [manaflow-ai/cmux#542](https://github.com/manaflow-ai/cmux/issues/542)
(mirrored in the cmux README):

- Prioritized feature requests / bug fixes
- Early access: cmux AI (context on every workspace/tab/panel)
- Early access: iOS app (TestFlight, terminals synced desktop <-> phone)
- **Early access: Cloud VMs**
- Early access: voice mode
- Founder's personal iMessage/WhatsApp

**Cloud VM entitlement** (from the cmux billing runbook
`skills/cmux-billing/SKILL.md` — `web/services/vms/entitlements.ts` +
`machineSpec.ts`):

- **Founders = Pro tier.** Same pool, same limits ("Founder's Edition and cmux
  Pro are the same tier and get the same stuff").
- Up to **5 active Cloud VMs** sharing **20 vCPUs + 40 GB RAM** (one pool per
  paid seat; you split it however you like — pooled, *not* per-VM).
- Largest single VM: **16 vCPU / 32 GB RAM** (the `xl` row).
- Disk is **per-VM, not pooled**: grows independently up to 256 GB.
  Default machine: 8 GB RAM / 32 GB disk.
- Only `provisioning` and `running` VMs count against the pool; **paused and
  stopped VMs don't count**.
- Current checkout keys (live catalog): Go $10/mo (one 2vCPU/4GB VM),
  Pro $50/mo, Max $200/mo (80 vCPU / 160 GB pool, 32vCPU-max `2xl` VMs),
  Team $60/user/mo. The **$30/mo price is the grandfathered
  `cmux-pro-monthly` key** — existing Founders subscriptions stay on it.

## What freestyle.sh gets you (raw)

From <https://www.freestyle.sh/pricing> (usage rates + limits table):

**Usage rates** (same allowances on every plan; paid-plan fees cover overage
dollar-for-dollar; on Free, allowances are hard stops):

| Resource | Price | Included/mo |
| --- | --- | --- |
| vCPU-hour | $0.04032 | 200 |
| GiB-hour memory | $0.0129 | 400 |
| GiB-hour storage | $0.000086 | 60,000 |
| GB data transfer | $0.02 | 50 (Free) / 500 (paid) |

**Plan limits:**

| Limit | Free ($0) | Hobby ($50/mo) | Pro ($500/mo) |
| --- | --- | --- | --- |
| Concurrent VMs | 10 | 40 | 400 |
| Saved VMs | 10 | 200 | 4,000 |
| Snapshots | 10 | 1,000 | 12,000 |
| Total vCPUs | 40 | 800 | 16,000 |
| Total memory | 80 GiB | 1,600 GiB | 32,000 GiB |
| Total disk | 320 GiB | 3,125 GiB | 62,500 GiB |
| Data transfer | 50 GB/mo | 500 GB/mo | 500 GB/mo |
| Persistent VMs / snapshots | no | yes | yes |
| Custom sizing | no | yes | yes |
| Max vCPU per VM | 4 | 8 | 32 |
| Max memory per VM | 8 GiB | 16 GiB | 64 GiB |
| Max disk per VM | 32 GiB | 64 GiB | 256 GiB |

## Head-to-head: Founders $30 vs Freestyle

| | cmux Founders ($30/mo flat) | Freestyle Free ($0) | Freestyle Hobby ($50/mo + usage) |
| --- | --- | --- | --- |
| Model | Flat pool, no metering | 200 vCPU-h + 400 GiB-h/mo, then hard stop | $50 credit vs usage, then metered |
| Active VMs | 5 (running/provisioning only) | 10 concurrent | 40 concurrent |
| vCPU pool | 20 | 40 total cap (allowance-limited) | 800 total cap |
| RAM pool | 40 GB | 80 GiB total cap (allowance-limited) | 1,600 GiB total cap |
| Max single VM | 16 vCPU / 32 GB | 4 vCPU / 8 GiB | 8 vCPU / 16 GiB |
| Persistence | yes (pause/stop free) | **no** | yes |
| Disk | up to 256 GB per VM | up to 32 GB per VM | up to 64 GB per VM |
| Images/API | cmux devbox only | full Freestyle API | full Freestyle API |

**Reading it:**

- Founders is the better deal than raw Freestyle **if you live inside cmux**:
  flat $30 for a persistent 20vCPU/40GB pool with 16vCPU-capable VMs beats
  Free's non-persistent 4vCPU-max VMs, and undercuts Hobby's $50 while
  allowing bigger single VMs (16 vs 8 vCPU).
- Raw Freestyle wins **if you need your own images/snapshots/API control**
  (e.g. a custom Windows-RE cloud backend): Founders locks you to cmux's
  devbox image and its 5-VM / 20vCPU / 40GB ceiling with no overage path
  except upgrading to Max ($200/mo).
- For the app in [windows-rust-app-plan.md](windows-rust-app-plan.md): start
  on **Freestyle Free** for API prototyping (mind the 4vCPU/no-persistence
  caps), move to **Hobby** once you need persistent snapshots and bigger VMs.
  Don't build on Founders — it's cmux-app credit, not API credit.
