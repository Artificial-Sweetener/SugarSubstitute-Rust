import {
  CanvasEditKeyboard
} from "../../chunk-BQYNHRW2.js";
import {
  CanvasEditOverlay,
  canvasEditSurface
} from "../../chunk-J5S5ZY6K.js";
import {
  ensureCanvasEditStyles
} from "../../chunk-CETUOIZM.js";
import "../../chunk-A3QVOJMG.js";
import {
  CanvasEditContextMenu
} from "../../chunk-ZWY52QO3.js";
import "../../chunk-GJM3NOUL.js";
import "../../chunk-AWHG7O74.js";
import "../../chunk-CSZBNOIR.js";
import "../../chunk-3HDL7XRD.js";
import {
  canvasEditSlotOf,
  canvasEditSlotViews,
  canvasEditToolAvailable
} from "../../chunk-KA4VBPJE.js";
import "../../chunk-TPZEQNX7.js";
import {
  DEFAULT_COVERAGE_MODIFICATION
} from "../../chunk-EFQNN4SR.js";
import "../../chunk-2WJX4EBF.js";
import "../../chunk-X5ULNSRV.js";
import {
  createApp,
  h,
  shallowRef
} from "../../chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/canvas_editing/domain/CanvasEditKeys.ts
var NUDGES = {
  ArrowLeft: [-1, 0],
  ArrowRight: [1, 0],
  ArrowUp: [0, -1],
  ArrowDown: [0, 1]
};
function canvasEditKeyIntent(key, state, surface) {
  if (surface.kind === "coverage")
    return key.phase === "down" && key.key === "Escape" ? { kind: "close-coverage" } : null;
  if (key.key === " " && !key.ctrl && !key.meta && !key.alt)
    return { kind: "command", command: { type: "navigation", held: key.phase === "down" } };
  if (key.phase !== "down") return null;
  if (surface.kind === "modify") {
    if (key.key === "Escape") return { kind: "cancel-modify" };
    return key.key === "Enter" ? { kind: "apply-modify" } : null;
  }
  const command = (key.ctrl || key.meta) && !key.alt ? historyCommand(key) : plainCommand(key, state);
  return command ? { kind: "command", command } : null;
}
function historyCommand(key) {
  const letter = key.key.toLowerCase();
  if (letter === "z") return { type: "history", action: key.shift ? "redo" : "undo" };
  if (letter === "y" && !key.shift) return { type: "history", action: "redo" };
  return null;
}
function plainCommand(key, state) {
  if (key.ctrl || key.meta || key.alt) return null;
  if (key.key === "Enter") return { type: "commit" };
  if (key.key === "Escape") return { type: "cancel" };
  if (key.key === "Delete") return state.selection ? { type: "selection", action: "clear" } : null;
  const nudge = NUDGES[key.key];
  if (!nudge || !nudges(state)) return null;
  const step = key.shift ? 10 : 1;
  return { type: "nudge", dx: nudge[0] * step, dy: nudge[1] * step };
}
function nudges(state) {
  return state.tool === "move" || state.session?.kind === "move" || state.session?.kind === "transform";
}

