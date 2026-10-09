import { invoke } from "@tauri-apps/api/core";

window.addEventListener("DOMContentLoaded", () => {
  const statusEl = document.querySelector<HTMLElement>("#backend-status");
  if (!statusEl) return;

  invoke<string>("greet", { name: "EpicOrganizer" })
    .then((message) => {
      statusEl.textContent = message;
      statusEl.classList.add("ok");
    })
    .catch((error: unknown) => {
      statusEl.textContent = `Rust core unreachable: ${String(error)}`;
      statusEl.classList.add("error");
    });
});
