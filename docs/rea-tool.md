# REA — tool details

**Repo:** <https://github.com/morluto/rea> · **Site:** <https://rea.tools> ·
**Package:** `rea-agents` on npm (`npx rea-agents setup`, `rea --help`) ·
**License:** MIT · ~211k stars. Requires Node.js 22.19+ / 24.11+ / 26+.

**One-liner:** "Reverse Engineer Anything" — one MCP server that lets coding
agents inspect native binaries, apps, and runtime behavior, returning findings
**with evidence and stated limitations**. Analysis runs locally; the agent gets
tool results and iterates (ask follow-ups, explain, implement, test).

## Architecture

```
agent (Claude Code / Codex / Cursor / Gemini / Grok / ...) --MCP stdio--> REA server
  |--- native providers: Hopper | Ghidra | IDA (+ optional native MCP bridge)
  |--- offline engines: objdump, Rizin, WABT, pwntools, tshark, sigrok, binwalk...
  |--- app/static parsers: JS/Electron, .NET (static, no engine), APK (JADX), Mach-O...
  |--- runtime: browser observation, process capture, Frida, ADB, GDB, mitmproxy captures
  +--- session bundle: retained Evidence records, unknowns ledger, comparisons
```

- Setup (`npx rea-agents setup`) registers the MCP server + workflow
  instructions (skills) per agent, with backups; restart the agent after.
- `binary_session` is the authoritative source of catalog identity + tool
  availability on a running server (the `product-catalog.json` is generated
  at build, not checked in).
- Server implementations live in `src/server/register*Tools.ts` (one module
  per domain: native, application, browser, android, firmware, pcap, ...).

## What it can analyze (target matrix)

| Target | Returns | Requirements |
| --- | --- | --- |
| Native binaries | Pseudocode, asm, strings, symbols, calls, xrefs | Hopper, Ghidra, or IDA |
| Offline ELF layout | Sections/segments/symbols/relocs, mitigation candidates | pwntools on Linux x64 |
| Go binaries | Compiler version, modules, replacements, build settings | Built-in static ELF/PE/Mach-O inspection |
| Rust binaries | cargo-auditable crates/versions/sources/edges | Built-in static inspection |
| WASM | Validation, sections, import/export WAT, decoded WAT | WABT 1.0.42 |
| EVM bytecode | Selectors, offsets, inferred args, mutability | Local carrier on Linux x64 |
| SQLite DBs | Schema + selected rows | Node 24.15+/26+ native SQLite |
| Recorded Linux crashes | Notes, per-thread regs/signals, mapping candidates | pwntools; optional GDB/pwndbg |
| JS / Electron | Modules, imports, sourcemaps, routes, IPC, native add-ons | Node+npm (no native engine) |
| Websites | Structure, scripts, network obs, screenshots | Chrome-family browser |
| HAR/SAZ captures | Requests/responses/payloads/source locations | HAR or Fiddler SAZ; mitmdump on Linux |
| .NET assemblies | Metadata, CIL, native deps, build comparisons | None (static only) |
| Android APKs | Manifest, classes, decompiled methods, xrefs | Headless JADX + full JDK |
| Android devices | Packages, procs, logs, dumps, screenshots, transfers | adb |
| JEB projects | Units, digests, pseudo-code | Caller-started JEB client over MCP (`REA_JEB_MCP_URL`) |
| APK resources | Decoded manifest, strings, locales | Apktool |
| Flutter apps | Dart snapshot hash, per-ABI digests, engine provenance | None (static APK parsing) |
| Firmware | Regions, extraction, native-analysis handoffs | Binwalk/Unblob on Linux |
| pcap / packets / signals | Endpoints/DNS/SNI/HTTP; tshark fields; sigrok decodes | tshark 4.x / sigrok-cli (Linux) |
| Packages/resources | Inventories, digests, plists, Apple bundles | Built-in |
| Process behavior | Output, interactions, exit, fs obs, run comparisons | Linux/macOS native PTY |

