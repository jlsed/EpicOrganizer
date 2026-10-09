import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

import { refreshHistory } from "./history";
import type {
  DecryptFileResult,
  DetectionReport,
  EncryptReport,
  ExecutionReport,
  FolderEntry,
  OrganizePlan,
  OperationRequest,
  ProgressEvent,
} from "./types";

function element<T extends HTMLElement>(id: string): T {
  const node = document.getElementById(id);
  if (!node) {
    throw new Error(`Missing element #${id}`);
  }
  return node as unknown as T;
}

export interface OrganizerApi {
  selectRoot: (root: string, instruction?: string) => void;
}

export function initOrganizer(): OrganizerApi {
  // Top & Header
  const aiStatus = element<HTMLDivElement>("ai-status");
  const folderName = element<HTMLDivElement>("folder-name");
  const folderPath = element<HTMLDivElement>("folder-path");
  const pickButton = element<HTMLButtonElement>("pick-folder");

  // Sidebar Controls
  const instruction = element<HTMLTextAreaElement>("instruction");
  const planButton = element<HTMLButtonElement>("plan");
  const progress = element<HTMLDivElement>("progress");

  // Right Workspace Containers
  const welcomePane = element<HTMLDivElement>("welcome-pane");
  const dualPanes = element<HTMLDivElement>("dual-panes");
  const sourcePaneTitle = element<HTMLSpanElement>("source-pane-title");
  const sourcePaneCount = element<HTMLSpanElement>("source-pane-count");
  const sourceFileList = element<HTMLUListElement>("source-file-list");

  const targetPaneBadge = element<HTMLSpanElement>("target-pane-badge");
  const planEmptyNote = element<HTMLDivElement>("plan-empty-note");
  const planTreeContent = element<HTMLDivElement>("plan-tree-content");

  // Footer Actions
  const commanderFooter = element<HTMLDivElement>("commander-footer");
  const footerSummary = element<HTMLDivElement>("footer-summary");
  const approveButton = element<HTMLButtonElement>("approve");
  const discardButton = element<HTMLButtonElement>("discard");

  // Results & Confidential Panels
  const reportPanel = element<HTMLDivElement>("report-panel");
  const reportList = element<HTMLUListElement>("report");
  const scanButton = element<HTMLButtonElement>("scan");
  const resetButton = element<HTMLButtonElement>("reset");

  const confidentialPanel = element<HTMLDivElement>("confidential-panel");
  const confidentialStatus = element<HTMLParagraphElement>("confidential-status");
  const confidentialList = element<HTMLUListElement>("confidential-list");
  const passphraseRow = element<HTMLDivElement>("passphrase-row");
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
  let isPlanning = false;
  let planRequestId = 0;

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
    planButton.disabled = isPlanning ? false : (busy || !root || !hasInstruction);
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

  function clearConfidential(): void {
    detection = null;
    sessionPassphrase = null;
    confidentialList.replaceChildren();
    encryptResults.replaceChildren();
    passphraseInput.value = "";
    passphraseConfirm.value = "";
    confidentialPanel.classList.add("hidden");
  }

  function getFileIcon(name: string, isDir: boolean): string {
    if (isDir) return "📁";
    const lower = name.toLowerCase();

    // PDFs
    if (lower.endsWith(".pdf")) return "📕";
    // Word / Text Documents
    if (lower.match(/\.(docx?|odt|rtf|txt|md|pages)$/)) return "📄";
    // Excel / Spreadsheets
    if (lower.match(/\.(xlsx?|csv|tsv|ods|numbers)$/)) return "📊";
    // Presentations
    if (lower.match(/\.(pptx?|key|odp)$/)) return "📑";
    // Images
    if (lower.match(/\.(jpg|jpeg|png|gif|webp|svg|bmp|ico|tiff?|heic)$/)) return "🖼️";
    // Videos
    if (lower.match(/\.(mp4|mkv|mov|avi|webm|flv|wmv|m4v)$/)) return "🎬";
    // Audio / Music
    if (lower.match(/\.(mp3|wav|flac|aac|ogg|m4a|wma)$/)) return "🎵";
    // Archives / Compressed
    if (lower.match(/\.(zip|tar|gz|7z|rar|bz2|xz|iso)$/)) return "📦";
    // Code & Config
    if (lower.match(/\.(js|ts|tsx|jsx|rs|py|go|cpp|c|java|html|css|json|yaml|yml|toml|xml|sql|sh|ps1)$/)) return "📜";
    // Encrypted / Keys / Credentials
    if (lower.endsWith(".enc") || lower.match(/\.(key|pem|pub|crt|p12|kdbx)$/)) return "🔒";
    // Executables / Installers
    if (lower.match(/\.(exe|msi|dmg|app|deb|rpm)$/)) return "⚙️";

    return "📄";
  }

  function renderSourceEntries(entries: FolderEntry[]): void {
    sourceFileList.replaceChildren();
    if (entries.length === 0) {
      sourcePaneCount.textContent = "0 items";
      const emptyLi = document.createElement("li");
      emptyLi.className = "file-row";
      emptyLi.textContent = "(Empty folder)";
      sourceFileList.append(emptyLi);
      return;
    }

    sourcePaneCount.textContent = `${entries.length} item(s)`;
    for (const item of entries) {
      const li = document.createElement("li");
      li.className = "file-row";
      
      const icon = getFileIcon(item.name, item.isDir);

      const nameSpan = document.createElement("span");
      nameSpan.textContent = `${icon} ${item.name}`;

      const sizeSpan = document.createElement("span");
      sizeSpan.style.color = "var(--text-tertiary)";
      sizeSpan.style.fontSize = "11px";
      if (item.isDir) {
        sizeSpan.textContent = "folder";
      } else if (item.size < 1024) {
        sizeSpan.textContent = `${item.size} B`;
      } else if (item.size < 1024 * 1024) {
        sizeSpan.textContent = `${(item.size / 1024).toFixed(1)} KB`;
      } else {
        sizeSpan.textContent = `${(item.size / (1024 * 1024)).toFixed(1)} MB`;
      }

      li.append(nameSpan, sizeSpan);
      sourceFileList.append(li);
    }
  }

  async function loadFolder(selected: string): Promise<void> {
    root = selected;
    
    // Extract base folder name
    const parts = selected.split(/[/\\]/).filter(Boolean);
    const leaf = parts[parts.length - 1] || selected;
    folderName.textContent = leaf;
    folderPath.textContent = selected;
    folderPath.classList.remove("muted");

    sourcePaneTitle.textContent = `Current Folder: ${leaf}/`;
    
    clearConfidential();
    reportPanel.classList.add("hidden");
    welcomePane.classList.add("hidden");
    dualPanes.classList.remove("hidden");

    // Load actual folder contents immediately
    try {
      const entries = await invoke<FolderEntry[]>("list_folder_contents", { root });
      renderSourceEntries(entries);
      setProgress("");
    } catch (e) {
      root = null; // Unset invalid / protected root
      sourcePaneCount.textContent = "Access denied";
      const errLi = document.createElement("li");
      errLi.className = "file-row";
      errLi.style.color = "var(--badge-skip-text)";
      errLi.style.backgroundColor = "var(--badge-skip-bg)";
      errLi.textContent = `⚠️ ${String(e)}`;
      sourceFileList.replaceChildren(errLi);
      setProgress(String(e), true);
    }

    // Clear previous plan preview
    plan = null;
    planEmptyNote.classList.remove("hidden");
    planTreeContent.classList.add("hidden");
    planTreeContent.replaceChildren();
    targetPaneBadge.textContent = "Waiting for plan";
    commanderFooter.classList.add("hidden");

    refreshButtons();
  }

  async function chooseFolder(): Promise<void> {
    const selected = await open({ directory: true, multiple: false, title: "Choose the folder to organize" });
    if (typeof selected !== "string") {
      return;
    }
    await loadFolder(selected);
  }

  function renderTargetTree(current: OrganizePlan): void {
    planEmptyNote.classList.add("hidden");
    planTreeContent.classList.remove("hidden");
    planTreeContent.replaceChildren();

    // Group proposed operations into tree structure
    const folders = new Map<string, string[]>();
    const rootFiles: string[] = [];

    for (const op of current.operations) {
      if (op.tool === "move_file" && op.arguments && typeof op.arguments === "object") {
        const dst = String((op.arguments as Record<string, unknown>).destination || "");
        const parts = dst.split(/[/\\]/);
        if (parts.length > 1) {
          const folder = parts.slice(0, -1).join("/") + "/";
          const file = parts[parts.length - 1];
          if (!folders.has(folder)) {
            folders.set(folder, []);
          }
          folders.get(folder)!.push(file);
        } else {
          rootFiles.push(dst);
        }
      } else if (op.tool === "create_folder" && op.arguments && typeof op.arguments === "object") {
        const folder = String((op.arguments as Record<string, unknown>).path || "");
        const formatted = folder.endsWith("/") ? folder : folder + "/";
        if (!folders.has(formatted)) {
          folders.set(formatted, []);
        }
      }
    }

    if (folders.size === 0 && rootFiles.length === 0) {
      targetPaneBadge.textContent = "No changes needed";
      const emptyNote = document.createElement("div");
      emptyNote.className = "pane-placeholder";
      emptyNote.textContent = current.summary || "No operations proposed for this folder.";
      planTreeContent.append(emptyNote);
      commanderFooter.classList.add("hidden");
      return;
    }

    folders.forEach((files, folderName) => {
      const folderNode = document.createElement("div");
      folderNode.style.marginBottom = "8px";

      const folderHeader = document.createElement("div");
      folderHeader.className = "file-tree-folder";
      folderHeader.textContent = `📁 ${folderName}`;
      folderNode.append(folderHeader);

      for (const f of files) {
        const leaf = document.createElement("div");
        leaf.className = "file-tree-item";
        const icon = getFileIcon(f, false);
        leaf.textContent = `↳ ${icon} ${f}`;
        folderNode.append(leaf);
      }
      planTreeContent.append(folderNode);
    });

    for (const f of rootFiles) {
      const leaf = document.createElement("div");
      leaf.className = "file-tree-item";
      const icon = getFileIcon(f, false);
      leaf.textContent = `${icon} ${f}`;
      planTreeContent.append(leaf);
    }

    const validOps = current.operations.filter((op) => op.valid);
    targetPaneBadge.textContent = `+${folders.size} folder(s), ${validOps.length} action(s)`;
    
    footerSummary.innerHTML = `Plan ready: <strong>${validOps.length} operations</strong> to organize.`;
    commanderFooter.classList.remove("hidden");
  }

  async function cancelPlan(): Promise<void> {
    if (!isPlanning) {
      return;
    }
    isPlanning = false;
    planRequestId++;
    try {
      await invoke("cancel_plan");
    } catch {
      // Best effort cancel
    }
    plan = null;
    planTreeContent.innerHTML = '<div class="pane-placeholder">Plan creation cancelled. Click "Generate Plan" to try again.</div>';
    targetPaneBadge.textContent = "Cancelled";
    setProgress("Planning cancelled by user.");
    planButton.classList.remove("btn-cancel");
    planButton.innerHTML = '<span>Generate Plan</span><span>→</span>';
    setBusy(false);
  }

  async function makePlan(): Promise<void> {
    if (isPlanning) {
      void cancelPlan();
      return;
    }
    if (!root) {
      return;
    }
    isPlanning = true;
    const currentReqId = ++planRequestId;
    setBusy(true);
    planButton.classList.add("btn-cancel");
    planButton.innerHTML = '<span class="spinner"></span><span>✕ Cancel Planning</span>';
    targetPaneBadge.textContent = "Analyzing…";
    planEmptyNote.classList.add("hidden");
    planTreeContent.classList.remove("hidden");
    planTreeContent.innerHTML = `
      <div class="planning-loader">
        <div class="loader-spinner"></div>
        <div class="loader-title">Analyzing folder & drafting plan…</div>
        <div class="loader-sub">Reading files and determining target structure</div>
      </div>
    `;
    setProgress("Analyzing folder and drafting tree…");
    clearConfidential();
    reportPanel.classList.add("hidden");

    try {
      const res = await invoke<OrganizePlan>("plan_organize", { root, instruction: instruction.value.trim() });
      if (currentReqId !== planRequestId) {
        return;
      }
      plan = res;
      renderTargetTree(plan);

      const validCount = plan.operations.filter((op) => op.valid).length;
      setProgress(`Plan ready — ${validCount} operation(s) ready.`);
    } catch (error) {
      if (currentReqId !== planRequestId) {
        return;
      }
      plan = null;
      planTreeContent.innerHTML = `<div class="pane-placeholder">${String(error)}</div>`;
      targetPaneBadge.textContent = "Plan failed";
      setProgress(String(error), true);
    } finally {
      if (currentReqId === planRequestId) {
        isPlanning = false;
        planButton.classList.remove("btn-cancel");
        planButton.innerHTML = '<span>Generate Plan</span><span>→</span>';
        setBusy(false);
      }
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
    approveButton.innerHTML = '<span class="spinner"></span><span>Executing…</span>';
    setProgress("Executing the approved actions…");
    try {
      const report = await invoke<ExecutionReport>("execute_plan", {
        root,
        instruction: plan.instruction,
        operations,
      });
      renderReport(report);
      reportPanel.classList.remove("hidden");
      commanderFooter.classList.add("hidden");
      setProgress(`${report.okCount} succeeded, ${report.failedCount} failed.`, report.failedCount > 0);
      void refreshHistory();
      void scanConfidential();
    } catch (error) {
      setProgress(String(error), true);
    } finally {
      setBusy(false);
      approveButton.textContent = "Execute";
    }
  }

  async function scanConfidential(): Promise<void> {
    if (!root || scanning) {
      return;
    }
    scanning = true;
    scanButton.innerHTML = '<span class="spinner spinner-dark"></span><span>Scanning…</span>';
    confidentialPanel.classList.remove("hidden");
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
      scanButton.textContent = "Scan for Confidential Files";
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
      `${report.findings.length} of ${report.scanned} file(s) look confidential — review reasons and set passphrase to encrypt.`,
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
      void refreshHistory();
      setConfidentialStatus(
        `${report.okCount} file(s) encrypted, ${report.failedCount} failed. Use Decrypt to verify recovery.`,
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
        decryptButton.className = "btn-default btn-small";
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
      void refreshHistory();
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
    planEmptyNote.classList.remove("hidden");
    planTreeContent.classList.add("hidden");
    planTreeContent.replaceChildren();
    targetPaneBadge.textContent = "Plan discarded";
    commanderFooter.classList.add("hidden");
    setProgress("Plan discarded — nothing was changed.");
    refreshButtons();
  }

  function reset(): void {
    root = null;
    plan = null;
    instruction.value = "";
    clearConfidential();
    folderName.textContent = "No folder selected";
    folderPath.textContent = "Choose a folder to organize";
    folderPath.classList.add("muted");
    sourceFileList.replaceChildren();
    welcomePane.classList.remove("hidden");
    dualPanes.classList.add("hidden");
    reportPanel.classList.add("hidden");
    commanderFooter.classList.add("hidden");
    setProgress("");
    refreshButtons();
  }

  function selectRoot(nextRoot: string, nextInstruction = ""): void {
    if (nextInstruction.trim().length > 0) {
      instruction.value = nextInstruction;
    }
    void loadFolder(nextRoot);
  }

  // Preset chips click
  document.querySelectorAll<HTMLSpanElement>(".chip-btn").forEach((chip) => {
    chip.addEventListener("click", () => {
      const promptText = chip.dataset.prompt;
      if (promptText) {
        instruction.value = promptText;
        refreshButtons();
        instruction.focus();
      }
    });
  });

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
    .then((_message) => {
      const label = aiStatus.querySelector(".status-label");
      if (label) {
        label.textContent = "100% Local & Private";
      }
      aiStatus.classList.remove("error");
    })
    .catch((error: unknown) => {
      const label = aiStatus.querySelector(".status-label");
      if (label) {
        label.textContent = `Offline engine not ready (${String(error)})`;
      }
      aiStatus.classList.add("error");
    });

  refreshButtons();
  return { selectRoot };
}
