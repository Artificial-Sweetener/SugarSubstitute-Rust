import {
  CanvasOverlay
} from "../../../chunk-S6OY25DX.js";
import "../../../chunk-NDQ7H2BO.js";
import "../../../chunk-OJGHAI47.js";
import "../../../chunk-XB46K7OE.js";
import {
  createApp,
  h,
  shallowRef
} from "../../../chunk-MNPWJMUZ.js";

// frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.ts
function sameCanvasAttachment(left, right) {
  return left.sessionId === right.sessionId && left.viewId === right.viewId && left.windowId === right.windowId && left.attachmentId === right.attachmentId && left.generation === right.generation;
}

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.ts
function mountCanvasOverlay(element, port, reportFailure) {
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
  const unsubscribe = port.subscribe((snapshot) => {
    if (!active || !sameCanvasAttachment(initial.address, snapshot.address) || snapshot.revision <= state.value.revision)
      return;
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
    unsubscribe();
    reportFailure(error, null);
    throw error;
  }
  return {
    /** Make root teardown idempotent while invalidating pending command feedback. */
    dispose() {
      if (!active) return;
      active = false;
      unsubscribe();
      app.unmount();
    }
  };
}
export {
  mountCanvasOverlay
};
