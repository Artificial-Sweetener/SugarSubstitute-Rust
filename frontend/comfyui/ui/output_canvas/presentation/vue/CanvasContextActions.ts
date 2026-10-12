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
  action(
    'Compare outputs',
    { type: 'comparison', enabled: !comparison },
    state.mode === 'overview' ||
      (!comparison &&
        (!snapshot.selected ||
          snapshot.preview ||
          snapshot.resource.state !== 'ready' ||
          snapshot.selected.mediaKind !== 'image')),
    Boolean(comparison),
  );
  if (comparison) {
    const secondary = comparison.secondary;
    if (capabilities.commands.includes('select-comparison')) {
      entries.push({
        label: 'Compare with',
        children: snapshot.sources.map((source) => ({
          label: source.label,
          checked: secondary?.sourceId === source.id,
          select: () =>
            send({
              type: 'select-comparison',
              selection: { sourceId: source.id, batchIndex: secondary?.batchIndex ?? 0 },
            }),
        })),
      });
      if (secondary) {
        const count =
          snapshot.sources.find((source) => source.id === secondary.sourceId)?.batchCount ?? 0;
        action(
          'Previous comparison image',
          {
            type: 'select-comparison',
            selection: { ...secondary, batchIndex: Math.max(0, secondary.batchIndex - 1) },
          },
          secondary.batchIndex === 0,
        );
        action(
          'Next comparison image',
          {
            type: 'select-comparison',
            selection: { ...secondary, batchIndex: secondary.batchIndex + 1 },
          },
          secondary.batchIndex + 1 >= count,
        );
      }
    }
    if (state.selection)
      action('Refresh first image', { type: 'select', selection: state.selection });
    if (secondary)
      action('Refresh comparison image', { type: 'select-comparison', selection: secondary });
    action('Swap images', { type: 'swap' }, !secondary);
    action(comparison.orientation === 'vertical' ? 'Horizontal split' : 'Vertical split', {
      type: 'wipe',
      split: comparison.split,
      orientation: comparison.orientation === 'vertical' ? 'horizontal' : 'vertical',
    });
  }
  return entries;
}
