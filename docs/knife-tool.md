# knife — tool details

**Repo:** <https://github.com/bl4ckr0ss3/knife> · **Crate:** `reknife`
(`cargo install reknife`) · **License:** MIT · **Platforms:** Linux / macOS /
Windows · ~376 stars, actively pushed (Oct 2026).

**One-liner:** terminal-first static reverse-engineering toolkit in Rust.
Parses, triages, disassembles, and audits **PE, ELF, Mach-O, and WASM** from
one small binary. **Static only — never runs the target**, so it's safe to
point at malware.

## Why it's interesting for the Windows Rust app

- **It's already the architecture you want:** one Rust engine with three
  frontends (CLI, TUI, MCP server). Nothing is reimplemented per frontend —
  the MCP tools call the same engine the CLI does, so agent findings are
  reproducible by hand.
- **PE-first wins:** seeds function discovery from the PE exception directory
  (example: `7z.dll` goes 87 -> 6,472 recovered functions), ranks dangerous
  calls by argument provenance, reads *real* mitigations (not just flags).
- **Analyst state is a SHA-256-keyed DB** (names, notes, bookmarks, patches,
  types, prototypes) shared across CLI/TUI/MCP; follows the binary, not the
  path; `--db PATH` relocates it, `KNIFE_NO_CACHE=1` forces fresh analysis.
- **MCP over stdio deliberately:** no port/token/listener; the pipe is the
  security boundary. (Relevant when you add iroh transport — see
  [windows-rust-app-plan.md](windows-rust-app-plan.md).)

## CLI command inventory

`knife FILE` = full triage (verdict, hashes, sections, mitigations,
capabilities, IOCs, artifacts, entry disasm). Bare `knife` / `knife <dir>`
opens the file explorer; `knife tui FILE` opens the workspace. `--json` on
every analysis command.

| Command | What you get |
| --- | --- |
| `sec` | Exploit mitigations + what each missing one buys an attacker |
| `sinks [--class C] [--all]` | Dangerous-API call sites grouped by bug class |
| `audit [--reachable]` | Sink call sites whose args look exploitable (backward data-flow walk; x86/x64) |
| `xrefs TARGET` / `xrefs --str TEXT` | What references a func/import/address/string |
| `paths TARGET [--from F]` | Call chains from entry points + exports down to a sink |
| `graph [--from FUNC] [--reachable] [--dot]` | Whole-program call graph (text/JSON/DOT) |
| `graph --func FUNC [--dot]` | One function's CFG |
| `funcs [--by-refs]` | Recovered functions via CFG analysis |
| `dis --func NAME` / `dis [--vaddr X \| --off Y] [--count N]` | Function / linear disassembly (x86/x64, AArch64) |
| `pseudo --func NAME` | Lifted pseudocode: statements + calls with args |
| `headers` / `segments` / `sections` | ELF records / program headers / sections with entropy bars |
| `imports` / `exports` | Libs+funcs (suspicious APIs flagged) / exported symbols |
| `caps` | Capabilities inferred from symbol surface |
| `strings --min N` | ASCII + UTF-16 strings |
| `iocs` | URLs, IPs, domains, emails, wallets, reg keys, paths (defanged) |
| `hashes` | MD5 / SHA-1 / SHA-256 + imphash |
| `drv [--reachable]` | Kernel-driver / BYOVD analysis: devices, IRP dispatch, IOCTLs, signing, loldrivers matches |
| `scan` | Crypto constants, packer markers, embedded formats |
| `map` | Whole-file entropy sparkline, packed regions flagged |
| `hex --off O --len L` | Hex dump |
| `ls` | Archive (.a/.lib) members |
| `name ADDR NAME` / `note ADDR TEXT` | Persist names/notes (naming an address declares a function there) |
| `field --type T OFFSET NAME [--data-type C]` | Reusable, optionally typed struct fields |
| `type --func F BASE TYPE` / `var --func F BASE NAME` | Bind pseudocode base to user type / rename recovered var |
| `proto --func F --returns T [--param T ...]` | Exact persistent function prototype |
| `patch [--vaddr \| --off] --bytes HEX` / `--clear` / `--export PATH` | Stage byte patches (never touches input); atomic patched-copy export |
| `typelib --export/--import PATH` | Share layouts across binaries |
| `db` | Everything stored for this binary |
| `mcp [--file F]` | MCP server over stdio |
| `diff A B` | Compare functions/imports/sections; exit 1 on change |
| `completions SHELL` | bash/zsh/fish/powershell/elvish |
| `update [--check]` | Self-update with SHA-256-verified GitHub assets |

