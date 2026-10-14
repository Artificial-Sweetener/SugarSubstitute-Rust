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
/** Project renderer-independent inspection intents into the shared transient menu. */

import type { ContextMenuEntry } from '../../../menus/ContextMenu.js';
import { canvasMediaItem, type CanvasMediaTarget } from '../../domain/CanvasMedia.js';
import type {
  CanvasCapabilities,
  CanvasCommand,
  CanvasViewSnapshot,
} from '../../domain/CanvasViewPort.js';

/** Keep optional actions off the image while delegating every transition to its view authority. */
export function canvasContextActions(
  snapshot: CanvasViewSnapshot,
  capabilities: CanvasCapabilities,
  send: (command: CanvasCommand) => void,
  target: CanvasMediaTarget = 'primary',
): ContextMenuEntry[] {
  const entries: ContextMenuEntry[] = [];
  const state = snapshot.view;
  const comparison = state.comparison;
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
  const available = canvasMediaItem(snapshot, target) !== null;
  if (snapshot.mediaActions)
    action(
      snapshot.mediaActions.fileAction === 'reveal' ? 'Reveal in folder' : 'Download',
      { type: 'file-media', target },
      !available,
    );
  /** Present the shared preferred format first and the explicit alternate format second. */
  function copies(itemTarget: CanvasMediaTarget, label: string, alternateLabel: string): void {
    const disabled = canvasMediaItem(snapshot, itemTarget) === null;
    action(label, { type: 'copy-media', target: itemTarget, format: 'preferred' }, disabled);
    if (snapshot.mediaActions) {
      const format = snapshot.mediaActions.preferredCopyFormat === 'jpeg' ? 'png' : 'jpeg';
      action(
        `${alternateLabel} .${format === 'jpeg' ? 'jpg' : 'png'}`,
        { type: 'copy-media', target: itemTarget, format },
        disabled,
      );
    }
  }
  copies(target, 'Copy', 'Copy');
  if (comparison && target === 'primary')
    copies('secondary', 'Copy comparison image', 'Copy comparison');
  action(
    'Compare outputs',
    { type: 'comparison', enabled: !comparison },
    state.mode === 'overview' ||
      (!comparison &&
        (!snapshot.comparisonAvailable ||
          !snapshot.selected ||
          snapshot.preview ||
          snapshot.resource.state !== 'ready' ||
          snapshot.selected.mediaKind !== 'image')),
    Boolean(comparison),
  );
  if (comparison) {
    const secondary = comparison.secondary;
    action('Swap images', { type: 'swap' }, !secondary);
    action(comparison.orientation === 'vertical' ? 'Horizontal split' : 'Vertical split', {
      type: 'wipe',
      split: comparison.split,
      orientation: comparison.orientation === 'vertical' ? 'horizontal' : 'vertical',
    });
  }
  return entries;
}
