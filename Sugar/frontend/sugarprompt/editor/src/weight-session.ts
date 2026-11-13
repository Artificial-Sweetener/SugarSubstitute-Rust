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
/** CodeMirror owns one source-backed adjustment intent and its derived temporary presentation. */
import { StateEffect, StateField, type Transaction, type Range } from '@codemirror/state';
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { changesExactSource } from './exact-source.js';
import { richRendering } from './presentation.js';
import { exactWeightDraft } from './exact-weight-draft.js';
import { decorateWeightInput } from './exact-weight-projection.js';
import { EmphasisWidget, SourceToken } from './projection-widgets.js';
import type { WeightSubject } from './weight-target.js';
import { wheelFeedback } from './wheel-feedback.js';
import { decorateWeightFeedback, sameWeightSubject } from './weight-feedback-decoration.js';

/** Retained range intent uses logical-document UTF-16; boundary preference observes the native caret rather than replacing it. */
export interface WeightSession extends WeightSubject {
  readonly owner: 'keyboard' | 'pointer' | 'wheel' | 'exact';
  /** Opaque wheel gesture identity prevents a later hovered token from adopting an earlier queued result. */
  readonly token: symbol | null;
  readonly boundary: 'start' | 'end';
  readonly neutral: boolean;
  readonly decorations: DecorationSet;
}

/** A source-current publication binds native freshness to its resulting logical-document UTF-16 target range. */
export type WeightSessionPublication = {
  readonly freshness: EditorFreshness;
  readonly boundary: 'start' | 'end';
  readonly neutral: boolean;
} & WeightSubject &
  (
    | { readonly owner: 'keyboard' | 'pointer' | 'exact' }
    | { readonly owner: 'wheel'; readonly token: symbol }
  );

/** Only the finite command owner publishes new intent; other gestures explicitly retire it. */
export const installWeightSession = StateEffect.define<WeightSessionPublication>();
/** Native navigation, pointer and focus intent can retire a session without source/history changes. */
export const retireWeightSession = StateEffect.define<null>();

/** Build constant-size decoration output from already prepared range; no parsing or geometry occurs in the field. */
function publishedSession(
  value: WeightSessionPublication,
  transaction: Transaction,
): WeightSession {
  const range = Object.freeze({ ...value.range });
  const ranges: Range<Decoration>[] = [];
  if (transaction.state.facet(richRendering) && value.neutral) {
    const token = new SourceToken();
    ranges.push(
      Decoration.widget({
        widget: new EmphasisWidget(token, 'prefix', '', false, null, 2),
        side: -1,
      }).range(range.from),
      Decoration.widget({
        widget: new EmphasisWidget(token, 'suffix', '1.00', true, null, 2),
        side: 1,
      }).range(range.to),
    );
  }
  return Object.freeze({
    owner: value.owner,
    kind: value.kind,
    token: value.owner === 'wheel' ? value.token : null,
    range,
    boundary: value.boundary,
    neutral: value.neutral,
    decorations: Decoration.set(ranges, true),
  });
}

/** One native state field owns adjustment continuation and neutral chrome; source, selection and history stay native. */
export const weightSession = StateField.define<WeightSession | null>({
  /** Baselines and remounts never inherit an adjustment target. */
  create() {
    return null;
  },
  /** Unrelated changes retire intent; a source-current command alone can replace it with its exact resulting range. */
  update(current, transaction) {
    const before = transaction.startState.field(editorFreshness);
    const after = transaction.state.field(editorFreshness);
    if (
      before.context !== after.context ||
      transaction.startState.facet(richRendering) !== transaction.state.facet(richRendering)
    )
      return null;
    let next = changesExactSource(transaction) || transaction.selection ? null : current;
    for (const effect of transaction.effects) {
      if (effect.is(retireWeightSession)) next = null;
      if (
        effect.is(installWeightSession) &&
        effect.value.freshness.source === before.source &&
        effect.value.freshness.context === before.context &&
        !transaction.state.readOnly
      )
        next = publishedSession(effect.value, transaction);
    }
    return next;
  },
  provide: (field) => [
    exactWeightDraft,
    wheelFeedback,
    EditorView.decorations.compute([field, exactWeightDraft, wheelFeedback], (state) => {
      const session = state.field(field),
        draft = state.field(exactWeightDraft);
      if (!session?.neutral) return session?.decorations ?? Decoration.none;
      const wheel = state.field(wheelFeedback).target;
      const chrome = decorateWeightFeedback(
        session.decorations,
        session,
        !!wheel && sameWeightSubject(wheel, session),
        true,
      );
      return draft ? decorateWeightInput(chrome, draft, draft.key) : chrome;
    }),
  ],
});
