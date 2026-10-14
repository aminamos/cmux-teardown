import "./styles.css";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

interface Target {
  path: string;
  sha256: string;
  size: number;
}

interface Evidence {
  evidence_id: string;
  tool: string;
  target_sha256: string;
  producer: string;
  coverage: { scope: string; status: string; facets?: Record<string, unknown> };
  result: unknown;
  limitations: string[];
}

interface NetStatus {
  nodeId: string;
  shortId: string;
  addr: string;
}

const $ = <T extends HTMLElement>(id: string): T => {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing element #${id}`);
  return el as T;
};

let target: Target | null = null;
let knifeBin: string | null = null;

function setStatus(msg: string, isError = false): void {
  const line = $("status-line");
  line.textContent = msg;
  line.classList.toggle("error", isError);
}

function setPill(id: string, text: string, state: "ok" | "bad" | ""): void {
  const pill = $(id);
  pill.textContent = text;
  pill.classList.remove("ok", "bad");
  if (state) pill.classList.add(state);
}

function showEvidence(ev: Evidence): void {
  $("view-evidence").textContent = JSON.stringify(ev, null, 2);
  switchTab("evidence");
  setStatus(
    `${ev.tool} evidence=${ev.evidence_id.slice(0, 12)} coverage=${ev.coverage.status}`
  );
}

function switchTab(name: "evidence" | "store"): void {
  document.querySelectorAll(".tab").forEach((t) =>
    t.classList.toggle("active", (t as HTMLElement).dataset.tab === name)
  );
  $("view-evidence").classList.toggle("hidden", name !== "evidence");
  $("view-store").classList.toggle("hidden", name !== "store");
}

async function refreshStore(): Promise<void> {
  if (!target) return;
  try {
    const ann = await invoke("get_annotations", { path: target.path });
    $("view-store").textContent = JSON.stringify(ann, null, 2);
  } catch (e) {
    $("view-store").textContent = `store unavailable: ${e}`;
  }
}

async function selectTarget(path: string): Promise<void> {
  const t = await invoke<Target>("open_target", { path });
  target = t;
  $("target-path").textContent = t.path;
  $("target-sha").textContent = t.sha256;
  $("target-size").textContent = `${t.size.toLocaleString()} bytes`;
  await refreshStore();
  setStatus(`open ${t.sha256.slice(0, 12)} (${t.size.toLocaleString()} bytes)`);
}

async function runTool(tool: string): Promise<void> {
  if (!target) {
    setStatus("open a target first", true);
    return;
  }
  setStatus(`running ${tool}…`);
  try {
    const ev = await invoke<Evidence>("analyze", {
      path: target.path,
      tool,
      knifeBin,
    });
    showEvidence(ev);
  } catch (e) {
    setStatus(`${tool} failed: ${e}`, true);
  }
}

async function boot(): Promise<void> {
  document.querySelectorAll(".tab").forEach((t) =>
    t.addEventListener("click", () =>
      switchTab((t as HTMLElement).dataset.tab as "evidence" | "store")
    )
  );

  $("pick-file").addEventListener("click", async () => {
    try {
      const picked = await open({ multiple: false, directory: false });
      if (typeof picked === "string") await selectTarget(picked);
    } catch (e) {
      setStatus(`file picker failed: ${e}`, true);
    }
  });

  document.querySelectorAll(".tool").forEach((b) =>
    b.addEventListener("click", () =>
      runTool((b as HTMLElement).dataset.tool ?? "triage")
    )
  );

  $("ann-save-name").addEventListener("click", async () => {
    if (!target) return setStatus("open a target first", true);
    const address = ($("ann-addr") as HTMLInputElement).value;
    const name = ($("ann-name") as HTMLInputElement).value;
    try {
      await invoke("set_name", { path: target.path, address, name });
      await refreshStore();
      setStatus(`named ${address} -> ${name}`);
    } catch (e) {
      setStatus(`set_name failed: ${e}`, true);
    }
  });

  $("ann-save-note").addEventListener("click", async () => {
    if (!target) return setStatus("open a target first", true);
    const address = ($("ann-addr") as HTMLInputElement).value;
    const text = ($("ann-note") as HTMLInputElement).value;
    try {
      await invoke("add_note", { path: target.path, address, text });
      ($("ann-note") as HTMLInputElement).value = "";
      await refreshStore();
      setStatus(`noted ${address}`);
    } catch (e) {
      setStatus(`add_note failed: ${e}`, true);
    }
  });

  $("net-echo").addEventListener("click", async () => {
    $("net-echo-out").textContent = "…";
    try {
      const reply = await invoke<string>("net_echo_self", {
        message: "hello-rework",
      });
      $("net-echo-out").textContent = `reply: ${reply}`;
      setStatus("echo roundtrip ok");
    } catch (e) {
      $("net-echo-out").textContent = "failed";
      setStatus(`echo failed: ${e}`, true);
    }
  });

  // Backend status probes (non-fatal).
  try {
    const knife = await invoke<string>("knife_status", { knifeBin });
    setPill("knife-status", `knife: ${knife}`, "ok");
  } catch (e) {
    setPill("knife-status", "knife: missing", "bad");
    setStatus(`knife unavailable: ${e}`, true);
  }
  try {
    const net = await invoke<NetStatus>("net_status");
    setPill("net-status", `net: ${net.shortId}`, "ok");
    $("net-id").textContent = `node id: ${net.nodeId}`;
  } catch (e) {
    setPill("net-status", "net: down", "bad");
    setStatus(`net unavailable: ${e}`, true);
  }
}

boot().catch((e) => setStatus(`boot failed: ${e}`, true));
