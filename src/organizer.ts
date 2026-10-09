import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

import type {
  DecryptFileResult,
  DetectionReport,
  EncryptReport,
  ExecutionReport,
  OrganizePlan,
  OperationRequest,
  ProgressEvent,
  ProposedOperation,
} from "./types";

function element<T extends HTMLElement>(id: string): T {
  const node = document.getElementById(id);
  if (!node) {
    throw new Error(`Missing element #${id}`);
  }
  return node as unknown as T;
}

export function initOrganizer(): void {
  const aiStatus = element<HTMLParagraphElement>("ai-status");
  const pickButton = element<HTMLButtonElement>("pick-folder");
  const folderPath = element<HTMLSpanElement>("folder-path");
  const instruction = element<HTMLTextAreaElement>("instruction");
  const planButton = element<HTMLButtonElement>("plan");
  const progress = element<HTMLSpanElement>("progress");
  const planPanel = element<HTMLElement>("plan-panel");
  const planSummary = element<HTMLParagraphElement>("plan-summary");
  const planOps = element<HTMLUListElement>("plan-ops");
  const planWarnings = element<HTMLParagraphElement>("plan-warnings");
  const approveButton = element<HTMLButtonElement>("approve");
  const discardButton = element<HTMLButtonElement>("discard");
  const reportPanel = element<HTMLElement>("report-panel");
  const reportList = element<HTMLUListElement>("report");
  const scanButton = element<HTMLButtonElement>("scan");
  const resetButton = element<HTMLButtonElement>("reset");
  const confidentialPanel = element<HTMLElement>("confidential-panel");
  const confidentialStatus = element<HTMLParagraphElement>("confidential-status");
  const confidentialList = element<HTMLUListElement>("confidential-list");
  const passphraseRow = element<HTMLElement>("passphrase-row");
  const passphraseInput = element<HTMLInputElement>("passphrase");
  const passphraseConfirm = element<HTMLInputElement>("passphrase-confirm");
  const encryptButton = element<HTMLButtonElement>("encrypt-selected");
  const encryptResults = element<HTMLUListElement>("encrypt-results");

  let root: string | null = null;
  let plan: OrganizePlan | null = null;
  let detection: DetectionReport | null = null;
  let sessionPassphrase: string | null = null;
  let busy = false;
  let scanning = false;

  function setProgress(message: string, isError = false): void {
    progress.textContent = message;
    progress.classList.toggle("error", isError);
  }

  function setConfidentialStatus(message: string, isError = false): void {
    confidentialStatus.textContent = message;
    confidentialStatus.classList.toggle("error", isError);
  }

  function checkedPaths(): string[] {
    return Array.from(
      confidentialList.querySelectorAll<HTMLInputElement>("input[type=checkbox]:checked"),
    )
      .map((checkbox) => checkbox.dataset.path ?? "")
      .filter((path) => path.length > 0);
  }

  function passphraseReady(): boolean {
    return (
      passphraseInput.value.length > 0 && passphraseInput.value === passphraseConfirm.value
    );
  }

  function refreshButtons(): void {
    const hasInstruction = instruction.value.trim().length > 0;
    const hasValidOperations = plan?.operations.some((operation) => operation.valid) ?? false;
    pickButton.disabled = busy;
    instruction.disabled = busy;
    planButton.disabled = busy || !root || !hasInstruction;
    discardButton.disabled = busy;
    resetButton.disabled = busy;
    approveButton.disabled = busy || !hasValidOperations;
    scanButton.disabled = busy || scanning || !root;
    encryptButton.disabled =
      busy || scanning || !detection || checkedPaths().length === 0 || !passphraseReady();
  }

  function setBusy(value: boolean): void {
    busy = value;
    refreshButtons();
  }

  function show(panel: HTMLElement): void {
    panel.classList.remove("hidden");
  }

  function hide(panel: HTMLElement): void {
    panel.classList.add("hidden");
  }

  function clearConfidential(): void {
    detection = null;
    sessionPassphrase = null;
    confidentialList.replaceChildren();
    encryptResults.replaceChildren();
    passphraseInput.value = "";
    passphraseConfirm.value = "";
    hide(confidentialPanel);
  }

  async function chooseFolder(): Promise<void> {
    const selected = await open({ directory: true, multiple: false, title: "Choose the folder to organize" });
    if (typeof selected !== "string") {
      return;
    }
    root = selected;
    folderPath.textContent = selected;
    folderPath.classList.remove("muted");
    clearConfidential();
    setProgress("");
    refreshButtons();
  }

  function operationRow(operation: ProposedOperation): HTMLLIElement {
    const row = document.createElement("li");
    row.className = operation.valid ? "op" : "op invalid";
    const label = document.createElement("span");
    label.textContent = operation.description;
    const badge = document.createElement("span");
    badge.className = "badge";
    badge.textContent = operation.valid ? "ready" : "skipped";
    row.append(label, badge);
    if (!operation.valid && operation.reason) {
      const reason = document.createElement("span");
      reason.className = "reason";
      reason.textContent = operation.reason;
      row.append(reason);
    }
    return row;
  }

  function renderPlan(current: OrganizePlan): void {
    planSummary.textContent = current.summary;
    planOps.replaceChildren(...current.operations.map(operationRow));
    planWarnings.textContent = current.warnings.join(" ");
    show(planPanel);
  }

  async function makePlan(): Promise<void> {
    if (!root) {
      return;
    }
    setBusy(true);
    setProgress("Planning with the local model…");
    hide(planPanel);
    hide(reportPanel);
    clearConfidential();
    try {
      plan = await invoke<OrganizePlan>("plan_organize", { root, instruction: instruction.value.trim() });
      renderPlan(plan);
      const validCount = plan.operations.filter((operation) => operation.valid).length;
      setProgress(`Plan ready — ${validCount} of ${plan.operations.length} operation(s) can run.`);
    } catch (error) {
      plan = null;
      setProgress(String(error), true);
    } finally {
      setBusy(false);
    }
  }

  function renderReport(report: ExecutionReport): void {
    reportList.replaceChildren(
      ...report.results.map((result) => {
        const row = document.createElement("li");
        row.className = result.ok ? "op" : "op invalid";
        const label = document.createElement("span");
        label.textContent = result.description;
        const badge = document.createElement("span");
        badge.className = "badge";
        badge.textContent = result.ok ? "done" : "failed";
        const message = document.createElement("span");
        message.className = "reason";
        message.textContent = result.message;
        row.append(label, badge, message);
        return row;
      }),
    );
  }

  async function approvePlan(): Promise<void> {
    if (!root || !plan) {
      return;
    }
    const operations: OperationRequest[] = plan.operations
      .filter((operation) => operation.valid)
      .map((operation) => ({ tool: operation.tool, arguments: operation.arguments }));
    if (operations.length === 0) {
      return;
    }
    setBusy(true);
    setProgress("Executing the approved operations…");
    try {
      const report = await invoke<ExecutionReport>("execute_plan", { root, operations });
      renderReport(report);
      hide(planPanel);
      show(reportPanel);
      setProgress(`${report.okCount} succeeded, ${report.failedCount} failed.`, report.failedCount > 0);
      void scanConfidential();
    } catch (error) {
      setProgress(String(error), true);
    } finally {
      setBusy(false);
    }
  }

  async function scanConfidential(): Promise<void> {
    if (!root || scanning) {
      return;
    }
    scanning = true;
    show(confidentialPanel);
    setConfidentialStatus("Scanning for confidential files…");
    confidentialList.replaceChildren();
    encryptResults.replaceChildren();
    refreshButtons();
    try {
      detection = await invoke<DetectionReport>("detect_confidential", { root });
      renderFindings(detection);
    } catch (error) {
      detection = null;
      setConfidentialStatus(`Scan failed — ${String(error)}`, true);
    } finally {
      scanning = false;
      refreshButtons();
    }
  }

  function renderFindings(report: DetectionReport): void {
    const warnings = report.warnings.length > 0 ? ` ${report.warnings.join(" ")}` : "";
    if (report.findings.length === 0) {
      passphraseRow.classList.add("hidden");
      setConfidentialStatus(
        `No confidential files detected (${report.scanned} file(s) scanned).${warnings}`,
      );
      return;
    }
    const rows = report.findings.map((finding) => {
      const row = document.createElement("li");
      row.className = "op finding";
      const label = document.createElement("label");
      label.className = "finding-label";
      const checkbox = document.createElement("input");
      checkbox.type = "checkbox";
      checkbox.checked = true;
      checkbox.dataset.path = finding.path;
      checkbox.addEventListener("change", refreshButtons);
      const pathSpan = document.createElement("span");
      pathSpan.textContent = finding.path;
      label.append(checkbox, pathSpan);
      const reasons = document.createElement("div");
      reasons.className = "reasons";
      for (const reason of finding.reasons) {
        const badge = document.createElement("span");
        badge.className = "badge reason-badge";
        badge.textContent = reason;
        reasons.append(badge);
      }
      row.append(label, reasons);
      return row;
    });
    confidentialList.replaceChildren(...rows);
    passphraseRow.classList.remove("hidden");
    setConfidentialStatus(
      `${report.findings.length} of ${report.scanned} file(s) look confidential — review the reasons, then encrypt what you choose.${warnings}`,
    );
  }

  async function encryptSelected(): Promise<void> {
    if (!root || !detection || busy) {
      return;
    }
    const paths = checkedPaths();
    if (paths.length === 0) {
      return;
    }
    if (!passphraseReady()) {
      setConfidentialStatus("Enter the same passphrase in both fields first.", true);
      return;
    }
    setBusy(true);
    setConfidentialStatus(`Encrypting ${paths.length} file(s)…`);
    try {
      const report = await invoke<EncryptReport>("encrypt_files", {
        root,
        paths,
        passphrase: passphraseInput.value,
      });
      sessionPassphrase = passphraseInput.value;
      passphraseInput.value = "";
      passphraseConfirm.value = "";
      renderEncryptResults(report);
      setConfidentialStatus(
        `${report.okCount} file(s) encrypted, ${report.failedCount} failed. Use Decrypt to prove a file is recoverable.`,
        report.failedCount > 0,
      );
    } catch (error) {
      setConfidentialStatus(`Encryption failed — ${String(error)}`, true);
    } finally {
      setBusy(false);
    }
  }

  function renderEncryptResults(report: EncryptReport): void {
    const rows = report.results.map((result) => {
      const row = document.createElement("li");
      row.className = result.ok ? "op" : "op invalid";
      const label = document.createElement("span");
      label.textContent = result.ok ? `${result.path} → ${result.output ?? ""}` : result.path;
      const badge = document.createElement("span");
      badge.className = "badge";
      badge.textContent = result.ok ? "encrypted" : "failed";
      const message = document.createElement("span");
      message.className = "reason";
      message.textContent = result.message;
      row.append(label, badge, message);
      const output = result.output;
      if (result.ok && output) {
        const decryptButton = document.createElement("button");
        decryptButton.type = "button";
        decryptButton.className = "button small";
        decryptButton.textContent = "Decrypt";
        decryptButton.addEventListener("click", () => void decryptFile(output, row));
        row.append(decryptButton);
      }
      return row;
    });
    encryptResults.replaceChildren(...rows);
  }

  async function decryptFile(encPath: string, row: HTMLLIElement): Promise<void> {
    if (!root) {
      return;
    }
    let passphrase = sessionPassphrase;
    if (!passphrase && passphraseReady()) {
      passphrase = passphraseInput.value;
    }
    const message = row.querySelector<HTMLSpanElement>(".reason");
    if (!passphrase) {
      setConfidentialStatus("Enter the passphrase above, then decrypt again.", true);
      passphraseInput.focus();
      return;
    }
    setBusy(true);
    try {
      const result = await invoke<DecryptFileResult>("decrypt_file", {
        root,
        path: encPath,
        passphrase,
      });
      if (message) {
        message.textContent = result.message;
      }
      setConfidentialStatus(`Recovered '${result.output}' from '${result.path}'.`);
    } catch (error) {
      if (message) {
        message.textContent = `Decrypt failed — ${String(error)}`;
      }
      setConfidentialStatus(`Decrypt failed — ${String(error)}`, true);
    } finally {
      setBusy(false);
    }
  }

  function discardPlan(): void {
    plan = null;
    hide(planPanel);
    setProgress("Plan discarded — nothing was changed.");
    refreshButtons();
  }

  function reset(): void {
    root = null;
    plan = null;
    instruction.value = "";
    clearConfidential();
    folderPath.textContent = "No folder selected";
    folderPath.classList.add("muted");
    hide(planPanel);
    hide(reportPanel);
    setProgress("");
    refreshButtons();
  }

  instruction.addEventListener("input", refreshButtons);
  passphraseInput.addEventListener("input", refreshButtons);
  passphraseConfirm.addEventListener("input", refreshButtons);
  pickButton.addEventListener("click", () => void chooseFolder());
  planButton.addEventListener("click", () => void makePlan());
  approveButton.addEventListener("click", () => void approvePlan());
  discardButton.addEventListener("click", discardPlan);
  scanButton.addEventListener("click", () => void scanConfidential());
  encryptButton.addEventListener("click", () => void encryptSelected());
  resetButton.addEventListener("click", reset);

  void listen<ProgressEvent>("organize-progress", (event) => setProgress(event.payload.message));
  void listen<ProgressEvent>("confidential-progress", (event) =>
    setConfidentialStatus(event.payload.message),
  );

  void invoke<string>("check_ollama")
    .then((message) => {
      aiStatus.textContent = message;
      aiStatus.classList.add("ok");
    })
    .catch((error: unknown) => {
      aiStatus.textContent = `Local AI unavailable — ${String(error)}`;
      aiStatus.classList.add("error");
    });

  refreshButtons();
}
