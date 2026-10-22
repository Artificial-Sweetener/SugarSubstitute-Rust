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
/** Bounded native draft projection changes only the current prepared numeric widget, never source or geometry. */
import type { EditorState, Range } from '@codemirror/state';
import { Decoration, type DecorationSet } from '@codemirror/view';
import type { PreparedProjection } from './projection-contracts.js';
import { exactWeightDraft, type ExactWeightDraft } from './exact-weight-draft.js';
import { EmphasisWidget } from './projection-widgets.js';
import { LoraWidget } from './lora-widget.js';

/** One exact subject resolves its authored suffix or complete atomic LoRA interval, including neutral point widgets. */
export function decorateWeightInput(
  decorations: DecorationSet,
  target: ExactWeightDraft,
  key: symbol | null,
): DecorationSet {
  const replacements: Range<Decoration>[] = [];
  decorations.between(target.range.from, target.range.to, (from, to, value) => {
    const widget = value.spec.widget;
    let replacement: EmphasisWidget | LoraWidget | null = null;
    if (
      target.kind === 'emphasis' &&
      from === target.range.to &&
      widget instanceof EmphasisWidget &&
      widget.part === 'suffix' &&
      widget.draftKey !== key
    )
      replacement = new EmphasisWidget(
        widget.token,
        widget.part,
        widget.weight,
        widget.synthetic,
        key,
        widget.feedback,
      );
    else if (
      target.kind === 'lora' &&
      from === target.range.from &&
      to === target.range.to &&
      widget instanceof LoraWidget &&
      widget.draftKey !== key
    )
      replacement = new LoraWidget(
        widget.token,
        widget.resource,
        widget.weight,
        widget.adjustment,
        widget.preparedMedia,
        key,
      );
    if (replacement)
      replacements.push(
        (from === to
          ? Decoration.widget({ widget: replacement, side: 1 })
          : Decoration.replace({ widget: replacement })
        ).range(from, to),
      );
  });
  if (!replacements.length) return decorations;
  return decorations.update({
    filterFrom: target.range.from,
    filterTo: target.range.to,
    filter: (from, to) => !replacements.some((value) => value.from === from && value.to === to),
    add: replacements,
  });
}
/** Current/prior derived intent bounds retire and mount at most two numeric cells; canonical prepared widgets remain source-only. */
export function projectWeightInput(
  prepared: PreparedProjection,
  state: EditorState,
): PreparedProjection {
  const draft = state.field(exactWeightDraft, false) ?? null;
  let decorations = prepared.decorations;
  if (prepared.weightDraft && prepared.weightDraft !== draft)
    decorations = decorateWeightInput(decorations, prepared.weightDraft, null);
  if (draft) decorations = decorateWeightInput(decorations, draft, draft.key);
  return decorations === prepared.decorations && prepared.weightDraft === draft
    ? prepared
    : { ...prepared, decorations, weightDraft: draft };
}
