import {
  CanvasCoverageControls
} from "./chunk-V45VCN34.js";
import {
  canvasEditToolAvailable
} from "./chunk-SITYEXXI.js";
import {
  CANVAS_EDIT_TEXT,
  CANVAS_TOOL_LABELS
} from "./chunk-OJDBLLHY.js";
import {
  canvasEditIcon
} from "./chunk-MOOZ7AAA.js";
import {
  h
} from "./chunk-ZMBAFDHL.js";

// frontend/comfyui/ui/canvas_editing/presentation/CanvasEditPages.ts
function iconButton(icon, label, action, run, enabled = true, primary = false) {
  return h(
    "button",
    {
      type: "button",
      class: primary ? "sc-canvas-edit-icon-button sc-canvas-edit-primary" : "sc-canvas-edit-icon-button",
      title: label,
      "aria-label": label,
      "data-canvas-action": action,
      disabled: !enabled,
      onClick: run
    },
    [canvasEditIcon(icon)]
  );
}
function separator() {
  return h("span", { class: "sc-canvas-edit-separator", "aria-hidden": "true" });
}
function settlement(apply, cancel, canApply) {
  return [
    iconButton("apply", CANVAS_EDIT_TEXT.apply, "apply", apply, canApply, true),
    iconButton("close", CANVAS_EDIT_TEXT.cancel, "cancel", cancel)
  ];
}
function selectionPage(snapshot, edit) {
  const state = snapshot.state;
  return [
    h(
      "button",
      { type: "button", "data-canvas-action": "modify", onClick: () => edit.beginModify() },
      [canvasEditIcon("modify"), CANVAS_EDIT_TEXT.modifySelection]
    ),
    separator(),
    iconButton(
      "transform",
      CANVAS_EDIT_TEXT.transform,
      "transform",
      () => edit.selectTool("transform"),
      canvasEditToolAvailable(state, "transform")
    ),
    iconButton(
      "eraser",
      CANVAS_EDIT_TEXT.clear,
      "clear",
      () => edit.execute({ type: "selection", action: "clear" }),
      state.activeMask !== null
    ),
    iconButton(
      "deselect",
      CANVAS_EDIT_TEXT.deselect,
      "deselect",
      () => edit.execute({ type: "selection", action: "deselect" })
    )
  ];
}
function modifyPage(snapshot, edit) {
  return [
    h(CanvasCoverageControls, {
      value: snapshot.modify,
      change: (value) => edit.updateModify(value)
    }),
    ...settlement(
      () => void edit.applyModify(),
      () => edit.cancelModify(),
      true
    )
  ];
}
function sessionHistory(snapshot, edit) {
  const session = snapshot.state.session;
  return [
    iconButton(
      "undo",
      CANVAS_EDIT_TEXT.undo,
      "session-undo",
      () => edit.execute({ type: "session", action: "undo" }),
      session?.undo ?? false
    ),
    iconButton(
      "redo",
      CANVAS_EDIT_TEXT.redo,
      "session-redo",
      () => edit.execute({ type: "session", action: "redo" }),
      session?.redo ?? false
    )
  ];
}
function transformPage(snapshot, edit, target) {
  const transform = (action) => edit.execute({ type: "transform", action });
  return [
    h(
      "span",
      { class: "sc-canvas-edit-label" },
      target === "selection" ? CANVAS_EDIT_TEXT.transformSelection : CANVAS_EDIT_TEXT.transform
    ),
    iconButton(
      "rotate-left",
      CANVAS_EDIT_TEXT.rotateLeft,
      "rotate-left",
      () => transform("rotate-left")
    ),
    iconButton(
      "rotate-right",
      CANVAS_EDIT_TEXT.rotateRight,
      "rotate-right",
      () => transform("rotate-right")
    ),
    iconButton(
      "flip-horizontal",
      CANVAS_EDIT_TEXT.flipHorizontal,
      "flip-horizontal",
      () => transform("flip-horizontal")
    ),
    iconButton(
      "flip-vertical",
      CANVAS_EDIT_TEXT.flipVertical,
      "flip-vertical",
      () => transform("flip-vertical")
    ),
    ...sessionHistory(snapshot, edit),
    // Apply and Cancel also leave Transform, Cancel even before a change; Enter and Esc only
    // resolve the transform in place.
    ...settlement(
      () => edit.execute({ type: "session", action: "apply" }),
      () => edit.execute({ type: "session", action: "cancel" }),
      snapshot.state.session !== null
    )
  ];
}
function sessionPage(snapshot, edit, tool) {
  const active = snapshot.state.session !== null;
  return [
    h("span", { class: "sc-canvas-edit-label" }, CANVAS_TOOL_LABELS[tool]),
    ...sessionHistory(snapshot, edit),
    ...settlement(
      () => edit.execute({ type: "session", action: "apply" }),
      // An armed tool without a session exits through the ordinary Esc command.
      () => edit.execute(active ? { type: "session", action: "cancel" } : { type: "cancel" }),
      active
    )
  ];
}

export {
  iconButton,
  selectionPage,
  modifyPage,
  transformPage,
  sessionPage
};
