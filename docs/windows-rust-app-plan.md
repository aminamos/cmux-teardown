# Windows Rust app plan (iroh + cloud VMs)

Goal: a Windows-first Rust RE workbench that steals knife's engine design and
REA's evidence/agent design, with cloud VMs for anything too heavy for the box.

## Stack

- **Core (Rust, Windows-first):** static analysis engine following knife's
  shape — one engine, three frontends (CLI, TUI/egui, MCP-stdio). Reuse
  `reknife` as a library or shell out to `knife` while you build; adopt its
  SHA-256-keyed analyst DB (names/notes/patches/types) from day one.
- **Agent layer:** MCP server (stdio locally, like knife) + REA-style evidence
  contract: canonical records with `evidence_id`, coverage
  (scope x complete/unknown), per-facet availability, unknowns ledger,
  `_meta.provenance.untrusted_content` marking on anything containing target
  bytes. Optionally spawn upstream `rea` for JS/.NET/APK/firmware targets you
  don't want to reimplement.
- **Networking (iroh):** `cargo add iroh` (1.x stable; 1.0 "dial keys, not
  IPs", ~1.1 current). Endpoints are Ed25519 keys (`EndpointId`); QUIC with
  UDP hole punching + encrypted relay fallback (`iroh-relay` is self-hostable).
  Use one ALPN per protocol (e.g. `re-app/analysis/0`, `re-app/evidence/1`):
  app dials cloud VMs by key, opens bidirectional QUIC streams, no IPs/ports
  to configure. Discovery: pkarr + relay rendezvous; run your own relay later
  for independence from n0's.
- **Cloud VMs (freestyle.sh API, not cmux Founders):** Founders credit is
  cmux-app-locked — no good as your backend. Use raw Freestyle:
  prototype on **Free** (200 vCPU-h + 400 GiB-h/mo, 4vCPU/8GiB-max VMs,
  no persistence), graduate to **Hobby ($50/mo)** for persistent
  snapshots + 8vCPU VMs. Bake a snapshot with your VM-side daemon +
  Ghidra/JADX/WABT/toolchain preinstalled (cmux's `cmux-freestyle` repo is
  the template for this pattern). Details:
  [cmux-founders-vs-freestyle.md](cmux-founders-vs-freestyle.md).

## Architecture

```
Windows app (Rust)
  |-- local engine: PE/ELF/Mach-O static analysis (knife-pattern), analyst DB
  |-- MCP server (stdio): local tools for agents (knife-pattern tool registry,
  |   read-only vs project-write vs target-exec annotations)
  |-- evidence store: REA-pattern records, coverage, unknowns ledger
  +-- iroh endpoint: dials VM daemons by EndpointId (QUIC, relay fallback)
                        |
cloud VM (Freestyle snapshot, per job or persistent)    |
  |-- vm daemon (Rust, iroh endpoint): receives targets/evidence refs        |
  |-- heavy engines: Ghidra headless, JADX, WABT, pwntools, tshark, binwalk  |
  +-- returns REA-pattern evidence bundles (paged over QUIC streams)         |
```

Rules of the road (stolen from both tools):

1. **Static locally, heavy remotely, nothing executed silently.** Local engine
   never runs the target (knife rule); any execution (process capture, Frida,
   browser obs) happens on a disposable cloud VM and is labeled as such in
   the evidence (REA rule).
2. **One engine, every frontend.** MCP tools call the same functions as the
   CLI/TUI. Agent findings must be reproducible by hand.
3. **Coverage honesty end-to-end.** Every result carries what was observed vs
   unknown, per facet; cloud results additionally name the engine + snapshot
   that produced them.
4. **Evidence by reference over iroh.** QUIC streams are cheap; multi-MB blobs
   are not. Return summaries + `evidence_id`s, page bodies on demand
   (REA's `inspect_analysis_view` continuation pattern), bundle-export for
   full sessions.
5. **Transport minimalism.** stdio locally (no local ports/tokens — knife's
   argument); iroh's authenticated QUIC remotely (no separate bearer-token
   layer — the EndpointId *is* the identity).

## Build order

1. **M0 — local core:** `cargo` workspace; PE parsing + triage + secs/sinks
   (wrap `knife` binary first, port inward); analyst DB; CLI with `--json`.
2. **M1 — MCP + evidence:** stdio MCP server, ~15 core tools
   (`open`, `session_status`, `triage`, `audit`, `xrefs`, `paths_to`,
   `disassemble`, `decompile`, `set_name/note`, ...); evidence records +
   unknowns ledger.
3. **M2 — iroh link:** endpoint in app + echo/analysis daemon; ALPN protocol;
   relay fallback verified (simulate NAT); evidence paging over streams.
4. **M3 — cloud snapshot:** Freestyle snapshot with daemon + Ghidra/JADX;
   `vm create --snapshot`, dial-by-key, run real heavy job (APK decompile,
   firmware unpack), return evidence bundle.
5. **M4 — TUI + parity:** egui/TUI workspace (func list, listing, xrefs);
   REA-provider parity for .NET/JS via spawned `rea` or native ports.
6. **M5 — hardening:** self-hosted `iroh-relay`, VM pooling/quotas against
   Freestyle limits, signed releases + `knife update`-style self-updater.

## Open questions

- Link `reknife` vs wrap `knife` binary vs clean-room engine? (Start: wrap.)
- Which heavy engines must be Windows-capable locally (note REA's
  experimental Windows-Ghidra doc) vs Linux-cloud-only?
- Freestyle Hobby cost ceiling per user once VMs stay persistent?
- Do you need relay self-hosting at M2, or is n0's relay fine for v1?
