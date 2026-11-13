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
/** Canonical decoded weight commands delegate storage and native history to the genuine edit owner. */
import { requireValue } from '../../core/src/required.js';
import { prepareProjection } from './projection-preparation.js';
import { Transaction, type EditorState } from '@codemirror/state';
import {
  mutateEmphasisWeight,
  mutateLoraWeight,
  wrapPlainEmphasisWeight,
  type WeightChange,
} from '../../core/src/prompt/weight-mutations.js';
import { mutateWildcardTag, type SourceRange, type PromptDocument } from '../../core/src/index.js';
import { utf16ToCodePoint } from '../../core/src/offsets.js';
import { weightEditingAllowed, type WeightSubject } from './weight-target.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import {
  nativeDocumentValueAt,
  type NativeDocumentValueContext,
  type NativeDocumentValueTarget,
} from './document-values.js';
import {
  prepareDocumentValueEdit,
  currentDocumentValueEdit,
} from './document-value-edit-issuance.js';
import { installSemanticStructure } from './structure.js';
import { installProjection } from './rich-projection.js';
import { weightSession, installWeightSession, retireWeightSession } from './weight-session.js';
import { semanticStructures, prepareNativeStructure } from './structure.js';
/** One decoded semantic target retains the native boundary preference. */
interface WeightTarget {
  readonly kind: WeightSubject['kind'];
  readonly range: SourceRange;
  readonly boundary: 'start' | 'end';
}
/** Spatial logical CM bounds retain current native source/context authority. */
export type SpatialWeightRequest = {
  readonly freshness: EditorFreshness;
} & WeightSubject &
  (
    | {
        readonly owner: 'pointer';
      }
    | {
        readonly owner: 'wheel';
        readonly token: symbol;
      }
    | {
        readonly owner: 'exact';
        readonly neutral: boolean;
        readonly completion: 'keyboard' | 'outside' | 'space';
      }
  );
/** Keyboard resolves current prepared semantics; spatial intent supplies a captured native range. */
export type WeightRequest =
  | {
      readonly owner: 'keyboard';
    }
  | SpatialWeightRequest;