// frontend/comfyui/ui/canvas_editing/application/CanvasEditCommandQueue.ts
var CanvasEditCommandQueue = class {
  /** Borrow the host's correlated transport; the queue owns no request identities. */
  constructor(host, outcomes) {
    this.host = host;
    this.outcomes = outcomes;
  }
  #queued = [];
  #running = false;
  #disposed = false;
  /** Queue a command; resolves with its reply, or null when superseded, closed or failed. */
  send(command, key = null) {
    if (this.#disposed) return Promise.resolve(null);
    return new Promise((settle) => {
      const entry = { command, key, settle };
      const replaced = key === null ? -1 : this.#queued.findIndex((queued) => queued.key === key);
      if (replaced >= 0) {
        this.#queued[replaced]?.settle(null);
        this.#queued[replaced] = entry;
      } else this.#queued.push(entry);
      void this.#pump();
    });
  }
  /** Settle every unsent command without sending it; an in-flight reply is still adopted. */
  dispose() {
    this.#disposed = true;
    for (const entry of this.#queued.splice(0)) entry.settle(null);
  }
  async #pump() {
    if (this.#running) return;
    this.#running = true;
    try {
      for (let entry = this.#queued.shift(); entry; entry = this.#queued.shift())
        entry.settle(await this.#execute(entry.command));
    } finally {
      this.#running = false;
    }
  }
  async #execute(command) {
    try {
      const result = await this.host.execute(command);
      if (result.kind === "state") this.outcomes.adopt(result.state);
      else if (result.status !== "cancelled")
        this.outcomes.report(new Error(result.message), command);
      return result;
    } catch (error) {
      this.outcomes.report(error, command);
      return null;
    }
  }
};

// frontend/comfyui/ui/canvas_editing/application/CanvasEditController.ts
var CanvasEditController = class {
  constructor(options) {
    this.options = options;
    this.host = options.host;
    this.#queue = new CanvasEditCommandQueue(options.host, {
      adopt: (state) => this.#adopt(state),
      report: (error, command) => options.report(error, command)
    });
  }
  host;
  #queue;
  #listeners = /* @__PURE__ */ new Set();
  #remembered = /* @__PURE__ */ new Map();
  #subscription = null;
  #state = null;
  #snapshot = null;
  #modifying = false;
  #modify = DEFAULT_COVERAGE_MODIFICATION;
  #coverage = DEFAULT_COVERAGE_MODIFICATION;
  #navigationHeld = false;
  #disposed = false;
  /** Observe pushed states and request the current one, since the host pushes only changes. */
  start() {
    if (this.#disposed || this.#subscription) return;
    this.#subscription = this.host.changes.subscribe((state) => this.#adopt(state));
    void this.#subscription.ready.then((status) => {
      if (status !== "observing" && !this.#disposed)
        this.options.report(new Error(`Canvas editing observation is ${status}.`), null);
    });
    void this.#queue.send({ type: "state" });
  }
  /** Read the latest snapshot; null until the host presents an editable document. */
  read() {
    return this.#snapshot;
  }
  /** Notify a listener after every snapshot change; returns its release. */
  subscribe(listener) {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }
  /** Activate the member a rail slot presents. */
  selectSlot(slot) {
    const view = this.#snapshot?.slots.find((candidate) => candidate.id === slot);
    if (view?.current.available) this.selectTool(view.current.id);
  }
  /** Activate a tool and remember it as its group's presented member. */
  selectTool(tool) {
    const state = this.#state;
    if (!state || !canvasEditToolAvailable(state, tool)) return;
    const slot = canvasEditSlotOf(tool);
    if (slot) this.#remembered.set(slot, tool);
    this.#publish();
    void this.#queue.send({ type: "tool", tool });
  }
  /** Replace brush or eraser options; drags send only their latest value. */
  setBrushOptions(tool, options) {
    void this.#queue.send({ type: "options", tool, options }, `options:${tool}`);
  }
  /** Send a one-shot command from a toolbar button. */
  execute(command) {
    void this.#queue.send(command);
  }
  /** Open the selection modification page and preview its initial amount at once. */
  beginModify() {
    if (!this.#state?.selection || this.#modifying) return;
    this.#modifying = true;
    this.#modify = DEFAULT_COVERAGE_MODIFICATION;
    this.#publish();
    this.#previewModify();
  }
  /** Replace the selection preview from its original with a new operation or amount. */
  updateModify(value) {
    if (!this.#modifying) return;
    this.#modify = value;
    this.#publish();
    this.#previewModify();
  }
  /** Commit the latest selection preview once. */
  async applyModify() {
    if (!this.#modifying) return;
    const result = await this.#queue.send(this.#modifyCommand("apply"));
    if (result?.kind === "state") this.#closeModify();
  }
  /** Restore the original selection and close the page. */
  cancelModify() {
    if (!this.#modifying && this.#state?.session?.kind !== "modify") return;
    void this.#queue.send(this.#modifyCommand("cancel"));
    this.#closeModify();
  }
  /** Enter the exclusive whole-mask coverage editor and preview its initial amount. */
  async openCoverage() {
    if (!this.#state?.activeMask || this.#state.mode === "coverage") return;
    this.#coverage = DEFAULT_COVERAGE_MODIFICATION;
    const result = await this.#queue.send({ type: "mode", mode: "coverage" });
    if (result?.kind === "state" && result.state.mode === "coverage") this.#previewCoverage();
  }
  /** Replace the whole-mask preview with a new operation or amount. */
  updateCoverage(value) {
    if (this.#state?.mode !== "coverage") return;
    this.#coverage = value;
    this.#publish();
    this.#previewCoverage();
  }
  /** Commit the whole-mask preview, then leave the exclusive editor. */
  async applyCoverage() {
    if (this.#state?.mode !== "coverage") return;
    const result = await this.#queue.send(this.#coverageCommand("apply"));
    if (result?.kind === "state") void this.#queue.send({ type: "mode", mode: "none" });
  }
  /** Discard the whole-mask preview and leave the exclusive editor. */
  closeCoverage() {
    if (this.#state?.mode !== "coverage") return;
    void this.#queue.send(this.#coverageCommand("cancel"));
    void this.#queue.send({ type: "mode", mode: "none" });
  }
  /** Apply one keyboard shortcut; returns whether the canvas consumed the key. */
  key(key) {
    const snapshot = this.#snapshot;
    if (!snapshot) return false;
    const intent = canvasEditKeyIntent(key, snapshot.state, snapshot.surface);
    if (!intent) return false;
    if (intent.kind === "close-coverage") this.closeCoverage();
    else if (intent.kind === "cancel-modify") this.cancelModify();
    else if (intent.kind === "apply-modify") void this.applyModify();
    else if (intent.command.type === "navigation") this.#navigate(intent.command.held);
    else this.execute(intent.command);
    return true;
  }
  /** Release temporary navigation when the page loses keyboard focus mid-hold. */
  releaseNavigation() {
    if (this.#navigationHeld) this.#navigate(false);
  }
  /** Stop observation and drop unsent commands. */
  dispose() {
    if (this.#disposed) return;
    this.releaseNavigation();
    this.#disposed = true;
    this.#subscription?.stop();
    this.#subscription = null;
    this.#queue.dispose();
    this.#listeners.clear();
  }
  #navigate(held) {
    if (held === this.#navigationHeld) return;
    this.#navigationHeld = held;
    this.execute({ type: "navigation", held });
  }
  #previewModify() {
    void this.#queue.send(this.#modifyCommand("preview"), "modify:selection");
  }
  #previewCoverage() {
    void this.#queue.send(this.#coverageCommand("preview"), "modify:mask");
  }
  #modifyCommand(phase) {
    return { type: "modify", target: "selection", ...this.#modify, phase };
  }
  #coverageCommand(phase) {
    return { type: "modify", target: "mask", ...this.#coverage, phase };
  }
  #closeModify() {
    if (!this.#modifying) return;
    this.#modifying = false;
    this.#publish();
  }
  /** Adopt only newer revisions; a resolved modification session closes its page. */
  #adopt(state) {
    if (this.#disposed || this.#state && state.revision <= this.#state.revision) return;
    const previous = this.#state;
    this.#state = state;
    if (this.#modifying && previous?.session?.kind === "modify" && state.session?.kind !== "modify")
      this.#modifying = false;
    if (this.#modifying && !state.selection && state.session?.kind !== "modify")
      this.#modifying = false;
    this.#publish();
  }
  #publish() {
    const state = this.#state;
    this.#snapshot = state && state.document !== null ? Object.freeze({
      state,
      surface: canvasEditSurface(state, this.#modifying),
      slots: canvasEditSlotViews(state, this.#remembered),
      modify: this.#modify,
      coverage: this.#coverage
    }) : null;
    for (const listener of [...this.#listeners]) listener();
  }
};

// frontend/comfyui/ui/canvas_editing/presentation/CanvasEditOverlayRuntime.ts
function mountCanvasEditOverlay(options) {
  const { root } = options;
  const document = root.ownerDocument;
  ensureCanvasEditStyles(document);
  root.classList.add("sc-canvas-edit-root");
  const controller = shallowRef(null);
  const snapshot = shallowRef(null);
  const viewport = shallowRef(null);
  const appearance = shallowRef(0);
  const layoutRevision = shallowRef(0);
  const obstacles = () => {
    const bounds = root.getBoundingClientRect();
    return (options.obstacles?.() ?? []).map((rect) => ({
      left: rect.left - bounds.left,
      top: rect.top - bounds.top,
      width: rect.width,
      height: rect.height
    }));
  };
  let stopSnapshot = null;
  let disposed = false;
  const menu = new CanvasEditContextMenu(options.region, () => controller.value);
  const keyboard = new CanvasEditKeyboard(document, options.scope, root, () => controller.value);
  const contain = (event) => {
    event.preventDefault();
    event.stopPropagation();
  };
  root.addEventListener("contextmenu", contain);
  const app = createApp({
    render: () => {
      void appearance.value;
      const edit = controller.value;
      const current = snapshot.value;
      if (!edit) return null;
      const mask = current?.state.activeMask;
      return h(CanvasEditOverlay, {
        snapshot: current,
        controller: edit,
        viewport: viewport.value,
        maskColor: mask ? options.maskColor?.(mask) ?? null : null,
        obstacles,
        layoutRevision: layoutRevision.value
      });
    }
  });
  app.config.errorHandler = (error) => options.report(error);
  app.mount(root);
  function measure() {
    const geometry = options.geometry();
    if (!geometry) {
      viewport.value = null;
      return;
    }
    const bounds = root.getBoundingClientRect();
    const next = {
      left: geometry.origin.x - bounds.left,
      top: geometry.origin.y - bounds.top,
      width: geometry.viewport.width,
      height: geometry.viewport.height,
      density: geometry.physicalPixelsPerCssPixel
    };
    const previous = viewport.value;
    if (!previous || JSON.stringify(previous) !== JSON.stringify(next)) viewport.value = next;
  }
  function retire() {
    stopSnapshot?.();
    stopSnapshot = null;
    controller.value?.dispose();
    controller.value = null;
    snapshot.value = null;
    menu.close();
  }
  return {
    attach(host) {
      if (disposed || controller.value?.host === host) return;
      retire();
      if (!host) return;
      const next = new CanvasEditController({
        host,
        report: (error) => options.report(error)
      });
      stopSnapshot = next.subscribe(() => {
        snapshot.value = next.read();
        if (!snapshot.value) menu.close();
      });
      controller.value = next;
      measure();
      next.start();
    },
    geometryChanged() {
      if (disposed) return;
      measure();
      layoutRevision.value++;
    },
    refresh() {
      if (!disposed) appearance.value++;
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      retire();
      keyboard.dispose();
      menu.dispose();
      root.removeEventListener("contextmenu", contain);
      app.unmount();
      root.classList.remove("sc-canvas-edit-root");
    }
  };
}
export {
  mountCanvasEditOverlay
};