**Vuln-research loop:** `sec` (what am I up against) -> `sinks --class memory`
(where's the surface) -> `audit --reachable` (what looks actually wrong) ->
`xrefs`/`paths` (what reaches it) -> `name`/`note` (write down understanding).
Worked example in repo: `docs/case-study-eqnedt32.md` (CVE-2017-11882).

## MCP tool inventory (`knife mcp`)

One target per server process; `open` binds it, then `file` is optional on
every tool. Analysis cached per target. Results capped (400 rows), bounded
instruction budget, `knife/resultId` content-addressed refs + `get_retained_result`
recovery, cursor pagination on `list_functions`/`strings`/`read_function_instructions`.

| Tool | Answers |
| --- | --- |
| `open` | Analyse a binary, make it the session target (returns `info` summary) |
| `session_status` | Content identity, coverage, per-target availability, limitations, versioned evidence |
| `provider_status` | Optional external engines, versions, safety mode, licenses |
| `discover_investigation_tools` | Tools grouped by task + operation policy + availability |
| `info` / `triage` / `hashes` / `entropy` | Format/arch/entry/sections · verdict+signals · hashes · entropy map |
| `hardening` / `signing` / `signatures` / `loldrivers` | Mitigations · Authenticode summary · const/packer/format hits · vulnerable-driver match |
| `list_functions` / `resolve_containing_function` | Recovered funcs + call counts · func containing an address |
| `inspect_address_context` / `address_to_file_offset` | Section/symbol/func/instr/xrefs at addr · VA->offset |
| `read_function_instructions` | Paginated structured instruction rows (bytes/targets/blocks/offsets) |
| `disassemble` / `decompile` | One func's disasm / pseudocode |
| `xrefs` / `callees` / `call_graph` / `paths_to` | Callers · callees · call closure · chains from entries/exports |
| `sinks` / `audit` / `trace_taint` | Raw sink sites · ranked sites · per-func arg provenance + chains |
| `trace_string_usage` / `strings` / `iocs` | Literals + ref sites + funcs · string query · IOC extraction |
| `inspect_input_validation` | Candidate comparisons/branches/validation APIs + dangerous-arg provenance |
| `imports` / `exports` / `capabilities` | Cross-boundary surface + inferred behaviour |
| `elf_structure` | ELF header/phdr/shdr records with exact file ranges |
| `driver_report` | Windows-driver surface: devices, IRP dispatch, decoded IOCTLs |
| `read_bytes` | Exact-length hex+ASCII dump at a static VA |
| `rizin_*` | Independent Rizin metadata/symbols/funcs/disasm/xrefs/CFG/call-graph |
| `capa_capabilities` / `capa_rule_matches` | capa rules + ATT&CK/MBC metadata + match locations/trees (PE/ELF) |
| `yarax_scan` / `yarax_scan_range` / `yarax_validate_rules` | Native YARA-X over workspace / VA range / rule validation |
| `compare_engine_findings` | Knife-vs-Rizin function boundaries (agreement/disagreement/unknown) |
| `explain_function` | Evidence-linked func context + bounded instrs + callers/callees + pseudocode |
| `investigation_*` (ask/add_plan_step/update_plan_step/append_message/record_evidence/link_evidence/conflicts/get_evidence/status) | Persisted investigation: questions, hypotheses, evidence links, plan steps, messages |
| `list_retained_results` / `get_retained_result` | Bounded retained-result ledger / exact payload by ref |
| `set_name` / `set_note` / `set_prototype` / `stage_patch` | Mutate saved DB (never the binary) |

**Limits:** lifting engines (`decompile`, `audit`, `trace_taint`,
`driver_report`) are x86/x64-only and refuse other archs; `disassemble` also
does AArch64. `session_status` reports per-format/arch availability with
reasons. Evidence envelope separates observations / inferences / unknowns.

## Reuse notes

- Treat knife as the **local static engine** in the app plan: shell out to the
  `knife` binary, link `reknife` crates, or speak MCP-stdio to `knife mcp`.
- Its operation-registry pattern (every tool annotated read-only vs
  project-write vs target-exec vs target-write) is worth copying for your own
  tool surface.
- Gaps knife leaves for REA/cloud to fill: managed runtimes (.NET), JS/Electron,
  APKs, firmware, runtime behavior, decompiler-backed pseudocode (Hopper/Ghidra/IDA).
