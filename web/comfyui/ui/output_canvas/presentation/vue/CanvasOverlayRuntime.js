import {
  ensureCanvasOverlayStyles
} from "../../../chunk-HS2FI4KU.js";
import {
  canvasContextActions,
  canvasMediaItem
} from "../../../chunk-NKMV6DCZ.js";
import {
  CanvasOverlay
} from "../../../chunk-BLQJSKWP.js";
import "../../../chunk-ZAF7C7KA.js";
import "../../../chunk-T26L5BHO.js";
import "../../../chunk-WIEFPHCO.js";
import "../../../chunk-CIRP6VLK.js";
import "../../../chunk-UEULIHLJ.js";
import "../../../chunk-SVNSOY77.js";
import "../../../chunk-5NVJVFWB.js";
import "../../../chunk-Z66IIUOP.js";
import {
  canvasNavigationSnapshot
} from "../../../chunk-HDLA7PWP.js";
import {
  createApp,
  h,
  shallowRef
} from "../../../chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/output_canvas/presentation/CanvasMediaLabel.ts
function canvasMediaLabel(kind) {
  switch (kind) {
    case "model":
      return "3D model";
    case "video":
      return "video";
    case "animated":
      return "animation";
    case "image":
      return "image";
    case "unsupported":
      return "output";
  }
}

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.ts
import { ContextMenu } from "../../../menus/ContextMenu.js";

// frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.ts
function sameCanvasAttachment(left, right) {
  return left.sessionId === right.sessionId && left.viewId === right.viewId && left.windowId === right.windowId && left.attachmentId === right.attachmentId && left.generation === right.generation;
}

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.ts
function mountCanvasOverlay(element, port, reportFailure, interactionSurface = element, contextTarget = () => "primary", route = null) {
  ensureCanvasOverlayStyles(element.ownerDocument);
  const initial = port.read();
  const state = shallowRef(initial);
  const routeState = shallowRef(route?.read() ?? null);
  const stopRoute = route?.subscribe(() => {
    if (active) routeState.value = route.read();
  });
  const announcement = shallowRef("");
  let active = true;
  let sequence = 0;
  async function send(command) {
    if (!active || !port.capabilities.commands.includes(command.type)) return;
    const invocation = ++sequence;
    let request = null;
    try {
      request = port.createRequest(command, state.value.revision);
      const outcome = await port.dispatch(request);
      if (!active || invocation !== sequence) return;
      if (outcome.status === "error") reportFailure(outcome, request);
      announcement.value = outcome.status === "error" ? outcome.message : outcome.status === "stale" ? "View updated; try again." : "";
    } catch (error) {
      reportFailure(error, request);
      if (active && invocation === sequence) {
        announcement.value = error instanceof Error ? error.message : "The canvas command failed.";
      }
    }
  }
  const menu = new ContextMenu(element.ownerDocument);
  let menuTarget = "primary";
  function openMenu(event) {
    if (event instanceof KeyboardEvent && event.key !== "ContextMenu" && !(event.shiftKey && event.key === "F10"))
      return;
    const target = event.target;
    if (target instanceof Element && target.closest("input,select,textarea")) return;
    event.preventDefault();
    event.stopPropagation();
    const bounds = interactionSurface.getBoundingClientRect();
    menuTarget = contextTarget(target);
    menu.open({
      label: "Output actions",
      anchor: {
        kind: "pointer",
        x: event instanceof MouseEvent ? event.clientX : bounds.left + 12,
        y: event instanceof MouseEvent ? event.clientY : bounds.bottom - 48
      },
      trigger: target instanceof HTMLElement ? target : interactionSurface,
      entries: canvasContextActions(state.value, port.capabilities, send, menuTarget)
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
    if (menu.isOpen()) {
      if (snapshot.selected?.id !== state.value.selected?.id || snapshot.secondary?.id !== state.value.secondary?.id || snapshot.preview !== state.value.preview || snapshot.view.mode !== state.value.view.mode || typeof menuTarget === "object" && !canvasMediaItem(snapshot, menuTarget))
        menu.close();
      else if (canvasNavigationSnapshot(snapshot, state.value) !== state.value || snapshot.mediaActions !== state.value.mediaActions)
        menu.updateEntries(canvasContextActions(snapshot, port.capabilities, send, menuTarget));
    }
    const spoken = canvasAnnouncement(state.value, snapshot);
    if (spoken !== null) announcement.value = spoken;
    state.value = snapshot;
  });
  const app = createApp({
    render: () => h(CanvasOverlay, {
      snapshot: state.value,
      capabilities: port.capabilities,
      send,
      announcement: announcement.value,
      route: routeState.value,
      chooseRoute: (value) => {
        if (active) route?.choose(value);
      }
    })
  });
  try {
    app.mount(element);
  } catch (error) {
    active = false;
    closeInteraction();
    unsubscribe();
    stopRoute?.();
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
      stopRoute?.();
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
    return `${source?.label ?? "Output"}: ${canvasMediaLabel(current.selected.mediaKind)} ${current.selected.batchIndex + 1} ready.`;
  if (previous.connected && !current.connected) return "Disconnected.";
  if (current.secondaryResource?.state === "error" && previous.secondaryResource?.state !== "error")
    return `B: ${current.secondaryResource.message}`;
  return null;
}
export {
  mountCanvasOverlay
};
