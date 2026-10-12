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
/** Choose and register the secondary output through the owning view port. */

import { defineComponent, h, type PropType } from 'vue';
import type {
  CanvasCapabilities,
  CanvasCommand,
  CanvasViewSnapshot,
} from '../../domain/CanvasViewPort.js';

/** Keep comparison selection in the authority rather than in a local Vue store. */
export const CanvasCompareControls = defineComponent({
  name: 'CanvasCompareControls',
  props: {
    snapshot: { type: Object as PropType<CanvasViewSnapshot>, required: true },
    capabilities: { type: Object as PropType<CanvasCapabilities>, required: true },
    send: { type: Function as PropType<(command: CanvasCommand) => void>, required: true },
  },
  /** Wire secondary selection intents while retaining authority-owned comparison state. */
  setup(props) {
    return () => {
      const comparison = props.snapshot.view.comparison;
      if (!comparison) return null;
      const secondary = comparison.secondary;
      const source = props.snapshot.sources.find(
        (candidate) => candidate.id === secondary?.sourceId,
      );
      return h('div', { class: 'sc-output-comparison', 'aria-label': 'Comparison controls' }, [
        h(
          'span',
          `A: ${props.snapshot.sources.find((candidate) => candidate.id === props.snapshot.view.selection?.sourceId)?.label ?? 'Unavailable output'}`,
        ),
        h('span', 'A/B pinned'),
        props.capabilities.commands.includes('select') && props.snapshot.view.selection
          ? h(
              'button',
              {
                type: 'button',
                title: 'Use the latest result for the selected A source and batch.',
                onClick: () => {
                  const selection = props.snapshot.view.selection;
                  if (selection) props.send({ type: 'select', selection });
                },
              },
              'Replace A',
            )
          : null,
        props.capabilities.commands.includes('select-comparison') && secondary
          ? h(
              'button',
              {
                type: 'button',
                title: 'Use the latest result for the selected B source and batch.',
                onClick: () => props.send({ type: 'select-comparison', selection: secondary }),
              },
              'Replace B',
            )
          : null,
        h('span', 'B'),
        h(
          'span',
          { class: 'sc-output-secondary-status' },
          props.snapshot.secondaryResource?.message ?? '',
        ),
        props.snapshot.secondaryResource?.canRetry && props.capabilities.commands.includes('retry')
          ? h('button', { type: 'button', onClick: () => props.send({ type: 'retry' }) }, 'Retry B')
          : null,
        props.snapshot.secondaryResource?.canAdmitLargeImage &&
        props.capabilities.commands.includes('admit-large-image')
          ? h(
              'button',
              { type: 'button', onClick: () => props.send({ type: 'admit-large-image' }) },
              'Load large pair',
            )
          : null,
        props.snapshot.secondary &&
        props.snapshot.secondary.mediaKind !== 'image' &&
        props.capabilities.commands.includes('open-media')
          ? h(
              'button',
              {
                type: 'button',
                onClick: () => props.send({ type: 'open-media', target: 'secondary' }),
              },
              'Open B media',
            )
          : null,
        props.capabilities.commands.includes('select-comparison')
          ? h(
              'select',
              {
                'aria-label': 'Comparison source',
                value: secondary?.sourceId ?? '',
                onChange: (event: Event) => {
                  const target = event.currentTarget;
                  if (!(target instanceof HTMLSelectElement)) return;
                  const chosen = props.snapshot.sources.find(
                    (candidate) => candidate.id === target.value,
                  );
                  if (chosen)
                    props.send({
                      type: 'select-comparison',
                      selection: { sourceId: chosen.id, batchIndex: secondary?.batchIndex ?? 0 },
                    });
                },
              },
              [
                h('option', { value: '', disabled: true }, 'Choose an output'),
                ...props.snapshot.sources.map((candidate) =>
                  h('option', { key: candidate.id, value: candidate.id }, candidate.label),
                ),
              ],
            )
          : h('span', source?.label ?? 'Unavailable output'),
        props.capabilities.commands.includes('select-comparison')
          ? h('input', {
              'aria-label': 'Comparison batch',
              type: 'number',
              min: 1,
              max: Math.max(1, source?.batchCount ?? 0, (secondary?.batchIndex ?? 0) + 1),
              value: (secondary?.batchIndex ?? 0) + 1,
              disabled: !secondary,
              onChange: (event: Event) => {
                const target = event.currentTarget;
                if (!(target instanceof HTMLInputElement)) return;
                const index = target.valueAsNumber - 1;
                if (secondary && Number.isSafeInteger(index) && index >= 0)
                  props.send({
                    type: 'select-comparison',
                    selection: { ...secondary, batchIndex: index },
                  });
              },
            })
          : null,
        props.capabilities.commands.includes('swap')
          ? h(
              'button',
              { type: 'button', disabled: !secondary, onClick: () => props.send({ type: 'swap' }) },
              'Swap A/B',
            )
          : null,
        props.capabilities.commands.includes('wipe')
          ? h(
              'button',
              {
                type: 'button',
                onClick: () =>
                  props.send({
                    type: 'wipe',
                    split: comparison.split,
                    orientation: comparison.orientation === 'vertical' ? 'horizontal' : 'vertical',
                  }),
              },
              comparison.orientation === 'vertical' ? 'Horizontal split' : 'Vertical split',
            )
          : null,
      ]);
    };
  },
});
