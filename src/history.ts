import { invoke } from "@tauri-apps/api/core";
import { ask } from "@tauri-apps/plugin-dialog";

import type { DecryptFileResult, HistoryEntry, SealedFileEntry } from "./types";

function element<T extends HTMLElement>(id: string): T {
  const node = document.getElementById(id);
  if (!node) {
    throw new Error(`Missing element #${id}`);
  }
  return node as unknown as T;
}

export interface HistoryOptions {
  onSelect: (entry: HistoryEntry) => void;
}

let listNode!: HTMLUListElement;
let emptyNode!: HTMLParagraphElement;
let clearButton!: HTMLButtonElement;
let options!: HistoryOptions;

function baseName(path: string): string {
  const parts = path.split(/[\\/]/).filter((part) => part.length > 0);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

function timeAgo(millis: number): string {
  const seconds = Math.max(0, Math.round((Date.now() - millis) / 1000));
  if (seconds < 60) {
    return "just now";
  }
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) {
    return `${minutes} min ago`;
  }
  const hours = Math.round(minutes / 60);
  if (hours < 24) {
    return `${hours} h ago`;
  }
  const days = Math.round(hours / 24);
  return `${days} d ago`;
}

function metaText(entry: HistoryEntry): string {
  const ops = `${entry.opsOk} op${entry.opsOk === 1 ? "" : "s"}`;
  const failed = entry.opsFailed > 0 ? ` · ${entry.opsFailed} failed` : "";
  const sealed = entry.sealed.length > 0 ? ` · ${entry.sealed.length} sealed` : "";
  const when = entry.exists ? timeAgo(entry.lastOrganized) : "folder not found";
  return `${when} · ${ops}${failed}${sealed}`;
}

function closeOpenPrompts(except?: HTMLElement): void {
  for (const prompt of listNode.querySelectorAll<HTMLElement>(".sealed-decrypt")) {
    if (prompt !== except) {
      prompt.classList.add("hidden");
    }
  }
}

function sealedRow(entry: HistoryEntry, file: SealedFileEntry): HTMLLIElement {
  const row = document.createElement("li");
  row.className = "sealed-row";

  const name = document.createElement("span");
  name.className = "sealed-name";
  name.textContent = file.path.toLowerCase().endsWith(".enc") ? file.path.slice(0, -4) : file.path;
  name.title = file.path;

  const badge = document.createElement("span");
  badge.className = "badge";
  badge.textContent = file.restored ? "restored" : "sealed";
  if (file.restored) {
    badge.classList.add("restored");
  }
  row.append(name, badge);

  if (!file.exists) {
    const missing = document.createElement("span");
    missing.className = "reason";
    missing.textContent = "sealed copy missing";
    row.append(missing);
    return row;
  }
  if (!entry.exists) {
    return row;
  }

  const button = document.createElement("button");
  button.type = "button";
  button.className = "button small";
  button.textContent = "Decrypt";

  const prompt = document.createElement("div");
  prompt.className = "sealed-decrypt hidden";
  const input = document.createElement("input");
  input.className = "input";
  input.type = "password";
  input.placeholder = "Passphrase (never stored)";
  input.autocomplete = "off";
  const confirmButton = document.createElement("button");
  confirmButton.type = "button";
  confirmButton.className = "button small primary";
  confirmButton.textContent = "Decrypt";
  const message = document.createElement("span");
  message.className = "reason sealed-message";
  prompt.append(input, confirmButton, message);
  row.append(button, prompt);

  const submit = async (): Promise<void> => {
    const passphrase = input.value;
    if (passphrase.length === 0) {
      message.classList.remove("ok");
      message.classList.add("error");
      message.textContent = "Enter the passphrase.";
      input.focus();
      return;
    }
    confirmButton.disabled = true;
    message.classList.remove("ok", "error");
    message.textContent = "Decrypting…";
    try {
      const result = await invoke<DecryptFileResult>("decrypt_file", {
        root: entry.root,
        path: file.path,
        passphrase,
      });
      input.value = "";
      message.classList.add("ok");
      message.textContent = result.message;
      badge.textContent = "restored";
      badge.classList.add("restored");
    } catch (error) {
      message.classList.add("error");
      message.textContent = `Decrypt failed — ${String(error)}`;
    } finally {
      confirmButton.disabled = false;
    }
  };

  button.addEventListener("click", () => {
    const wasHidden = prompt.classList.contains("hidden");
    closeOpenPrompts(prompt);
    prompt.classList.toggle("hidden", !wasHidden);
    if (wasHidden) {
      input.focus();
    }
  });
  confirmButton.addEventListener("click", () => void submit());
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      void submit();
    }
  });

  return row;
}

