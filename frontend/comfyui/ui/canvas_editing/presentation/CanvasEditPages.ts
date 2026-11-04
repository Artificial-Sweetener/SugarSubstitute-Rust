//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Build the rows of the contextual toolbar for each editing surface. */
import { h, type VNode } from 'vue';
import type {
  CanvasEditController,
  CanvasEditSnapshot,
} from '../application/CanvasEditController.js';
import { canvasEditToolAvailable } from '../domain/CanvasEditTools.js';
import { CanvasCoverageControls } from './CanvasCoverageControls.js';
import type { CanvasEditIconName } from './CanvasEditArtwork.js';
import { canvasEditIcon } from './CanvasEditIcons.js';
import { CANVAS_EDIT_TEXT, CANVAS_TOOL_LABELS } from './CanvasEditText.js';

/** Render an icon-only command with its name as tooltip and accessible label. */
export function iconButton(
  icon: CanvasEditIconName,
  label: string,
  action: string,
  run: () => void,
  enabled = true,
  primary = false,
): VNode {
  return h(
    'button',
    {
      type: 'button',
      class: primary
        ? 'sc-canvas-edit-icon-button sc-canvas-edit-primary'
        : 'sc-canvas-edit-icon-button',
      title: label,
      'aria-label': label,
      'data-canvas-action': action,
      disabled: !enabled,
      onClick: run,
    },
    [canvasEditIcon(icon)],
  );
}

function separator(): VNode {
  return h('span', { class: 'sc-canvas-edit-separator', 'aria-hidden': 'true' });
}

/** Apply on the left, Cancel on the right, as the reference settlement pattern places them. */
function settlement(apply: () => void, cancel: () => void, canApply: boolean): VNode[] {
  return [
    iconButton('apply', CANVAS_EDIT_TEXT.apply, 'apply', apply, canApply, true),
    iconButton('close', CANVAS_EDIT_TEXT.cancel, 'cancel', cancel),
  ];
}

/** Modify selection, then Transform, Clear and Deselect. */
export function selectionPage(snapshot: CanvasEditSnapshot, edit: CanvasEditController): VNode[] {
  const state = snapshot.state;
  return [
    h(
      'button',
      { type: 'button', 'data-canvas-action': 'modify', onClick: () => edit.beginModify() },
      [canvasEditIcon('modify'), CANVAS_EDIT_TEXT.modifySelection],
    ),
    separator(),
    iconButton(
      'transform',
      CANVAS_EDIT_TEXT.transform,
      'transform',
      () => edit.selectTool('transform'),
      canvasEditToolAvailable(state, 'transform'),
    ),
    iconButton(
      'eraser',
      CANVAS_EDIT_TEXT.clear,
      'clear',
      () => edit.execute({ type: 'selection', action: 'clear' }),
      state.activeMask !== null,
    ),
    iconButton('deselect', CANVAS_EDIT_TEXT.deselect, 'deselect', () =>
      edit.execute({ type: 'selection', action: 'deselect' }),
    ),
  ];
}

/** The live selection modification preview and its settlement. */
export function modifyPage(snapshot: CanvasEditSnapshot, edit: CanvasEditController): VNode[] {
  return [
    h(CanvasCoverageControls, {
      value: snapshot.modify,
      change: (value: typeof snapshot.modify) => edit.updateModify(value),
    }),
    ...settlement(
      () => void edit.applyModify(),
      () => edit.cancelModify(),
      true,
    ),
  ];
}

/** Session history controls shared by affine and edit-session pages. */
function sessionHistory(snapshot: CanvasEditSnapshot, edit: CanvasEditController): VNode[] {
  const session = snapshot.state.session;
  return [
    iconButton(
      'undo',
      CANVAS_EDIT_TEXT.undo,
      'session-undo',
      () => edit.execute({ type: 'session', action: 'undo' }),
      session?.undo ?? false,
    ),
    iconButton(
      'redo',
      CANVAS_EDIT_TEXT.redo,
      'session-redo',
      () => edit.execute({ type: 'session', action: 'redo' }),
      session?.redo ?? false,
    ),
  ];
}

/** Rotate, flip, provisional history, Apply and Cancel for one affine transform. */
export function transformPage(
  snapshot: CanvasEditSnapshot,
  edit: CanvasEditController,
  target: 'selection' | 'layer',
): VNode[] {
  const transform = (
    action: 'rotate-left' | 'rotate-right' | 'flip-horizontal' | 'flip-vertical',
  ) => edit.execute({ type: 'transform', action });
  return [
    h(
      'span',
      { class: 'sc-canvas-edit-label' },
      target === 'selection' ? CANVAS_EDIT_TEXT.transformSelection : CANVAS_EDIT_TEXT.transform,
    ),
    iconButton('rotate-left', CANVAS_EDIT_TEXT.rotateLeft, 'rotate-left', () =>
      transform('rotate-left'),
    ),
    iconButton('rotate-right', CANVAS_EDIT_TEXT.rotateRight, 'rotate-right', () =>
      transform('rotate-right'),
    ),
    iconButton('flip-horizontal', CANVAS_EDIT_TEXT.flipHorizontal, 'flip-horizontal', () =>
      transform('flip-horizontal'),
    ),
    iconButton('flip-vertical', CANVAS_EDIT_TEXT.flipVertical, 'flip-vertical', () =>
      transform('flip-vertical'),
    ),
    ...sessionHistory(snapshot, edit),
    ...settlement(
      () => edit.execute({ type: 'commit' }),
      () => edit.execute({ type: 'cancel' }),
      snapshot.state.session !== null,
    ),
  ];
}

/** The tool's name, provisional history, Apply and Cancel for a move or shared-edge session. */
export function sessionPage(
  snapshot: CanvasEditSnapshot,
  edit: CanvasEditController,
  tool: 'move' | 'shared-edges',
): VNode[] {
  const active = snapshot.state.session !== null;
  return [
    h('span', { class: 'sc-canvas-edit-label' }, CANVAS_TOOL_LABELS[tool]),
    ...sessionHistory(snapshot, edit),
    ...settlement(
      () => edit.execute({ type: 'session', action: 'apply' }),
      // An armed tool without a session exits through the ordinary Esc command.
      () => edit.execute(active ? { type: 'session', action: 'cancel' } : { type: 'cancel' }),
      active,
    ),
  ];
}