Static JS + .NET inspection never run the target. Runtime capture runs with
your user permissions; each guide documents effects. REA does **not** install
objdump/Rizin/Ghidra/IDA — caller-supplied. Setup can optionally install
Hopper with approval. Experimental Windows Ghidra support exists
(`docs/windows-ghidra-p0.md`).

## MCP contract (the part worth stealing)

Full spec: `docs/mcp-contracts.md`. Key ideas:

- **Evidence records:** evidence-producing tools return the complete canonical
  record in both `text` and `structuredContent` (`normalized_result` +
  `evidence_id`; `raw_result` for the upstream report or null). Same record is
  retained in the session bundle. `analysis_profile` always present.
- **Composable comparisons:** pass `analyze_function` evidence straight into
  `compare_functions`, `inspect_artifact` into `compare_artifacts`. Scopes must
  match; missing members -> `truncated`; unknown coverage stays `unknown`
  (never silently "equal").
- **Coverage honesty:** `coverage.scope` (root/container-entries) x
  `coverage.status` (complete/unknown); per-facet availability map for function
  dossiers (pseudocode/asm/comments/callers/callees/references/strings/CFG).
  Empty-but-observed is comparable; unobserved is `unknown`.
- **Tool errors:** `isError: true` + canonical `{error}` diagnostic JSON
  (code, target, partial observations, remediation). No `structuredContent` on
  failure.
- **Transport budget:** results prepared within a 10 MiB stdio receive budget
  (text + structured + envelope); oversize -> `resource_constraint` with an
  `evidence_reference` + ready-to-call `inspect_analysis_view` continuation
  (paged), or `export_evidence_bundle` (atomic file export). Tunable via
  `REA_MCP_MAX_RESPONSE_BYTES` (>= 10 MiB) if the client buffer grows too.
- **Untrusted-target marking:** every tool whose results/errors can contain
  target bytes carries `_meta.provenance = {untrusted_content: true,
  source: "analysis_target"}`; only pure-session tools
  (`export_evidence_bundle`, `record_unknown`) omit it. Server instructions
  tell agents to treat results as data (prompt-injection defense).
- **Unknowns ledger:** `record_unknown` / `update_unknown` / `list_unknowns`
  (paged) track open questions; comparisons and truncations feed it.

## Notable MCP tools (seen in contracts/CLI; authoritative list via `binary_session`)

Session/evidence: `binary_session`, `close_binary`, `get_evidence_bundle`
(+ `detail: summary` discovery with filters), `inspect_analysis_view`,
`export_evidence_bundle`, `record_unknown`, `update_unknown`, `list_unknowns`.
Native: `analyze_function`, `compare_functions`, `get_call_graph`,
`trace_call_path`, `inspect_binary_layout`, `inspect_macho`,
`inspect_artifact`, `compare_artifacts`, `inspect_wasm_artifact`,
`decode_interface_builder`. App/web: `analyze_javascript_application`,
browser-observation + scenario tools, capture tools, JS-runtime observation.
Plus per-domain modules: ADB, Android/JADX, Apktool, managed (.NET), EVM,
firmware, Frida, GDB, Go, Rust, pcap, signals, SQLite, JEB, Cutter, Rizin-debug,
Electron, process capture/comparison.

CLI mirrors the same workflows, e.g.
`rea-agents analyze-javascript-application /path/to/app --json`, plus native
commands, provider selection (`--provider hopper|ghidra|ida`), snapshots, and
`rea inspect-evidence-bundle ...` for portable bundles.

## Reuse notes

- Treat REA as the **agent-orchestration + evidence layer** in the app plan:
  either embed its flows (spawn `rea` beside your Rust core) or reimplement
  its **evidence-record + coverage + unknowns** contract in Rust and serve it
  over your own MCP endpoint.
- Its provider model (Hopper/Ghidra/IDA behind one tool surface + offline
  fallbacks) maps directly onto "local knife engine + cloud heavy engines".
- Watch: Node.js runtime dependency, Linux-x64 bias for offline engines, and
  per-provider fidelity differences (Ghidra/Hopper declare observed facets;
  IDA declares unsupported ones) — your Windows app needs the same honesty
  about what each backend actually observed.
