import { initHistory } from "./history";
import { initOrganizer } from "./organizer";

window.addEventListener("DOMContentLoaded", () => {
  const organizer = initOrganizer();
  initHistory({
    onSelect: (entry) => organizer.selectRoot(entry.root, entry.lastInstruction),
  });
});