function entryRow(entry: HistoryEntry): HTMLLIElement {
  const row = document.createElement("li");
  row.className = "history-entry";
  if (!entry.exists) {
    row.classList.add("missing");
  }

  const header = document.createElement("div");
  header.className = "history-row";

  const main = document.createElement("button");
  main.type = "button";
  main.className = "history-main";
  main.disabled = !entry.exists;
  if (!entry.exists) {
    main.title = "This folder no longer exists on disk";
  }

  const name = document.createElement("span");
  name.className = "history-name";
  name.textContent = baseName(entry.root);

  const path = document.createElement("span");
  path.className = "history-path";
  path.textContent = entry.root;
  path.title = entry.root;

  const meta = document.createElement("span");
  meta.className = "history-meta";
  meta.textContent = metaText(entry);

  main.append(name, path, meta);
  if (entry.lastInstruction.trim().length > 0) {
    const instruction = document.createElement("span");
    instruction.className = "history-instruction";
    instruction.textContent = entry.lastInstruction;
    main.append(instruction);
  }
  if (entry.exists) {
    main.addEventListener("click", () => options.onSelect(entry));
  }
  header.append(main);

  if (entry.sealed.length > 0) {
    const toggle = document.createElement("button");
    toggle.type = "button";
    toggle.className = "history-toggle";
    toggle.textContent = `${entry.sealed.length} sealed`;
    toggle.setAttribute("aria-expanded", "false");
    const sealedList = document.createElement("ul");
    sealedList.className = "sealed-list hidden";
    sealedList.append(...entry.sealed.map((file) => sealedRow(entry, file)));
    toggle.addEventListener("click", () => {
      const collapsed = sealedList.classList.toggle("hidden");
      toggle.setAttribute("aria-expanded", String(!collapsed));
      toggle.textContent = collapsed ? `${entry.sealed.length} sealed` : "Hide sealed";
      if (collapsed) {
        closeOpenPrompts();
      }
    });
    header.append(toggle);
    row.append(sealedList);
  }

  const remove = document.createElement("button");
  remove.type = "button";
  remove.className = "history-remove";
  remove.textContent = "×";
  remove.title = "Remove from history";
  remove.setAttribute("aria-label", `Remove ${entry.root} from history`);
  remove.addEventListener("click", async () => {
    remove.disabled = true;
    try {
      await invoke("remove_history", { root: entry.root });
      await refreshHistory();
    } catch (error) {
      remove.disabled = false;
      listNode.after(errorNode(`Could not remove the entry — ${String(error)}`));
    }
  });
  header.append(remove);

  row.prepend(header);
  return row;
}

function errorNode(message: string): HTMLParagraphElement {
  const node = document.createElement("p");
  node.className = "progress error rail-error";
  node.textContent = message;
  return node;
}

function render(entries: HistoryEntry[]): void {
  for (const stale of listNode.parentElement?.querySelectorAll(".rail-error") ?? []) {
    stale.remove();
  }
  listNode.replaceChildren(...entries.map(entryRow));
  emptyNode.classList.toggle("hidden", entries.length > 0);
  if (entries.length === 0) {
    emptyNode.textContent = "No folders organized yet.";
  }
  clearButton.disabled = entries.length === 0;
}

export async function refreshHistory(): Promise<void> {
  if (!listNode) {
    return;
  }
  try {
    render(await invoke<HistoryEntry[]>("list_history"));
  } catch (error) {
    listNode.replaceChildren();
    emptyNode.classList.remove("hidden");
    emptyNode.textContent = `Could not load history — ${String(error)}`;
  }
}

async function clearAll(): Promise<void> {
  const confirmed = await ask("Remove every folder from the history?", {
    title: "Clear history",
    kind: "warning",
  });
  if (!confirmed) {
    return;
  }
  try {
    await invoke("clear_history");
    await refreshHistory();
  } catch (error) {
    emptyNode.textContent = `Could not clear history — ${String(error)}`;
    emptyNode.classList.remove("hidden");
  }
}

export function initHistory(nextOptions: HistoryOptions): void {
  options = nextOptions;
  listNode = element("history-list");
  emptyNode = element("history-empty");
  clearButton = element("clear-history");
  clearButton.addEventListener("click", () => void clearAll());
  void refreshHistory();
}
