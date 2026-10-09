import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

import type {
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
  const resetButton = element<HTMLButtonElement>("reset");

  let root: string | null = null;
  let plan: OrganizePlan | null = null;
  let busy = false;

  function setProgress(message: string, isError = false): void {
    progress.textContent = message;
    progress.classList.toggle("error", isError);
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

  async function chooseFolder(): Promise<void> {
    const selected = await open({ directory: true, multiple: false, title: "Choose the folder to organize" });
    if (typeof selected !== "string") {
      return;
    }
    root = selected;
    folderPath.textContent = selected;
    folderPath.classList.remove("muted");
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
    } catch (error) {
      setProgress(String(error), true);
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
    folderPath.textContent = "No folder selected";
    folderPath.classList.add("muted");
    hide(planPanel);
    hide(reportPanel);
    setProgress("");
    refreshButtons();
  }

  instruction.addEventListener("input", refreshButtons);
  pickButton.addEventListener("click", () => void chooseFolder());
  planButton.addEventListener("click", () => void makePlan());
  approveButton.addEventListener("click", () => void approvePlan());
  discardButton.addEventListener("click", discardPlan);
  resetButton.addEventListener("click", reset);

  void listen<ProgressEvent>("organize-progress", (event) => setProgress(event.payload.message));

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