/** Resolve only grammar inside the actual admitted decoded scope. */
function targetFor(state: EditorState, admitted: NativeDocumentValueTarget): WeightTarget | null {
  const document = admitted.scope.document,
    selection = state.selection.main,
    session = state.field(weightSession),
    range = admitted.decodedRange;
  if (session?.owner === 'keyboard')
    return { kind: session.kind, range, boundary: session.boundary };
  if (!selection.empty)
    return {
      kind: 'emphasis',
      range,
      boundary: selection.head === selection.from ? 'start' : 'end',
    };
  const position = range.from;
  const wildcard = document.wildcardSpans.find(
    (span) => span.outerRange.from < position && position <= span.outerRange.to,
  );
  if (wildcard) return { kind: 'wildcard', range: wildcard.outerRange, boundary: 'end' };
  const reversed = [...document.emphasisSpans].reverse();
  const emphasis =
    reversed.find(
      (span) => span.contentRange.from <= position && position <= span.contentRange.to,
    ) ?? reversed.find((span) => span.outerRange.from < position && position <= span.outerRange.to);
  if (emphasis)
    return {
      kind: 'emphasis',
      range: emphasis.contentRange,
      boundary:
        utf16ToCodePoint(document.sourceText, position) <
        (utf16ToCodePoint(document.sourceText, emphasis.contentRange.from) +
          utf16ToCodePoint(document.sourceText, emphasis.contentRange.to)) /
          2
          ? 'start'
          : 'end',
    };
  return null;
}
/** Map a native word through the same admitted scope rather than treating source storage as grammar. */
function wordTarget(
  state: EditorState,
  context: NativeDocumentValueContext,
): NativeDocumentValueTarget | null {
  const word = state.wordAt(state.selection.main.head);
  if (!word) return null;
  const result = nativeDocumentValueAt(state, context, word);
  return result.status === 'ready' ? result.target : null;
}
/** Only genuine current native slots admit decoded mutation and source-safe seeded publication. */
export function prepareWeightChange(
  state: EditorState,
  change: WeightChange,
  request: WeightRequest = { owner: 'keyboard' },
): readonly Transaction[] {
  if (!weightEditingAllowed(state, request.owner === 'keyboard' ? undefined : request.kind))
    return [];
  if (request.owner !== 'keyboard' && request.freshness !== state.field(editorFreshness)) return [];
  const prepared = state.field(semanticStructures).native;
  if (!prepared || prepareNativeStructure(state) !== prepared) return [];
  const context: NativeDocumentValueContext = {
    values: prepared.nativeValues,
    semantics: prepared.documentSemantics,
    enabled: true,
    composing: false,
  };
  const session = state.field(weightSession);
  const selected =
    request.owner === 'keyboard'
      ? session?.owner === 'keyboard'
        ? session.range
        : state.selection.main
      : request.range;
  const admission = nativeDocumentValueAt(state, context, selected);
  if (admission.status !== 'ready') return [];
  let admitted = admission.target;
  let target =
    request.owner === 'keyboard'
      ? targetFor(state, admitted)
      : {
          kind: request.kind,
          range: admitted.decodedRange,
          boundary: 'end' as const,
        };
  if (!target && request.owner === 'keyboard') {
    const word = wordTarget(state, context);
    if (!word) return [];
    admitted = word;
    target = { kind: 'emphasis', range: word.decodedRange, boundary: 'end' };
  }
  if (!target || !weightEditingAllowed(state, target.kind)) return [];
  const document: PromptDocument = admitted.scope.document;
  const continuing =
    session?.owner === request.owner &&
    session.kind === target.kind &&
    (request.owner !== 'wheel' || session.token === request.token) &&
    (request.owner === 'keyboard' ||
      (session.range.from === request.range.from && session.range.to === request.range.to));
  const neutralTarget =
    target.kind === 'emphasis' &&
    (request.owner === 'exact' ? request.neutral : continuing && session.neutral);
  const emphasis = document.emphasisSpans.find(
    (span) =>
      span.contentRange.from === target.range.from && span.contentRange.to === target.range.to,
  );
  if (target.kind === 'emphasis' && request.owner !== 'keyboard' && !neutralTarget && !emphasis)
    return [];
  const outer = target.kind === 'emphasis' ? (emphasis?.outerRange ?? target.range) : target.range;
  const mutation =
    target.kind === 'wildcard'
      ? change.kind === 'step'
        ? mutateWildcardTag(document, target.range, change, admitted.scope.wildcardGroups)
        : null
      : target.kind === 'lora'
        ? mutateLoraWeight(document, target.range, change)
        : neutralTarget
          ? wrapPlainEmphasisWeight(document.sourceText, target.range, change)
          : mutateEmphasisWeight(document, target.range, change);
  if (!mutation) return [];
  const delta = mutation.text.length - document.sourceText.length;
  const lora =
    target.kind === 'lora'
      ? document.loraSpans.find(
          (span) =>
            span.outerRange.from === target.range.from && span.outerRange.to === target.range.to,
        )
      : undefined;
  const wildcard =
    target.kind === 'wildcard'
      ? document.wildcardSpans.find(
          (span) =>
            span.outerRange.from === target.range.from && span.outerRange.to === target.range.to,
        )
      : undefined;
  const replacement = lora?.firstWeightRange ?? wildcard?.contentRange ?? outer;
  const contentRange =
    target.kind === 'emphasis'
      ? mutation.selectionRange
      : { from: target.range.from, to: target.range.to + delta };
  const caret =
    request.owner === 'exact'
      ? (neutralTarget ? target.range.to : outer.to) + delta
      : target.boundary === 'start'
        ? contentRange.from
        : contentRange.to;
  /** Preserve decoded intent inside this value; actual adapter ChangeDesc owns every wholly outside range. */
  const decodedPosition = (logical: number): number | null => {
    const map = requireValue(admitted.entry.semanticMapping, 'weight-command');
    const decoded = map.logicalToDecoded[logical - map.logicalRange.from];
    if (decoded == null) return null;
    if (target.kind === 'emphasis' && target.range.from <= decoded && decoded <= target.range.to)
      return (
        contentRange.from +
        Math.min(decoded - target.range.from, contentRange.to - contentRange.from)
      );
    return decoded <= replacement.from
      ? decoded
      : decoded >= replacement.to
        ? decoded + delta
        : replacement.to + delta;
  };
  // Spatial edits and an exact edit finished by clicking elsewhere keep the user's own selection.
  const keepsSelection =
    request.owner === 'pointer' ||
    request.owner === 'wheel' ||
    (request.owner === 'exact' && request.completion === 'outside');
  const ranges = keepsSelection
    ? state.selection.ranges.map((range, index) => {
        if (
          range.to < admitted.entry.logicalStorage.from ||
          range.from > admitted.entry.logicalStorage.to
        )
          return { kind: 'outside' as const, index };
        const anchor = decodedPosition(range.anchor),
          head = decodedPosition(range.head);
        return anchor === null || head === null ? null : { kind: 'decoded' as const, anchor, head };
      })
    : [{ kind: 'decoded' as const, anchor: caret, head: caret }];
  if (ranges.some((range) => range === null)) return [];
  const outcome = prepareDocumentValueEdit(state, {
    publication: { purpose: 'weight' },
    context,
    target: admitted,
    range: replacement,
    text: mutation.text.slice(replacement.from, replacement.to + delta),
    selection: {
      ranges: ranges.filter((range) => range !== null),
      mainIndex: keepsSelection ? state.selection.mainIndex : 0,
    },
  });
  if (
    outcome.status === 'rejected' ||
    currentDocumentValueEdit(state, context, outcome.edit) !== null
  )
    return [];
  const edit = outcome.edit;
  if (outcome.status === 'unchanged') {
    if (request.owner !== 'exact') return [];
    const next = edit.transactions.at(-1)?.state ?? state;
    return [
      ...edit.transactions,
      next.update({
        selection: next.selection,
        effects: retireWeightSession.of(null),
        annotations: Transaction.addToHistory.of(false),
      }),
    ];
  }
  const changed = requireValue(edit.transactions[0], 'weight-command');
  const padding = admitted.scope.container.valueRange.from;
  const post = requireValue(
    requireValue(edit.postValues.byId[admitted.scope.id], 'weight-command').fullMapping,
    'weight-command',
  );
  /** Consume the canonical post-container map, including unsafe scalar and encoding gaps. */
  const position = (decoded: number): number => {
    const logical = post.decodedToLogical[padding + decoded];
    if (logical == null) throw new Error('weight-post-endpoint-unmapped');
    return logical;
  };
  const range = {
    from: position(contentRange.from),
    to: position(contentRange.to),
  };
  const semantic = prepareNativeStructure(changed.state, edit);
  const projection = prepareProjection(changed.state, semantic);
  if (currentDocumentValueEdit(state, context, edit) !== null) return [];
  const rendered = changed.state.update({
    selection: changed.state.selection,
    effects: [
      installSemanticStructure.of({ purpose: 'native', prepared: semantic }),
      installProjection.of(projection),
      installWeightSession.of({
        ...(request.owner === 'wheel'
          ? { owner: request.owner, token: request.token }
          : { owner: request.owner === 'exact' ? 'pointer' : request.owner }),
        freshness: changed.state.field(editorFreshness),
        kind: target.kind,
        range,
        boundary: target.boundary,
        neutral: 'neutral' in mutation && mutation.neutral === true,
      }),
    ],
    userEvent: 'select.weight',
  });
  return [changed, rendered];
}
