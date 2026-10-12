import {
  canvasContextActions
} from "../../../chunk-OKGOB56C.js";
import {
  CanvasOverlay
} from "../../../chunk-UGJKIAX4.js";
import "../../../chunk-OVL65VLS.js";
import "../../../chunk-ZAPDTI3U.js";
import {
  canvasNavigationSnapshot
} from "../../../chunk-HDLA7PWP.js";
import {
  ensureCanvasOverlayStyles
} from "../../../chunk-GN4T4KG6.js";
import "../../../chunk-MS5JIV6F.js";
import {
  createApp,
  h,
  shallowRef
} from "../../../chunk-MNPWJMUZ.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.ts
import { ContextMenu } from "../../../menus/ContextMenu.js";

// frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.ts
function sameCanvasAttachment(left, right) {
  return left.sessionId === right.sessionId && left.viewId === right.viewId && left.windowId === right.windowId && left.attachmentId === right.attachmentId && left.generation === right.generation;
}

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.ts
function mountCanvasOverlay(element, port, reportFailure, interactionSurface = element) {
  ensureCanvasOverlayStyles(element.ownerDocument);
  const initial = port.read();
  const state = shallowRef(initial);
  const announcement = shallowRef("");
  let active = true;
  let sequence = 0;
  async function send(command) {
    if (!active || !port.capabilities.commands.includes(command.type)) return;
    const requestId = ++sequence;
    const request = {
      schema: 1,
      address: initial.address,
      requestId,
      expectedRevision: state.value.revision,
      command
    };
    try {
      const outcome = await port.dispatch(request);
      if (!active || requestId !== sequence) return;
      if (outcome.status === "error") reportFailure(outcome, request);
      announcement.value = outcome.status === "error" ? outcome.message : outcome.status === "stale" ? "View updated; try again." : "";
    } catch (error) {
      reportFailure(error, request);
      if (active && requestId === sequence) {
        announcement.value = error instanceof Error ? error.message : "The canvas command failed.";
      }
    }
  }
  const menu = new ContextMenu(element.ownerDocument);
  function openMenu(event) {
    if (event instanceof KeyboardEvent && event.key !== "ContextMenu" && !(event.shiftKey && event.key === "F10"))
      return;
    const target = event.target;
    if (target instanceof Element && target.closest("input,select,textarea")) return;
    event.preventDefault();
    event.stopPropagation();
    const bounds = interactionSurface.getBoundingClientRect();
    menu.open({
      label: "Output actions",
      anchor: {
        kind: "pointer",
        x: event instanceof MouseEvent ? event.clientX : bounds.left + 12,
        y: event instanceof MouseEvent ? event.clientY : bounds.bottom - 48
      },
      trigger: target instanceof HTMLElement ? target : interactionSurface,
      entries: canvasContextActions(state.value, port.capabilities, send)
    });
  }
  interactionSurface.addEventListener("contextmenu", openMenu);
  interactionSurface.addEventListener("keydown", openMenu);
  function closeInteraction() {
    menu.close();
    interactionSurface.removeEventListener("contextmenu", openMenu);
    interactionSurface.removeEventListener("keydown", openMenu);
  }
  const unsubscribe = port.subscribe((snapshot) => {
    if (!active || !sameCanvasAttachment(initial.address, snapshot.address) || snapshot.revision <= state.value.revision)
      return;
    if (menu.isOpen() && canvasNavigationSnapshot(snapshot, state.value) !== state.value)
      menu.updateEntries(canvasContextActions(snapshot, port.capabilities, send));
    const spoken = canvasAnnouncement(state.value, snapshot);
    if (spoken !== null) announcement.value = spoken;
    state.value = snapshot;
  });
  const app = createApp({
    render: () => h(CanvasOverlay, {
      snapshot: state.value,
      capabilities: port.capabilities,
      send,
      announcement: announcement.value
    })
  });
  try {
    app.mount(element);
  } catch (error) {
    active = false;
    closeInteraction();
    unsubscribe();
    reportFailure(error, null);
    throw error;
  }
  return {
    /** Make root teardown idempotent while invalidating pending command feedback. */
    dispose() {
      if (!active) return;
      active = false;
      closeInteraction();
      unsubscribe();
      app.unmount();
    }
  };
}
function canvasAnnouncement(previous, current) {
  const source = current.sources.find((entry) => entry.id === current.view.selection?.sourceId);
  if (previous.view.selection?.sourceId !== current.view.selection?.sourceId)
    return source?.label ?? "Output source is unavailable.";
  if (current.resource.state === "error" && (previous.resource.state !== "error" || previous.resource.message !== current.resource.message))
    return current.resource.message;
  if (!current.preview && current.resource.state === "ready" && current.selected && (previous.selected?.id !== current.selected.id || previous.resource.state !== "ready" || previous.preview))
    return `${source?.label ?? "Output"}: image ${current.selected.batchIndex + 1} ready.`;
  if (previous.connected && !current.connected) return "Disconnected.";
  if (current.secondaryResource?.state === "error" && previous.secondaryResource?.state !== "error")
    return `B: ${current.secondaryResource.message}`;
  return null;
}
export {
  mountCanvasOverlay
};
