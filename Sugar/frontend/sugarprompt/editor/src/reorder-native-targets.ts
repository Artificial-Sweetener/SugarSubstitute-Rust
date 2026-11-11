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
/** Source-prepared native empty target/line identities use existing Core layout and finite supported Widget ownership. */
import { requireValue } from '../../core/src/required.js';
import type { Range } from '@codemirror/state';
import { Decoration, WidgetType, type EditorView } from '@codemirror/view';
import {
  emptyValueReorderPartitions,
  valueReorderLayout,
  type PreparedValueReorder,
  type ReorderOutput,
} from '../../core/src/index.js';
import type { NativeValueMapping } from './document-values.js';
/** Native and displaced empty targets share one zero-flow anchor and independently measured visible affordance. */
export function createEmptyReorderTarget(document: Document): {
  readonly root: HTMLElement;
  readonly target: HTMLElement;
} {
  const root = document.createElement('span'),
    target = document.createElement('span');
  root.className = 'sp-reorder-native-empty';
  root.textContent = '\u200b';
  target.className = 'sp-reorder-empty';
  root.append(target);
  return { root, target };
}
/** Empty targets retain a zero-width native position; their visible affordance paints outside layout. */
class EmptyReorderTarget extends WidgetType {
  readonly attributes: Readonly<Record<string, string>>;
  /** Retain only the actual prepared target identity. */
  constructor(attributes: Readonly<Record<string, string>>) {
    super();
    this.attributes = attributes;
  }
  /** Equivalent targets retain their finite native node. */
  override eq(other: EmptyReorderTarget): boolean {
    return JSON.stringify(this.attributes) === JSON.stringify(other.attributes);
  }
  /** Mount a literal noneditable zero-width target at its genuine native boundary. */
  toDOM(view: EditorView): HTMLElement {
    const { root: node, target } = createEmptyReorderTarget(view.dom.ownerDocument);
    const key = this.attributes['data-reorder-native-value'];
    if (key === undefined) throw new Error('native-reorder-target-value-unavailable');
    target.style.display = 'var(--sp-reorder-native-' + key + '-target-display,none)';
    target.style.visibility = 'var(--sp-reorder-native-' + key + '-target-visibility,hidden)';
    target.style.pointerEvents = 'var(--sp-reorder-native-' + key + '-target-events,none)';
    for (const [name, value] of Object.entries(this.attributes)) target.setAttribute(name, value);
    return node;
  }
  /** Native pointer routing remains available to the sole session owner. */
  override ignoreEvent(): boolean {
    return false;
  }
}
/** Prepare the actual initial Core empty targets at existing mapped native UTF-16 positions before any gesture. */
export function prepareNativeReorderTargets(
  prepared: PreparedValueReorder,
  output: ReorderOutput,
  mapping: NativeValueMapping,
  key: number,
): readonly Range<Decoration>[] {
  const order = prepared.initial,
    ranges: Range<Decoration>[] = [],
    layout = valueReorderLayout(order);
  /** One original serialized carrier retains its exact Core-owned zero-width target offsets. */
  const carrierStart = (owner: 'prefix' | 'slot' | 'suffix', slot: number): number =>
    owner === 'prefix'
      ? 0
      : owner === 'suffix'
        ? output.text.length - order.suffix.text.length
        : requireValue(
            output.renderedRanges[requireValue(order.order[slot], 'reorder-native-targets')],
            'reorder-native-targets',
          ).to;
  /** Source-only widget metadata stays immutable; root-issued variables alone activate its hit/paint affordance. */
  const target = (position: number, attributes: Readonly<Record<string, string>>): void => {
    const logical = mapping.decodedToLogical[position];
    if (logical == null) throw new Error('reorder-native-target-unmapped-endpoint');
    ranges.push(
      Decoration.widget({
        widget: new EmptyReorderTarget({
          ...attributes,
          'data-reorder-native-value': String(key),
        }),
        side: 1,
        nativeReorderValue: String(key),
      }).range(logical),
    );
  };
  for (const gap of layout.gaps)
    for (const [blank, offset] of gap.blankOffsets.entries())
      target(carrierStart(gap.carrier, gap.slot) + offset, {
        'data-reorder-gap': String(gap.index),
        'data-reorder-blank': String(blank),
      });
  for (const partition of emptyValueReorderPartitions(prepared, order))
    target(carrierStart(partition.carrier, partition.slot) + partition.offset, {
      'data-reorder-partition': String(partition.partition),
    });
  return ranges;
}
/** Shared physical CM lines may carry several independent prepared value roles; the root selects exactly one native line class. */
export function nativeReorderLineClass(value: string): string {
  if (!/^(0|[1-9][0-9]*)$/u.test(value)) throw new Error('native-reorder-line-value-invalid');
  return 'sp-reorder-native-line-' + value;
}
/** Read only owner-issued mark/widget target attributes; carrier marks have no semantic target identity. */
export function nativeReorderTargetAttributes(
  decoration: Decoration,
): Readonly<Record<string, string>> | null {
  if (decoration.spec.widget instanceof EmptyReorderTarget)
    return decoration.spec.widget.attributes;
  const attributes: unknown = decoration.spec.attributes;
  if (
    !attributes ||
    typeof attributes !== 'object' ||
    !('data-reorder-chip' in attributes) ||
    typeof attributes['data-reorder-chip'] !== 'string'
  )
    return null;
  const key =
    'data-reorder-native-value' in attributes ? attributes['data-reorder-native-value'] : null;
  if (typeof key !== 'string') return null;
  return {
    'data-reorder-chip': attributes['data-reorder-chip'],
    'data-reorder-native-value': key,
  };
}
