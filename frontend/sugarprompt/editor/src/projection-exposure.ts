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
/** Derived syntax exposure owns ordinary selection reveal and one finite native held-token visibility reservation. */
import { requireValue } from '../../core/src/required.js';
import { StateEffect, StateField, type EditorState } from '@codemirror/state';
import type { SourceRange } from '../../core/src/offsets.js';
import type { PreparedProjection } from './projection-contracts.js';
import { SourceToken, SourceWidget } from './projection-widgets.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { richRendering } from './presentation.js';
import { loraPresentation } from './lora-presentation.js';
import { loraMedia } from './lora-media-state.js';
import { wildcardCatalog } from './wildcard-catalog.js';
import { sceneErrors } from './scene-errors.js';
/** A reservation retains logical-document UTF-16 bounds and source/context authority, never selection or editable source. */
interface HeldProjection {
  readonly session: symbol;
  readonly freshness: EditorFreshness;
  readonly range: SourceRange;
  /** Canonical token capability separates pointer geometry from concealed-marker rendering. */
  readonly exposesSyntax: boolean;
}
/** Admission checks a current prepared token before retaining only logical lifetime metadata. */
interface HeldProjectionAdmission extends Omit<HeldProjection, 'exposesSyntax'> {
  readonly prepared: PreparedProjection;
}
/** Admit only the current prepared token captured by the supported native mouse-style factory. */
export const holdProjectionToken = StateEffect.define<HeldProjectionAdmission>();
/** A matching passive native termination releases only its own visibility reservation. */
export const releaseProjectionToken = StateEffect.define<symbol>();
/** Native view disposal retires prepared rendering and held visibility together. */
export const retireRichProjection = StateEffect.define<null>();
/** Real source/context or rich-mode replacement retires the logical hold independently of derived painting. */
function current(held: Pick<HeldProjection, 'freshness'>, state: EditorState): boolean {
  const freshness = state.field(editorFreshness);
  return (
    held.freshness.source === freshness.source &&
    held.freshness.context === freshness.context &&
    state.facet(richRendering)
  );
}
/** Admission validates every prepared rendering input once; subsequent painting cannot become source authority. */
function currentAdmission(candidate: HeldProjectionAdmission, state: EditorState): boolean {
  return (
    current(candidate, state) &&
    candidate.prepared.sourceIdentity === state.field(editorFreshness).source &&
    candidate.prepared.loraPresentation === state.field(loraPresentation) &&
    candidate.prepared.loraMedia === state.field(loraMedia) &&
    candidate.prepared.wildcardCatalog === state.facet(wildcardCatalog) &&
    candidate.prepared.sceneErrors === state.field(sceneErrors)
  );
}
/** The projection owner keeps at most one source/context-bound held token, independently of ordinary reveal/history metadata. */
export const heldProjection = StateField.define<HeldProjection | null>({
  /** Mounted rendering begins without a pointer reservation. */
  create() {
    return null;
  },
  /** Validate current token admission and retire source/context, explicit selection replacement or native lifetime boundaries. */
  update(held, transaction) {
    if (
      held &&
      (!current(held, transaction.state) ||
        (transaction.selection && !transaction.isUserEvent('select.pointer')))
    )
      held = null;
    for (const effect of transaction.effects) {
      if (effect.is(retireRichProjection)) held = null;
      else if (effect.is(releaseProjectionToken) && held?.session === effect.value) held = null;
      else if (
        effect.is(holdProjectionToken) &&
        currentAdmission(effect.value, transaction.state)
      ) {
        const candidate = effect.value;
        candidate.prepared.tokens.between(
          candidate.range.from,
          candidate.range.to,
          (from, to, token) => {
            if (from === candidate.range.from && to === candidate.range.to)
              held = {
                session: candidate.session,
                freshness: candidate.freshness,
                range: candidate.range,
                exposesSyntax: token.exposesSyntaxOnHold,
              };
          },
        );
      }
    }
    return held;
  },
});
/** Expose exact syntax around a selection inside a concealed wrapper; ordinary content reuses prepared output. */
export function exposeSelectedSyntax(
  prepared: PreparedProjection,
  state: EditorState,
): PreparedProjection {
  const positions = state.selection.ranges.flatMap((range) => [range.anchor, range.head]);
  const expanded = new Set<SourceToken>();
  const bounds = new Map<SourceToken, SourceRange>();
  if (prepared.revealed)
    prepared.tokens.between(prepared.revealed.from, prepared.revealed.to, (from, to, token) => {
      if (
        requireValue(prepared.revealed, 'projection-exposure').from <= from &&
        to <= requireValue(prepared.revealed, 'projection-exposure').to
      ) {
        expanded.add(token);
        bounds.set(token, { from, to });
      }
    });
  const held = state.field(heldProjection, false);
  if (held?.exposesSyntax)
    prepared.tokens.between(held.range.from, held.range.to, (from, to, token) => {
      if (from === held.range.from && to === held.range.to) {
        expanded.add(token);
        bounds.set(token, { from, to });
      }
    });
  for (const position of positions)
    prepared.all.between(position, position, (from, to, value) => {
      const widget = value.spec.widget;
      if (
        from < position &&
        position < to &&
        widget instanceof SourceWidget &&
        widget.revealOnInteriorSelection
      )
        expanded.add(widget.token);
    });
  if (
    expanded.size === prepared.expanded.length &&
    prepared.expanded.every((token) => expanded.has(token))
  )
    return prepared;
  for (const position of positions)
    prepared.tokens.between(position, position, (from, to, token) => {
      if (expanded.has(token)) bounds.set(token, { from, to });
    });
  let decorations = prepared.all,
    atomic = prepared.allAtomic;
  for (const [token, range] of bounds) {
    decorations = decorations.update({
      filterFrom: range.from,
      filterTo: range.to,
      filter: (_from, _to, value) =>
        !(value.spec.widget instanceof SourceWidget && value.spec.widget.token === token),
    });
    atomic = atomic.update({
      filterFrom: range.from,
      filterTo: range.to,
      filter: (_from, _to, value) => value.spec.widget.token !== token,
    });
  }
  return { ...prepared, decorations, atomic, expanded: [...expanded] };
}
