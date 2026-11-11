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
/** Project the original Substitute Output menu over renderer-independent media intents. */

import type { ContextMenuEntry } from '../../../menus/ContextMenu.js';
import type { WorkflowOutputItem } from '../../../execution/domain/WorkflowOutput.js';
import {
  canvasMediaItem,
  canvasMediaFile,
  type CanvasMediaTarget,
} from '../../domain/CanvasMedia.js';
import type {
  CanvasCapabilities,
  CanvasCommand,
  CanvasViewSnapshot,
} from '../../domain/CanvasViewPort.js';

/**
 * Offer the Output actions in the original order: Compare outputs, then Copy, Open in Photoshop,
 * Open All in Photoshop and Reveal in File Manager, delegating every transition to its owner.
 *
 * The overview offers no menu away from an image unless outputs are being compared; an image
 * there offers the actions addressed to it, without Open All. Video offers neither Compare nor
 * Photoshop. Camera commands belong to the canvas's own controls and keys, not this menu.
 */
export function canvasContextActions(
  snapshot: CanvasViewSnapshot,
  capabilities: CanvasCapabilities,
  send: (command: CanvasCommand) => void,
  target: CanvasMediaTarget = 'primary',
): ContextMenuEntry[] {
  const state = snapshot.view;
  const comparison = state.comparison;
  const tile = typeof target === 'object';
  if (state.mode === 'overview' && !tile && !comparison) return [];
  const entries: ContextMenuEntry[] = [];
  /** Include only commands supported by this renderer without reproducing their policies. */
  function action(
    label: string,
    command: CanvasCommand,
    disabled = false,
    checked?: boolean,
  ): void {
    if (!capabilities.commands.includes(command.type)) return;
    entries.push({
      label,
      disabled,
      ...(checked === undefined ? {} : { checked, selection: 'multiple' }),
      select: () => send(command),
    });
  }
  const video = targetItem(snapshot, target)?.mediaKind === 'video';
  if (!video && (comparison || (state.mode !== 'overview' && snapshot.comparisonAvailable)))
    action(
      'Compare outputs',
      { type: 'comparison', enabled: !comparison },
      !comparison &&
        (!snapshot.selected ||
          snapshot.preview ||
          snapshot.resource.state !== 'ready' ||
          snapshot.selected.mediaKind !== 'image'),
      Boolean(comparison),
    );
  const comparisonCount = entries.length;
  const unavailable = canvasMediaItem(snapshot, target) === null;
  const item = targetItem(snapshot, target);
  const actions = item ? snapshot.itemActions?.find((actions) => actions.itemId === item.id) : null;
  action(
    'Copy',
    { type: 'copy-media', target, format: 'preferred' },
    unavailable || actions?.copy === false,
  );
  if (!video && snapshot.mediaActions?.photoshop) {
    action(
      'Open in Photoshop',
      { type: 'photoshop-media', target },
      unavailable || actions?.photoshop === false,
    );
    if (!tile)
      action(
        'Open All in Photoshop',
        { type: 'photoshop-all-media' },
        snapshot.photoshopAllAvailable === false,
      );
  }
  if (snapshot.mediaActions)
    action(
      (actions?.fileAction ?? snapshot.mediaActions.fileAction) === 'reveal'
        ? 'Reveal in File Manager'
        : 'Download',
      { type: 'file-media', target },
      canvasMediaFile(snapshot, target) === null || actions?.file === false,
    );
  if (comparisonCount > 0 && entries.length > comparisonCount)
    entries.splice(comparisonCount, 0, { label: '', separator: true });
  return entries;
}

/** The addressed output of any media kind, so video can withhold image-only actions. */
function targetItem(
  snapshot: CanvasViewSnapshot,
  target: CanvasMediaTarget,
): WorkflowOutputItem | null {
  if (target === 'primary') return snapshot.selected;
  if (target === 'secondary') return snapshot.secondary;
  return (
    snapshot.grid
      .find((entry) => entry.source.id === target.sourceId)
      ?.items.find((item) => item.id === target.itemId) ?? null
  );
}
