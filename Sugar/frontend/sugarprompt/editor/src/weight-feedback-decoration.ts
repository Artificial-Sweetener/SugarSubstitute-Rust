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
/** One prepared widget transform serves canonical projection and finite neutral-emphasis chrome. */
import { Decoration, type DecorationSet } from '@codemirror/view';
import type { Range } from '@codemirror/state';
import type { WeightSubject } from './weight-target.js';
import { EmphasisWidget, type EmphasisFeedback } from './projection-widgets.js';
import { LoraWidget } from './lora-widget.js';
import { WildcardWidget } from './wildcard-widget.js';

/** Exact kind and logical-document UTF16 bounds distinguish nested numeric subjects. */
export function sameWeightSubject(first: WeightSubject, second: WeightSubject): boolean {
  return (
    first.kind === second.kind &&
    first.range.from === second.range.from &&
    first.range.to === second.range.to
  );
}

/** Two boundary queries classify actual canonical widgets; no interior token scan, grammar, geometry or DOM mutation occurs. */
export function decorateWeightFeedback(
  decorations: DecorationSet,
  subject: WeightSubject,
  wheel: boolean,
  adjusting: boolean,
): DecorationSet {
  const replacements: {
    readonly before: Range<Decoration>;
    readonly after: Range<Decoration>;
  }[] = [];
  const adjustment = adjusting ? 'pointer' : wheel ? 'wheel' : 'none';
  const emphasisFeedback: EmphasisFeedback = adjusting ? (wheel ? 3 : 2) : wheel ? 1 : 0;
  const { range, kind } = subject;
  /** Boundary membership is the actual prepared widget role, never incidental parent DOM ancestry. */
  const observe = (from: number, to: number, value: Decoration): void => {
    if (
      replacements.some(
        (item) => item.before.from === from && item.before.to === to && item.before.value === value,
      )
    )
      return;
    const widget = value.spec.widget;
    let replacement: EmphasisWidget | LoraWidget | WildcardWidget | null = null;
    if (
      kind === 'emphasis' &&
      widget instanceof EmphasisWidget &&
      (widget.part === 'prefix' ? to === range.from : from === range.to) &&
      widget.feedback !== emphasisFeedback
    )
      replacement = new EmphasisWidget(
        widget.token,
        widget.part,
        widget.weight,
        widget.synthetic,
        widget.draftKey,
        emphasisFeedback,
      );
    else if (
      kind === 'lora' &&
      widget instanceof LoraWidget &&
      from === range.from &&
      to === range.to &&
      widget.adjustment !== adjustment
    )
      replacement = new LoraWidget(
        widget.token,
        widget.resource,
        widget.weight,
        adjustment,
        widget.preparedMedia,
        widget.draftKey,
      );
    else if (
      kind === 'wildcard' &&
      widget instanceof WildcardWidget &&
      (widget.part === 'prefix' ? from === range.from : to === range.to) &&
      widget.adjustment !== adjustment
    )
      replacement = new WildcardWidget(
        widget.token,
        widget.part,
        widget.group,
        widget.resolution,
        adjustment,
      );
    if (replacement) {
      const after = (
        from === to
          ? Decoration.widget({ ...value.spec, widget: replacement })
          : Decoration.replace({ ...value.spec, widget: replacement })
      ).range(from, to);
      replacements.push({ before: value.range(from, to), after });
    }
  };
  decorations.between(range.from, range.from, observe);
  if (range.to !== range.from) decorations.between(range.to, range.to, observe);
  for (const { before, after } of replacements)
    decorations = decorations.update({
      filterFrom: before.from,
      filterTo: before.to,
      filter: (from, to, value) =>
        !(from === before.from && to === before.to && value === before.value),
      add: [after],
    });
  return decorations;
}
