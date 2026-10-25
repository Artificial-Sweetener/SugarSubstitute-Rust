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
/** Prepared CodeMirror rich projection; editable content remains native source text. */
import { requireValue } from '../../core/src/required.js';
import {
  StateEffect,
  StateField,
  type EditorState,
  type Extension,
  type Transaction,
} from '@codemirror/state';
import { invertedEffects } from '@codemirror/commands';
import { EditorView } from '@codemirror/view';
import type { SourceRange } from '../../core/src/offsets.js';
import { featureEnabled } from '../../services/src/index.js';
import { changesExactSource } from './exact-source.js';
import { editorFreshness } from './freshness.js';
import { featureProfile } from './features.js';
import { richRendering } from './presentation.js';
import { emptyProjection, type PreparedProjection } from './projection-contracts.js';
import { loraPresentation } from './lora-presentation.js';
import { loraMedia } from './lora-media-state.js';
import {
  exposeSelectedSyntax,
  heldProjection,
  retireRichProjection,
} from './projection-exposure.js';
import { projectWeightFeedback } from './projection-feedback.js';
import { wildcardCatalog } from './wildcard-catalog.js';
import { projectWildcardDiagnosticVisibility } from './projection-diagnostics.js';
import { sceneErrors } from './scene-errors.js';
import { documentKind } from './document-kind.js';
import { projectWeightInput } from './exact-weight-projection.js';
/** Publish only prepared output, never parser work in a transaction or paint callback. */
export const installProjection = StateEffect.define<PreparedProjection>();
/** Publish one current projection for source input; native measurement still owns painting. */
export const installUnmeasuredProjection = StateEffect.define<PreparedProjection>();
/** All existing projection dependencies bind unmeasured output to actual current source and presentation. */
function projectionCurrent(prepared: PreparedProjection, state: EditorState): boolean {
  return (
    projectsRichSyntax(state) &&
    prepared.sourceIdentity === state.field(editorFreshness).source &&
    prepared.documentKind === state.facet(documentKind) &&
    prepared.emphasisEnabled === featureEnabled(state.facet(featureProfile), 'emphasis') &&
    prepared.loraEnabled === featureEnabled(state.facet(featureProfile), 'lora_syntax') &&
    prepared.wildcardEnabled === featureEnabled(state.facet(featureProfile), 'wildcard_syntax') &&
    prepared.loraPresentation === state.field(loraPresentation) &&
    prepared.loraMedia === state.field(loraMedia) &&
    prepared.wildcardCatalog === state.facet(wildcardCatalog) &&
    prepared.sceneErrors === state.field(sceneErrors)
  );
}
/** Retain at most one actual current compilation between history settlement and native measurement; it supplies no DOM decorations. */
export const unmeasuredProjection = StateField.define<PreparedProjection | null>({
  /** Native creation discovers no source. */
  create: () => null,
  /** Source/configuration retirement releases output; measured publication transfers it to the paint owner. */
  update(prepared, transaction) {
    let next =
      transaction.startState.readOnly === transaction.state.readOnly &&
      transaction.startState.facet(featureProfile) === transaction.state.facet(featureProfile) &&
      prepared &&
      projectionCurrent(prepared, transaction.state)
        ? prepared
        : null;
    for (const effect of transaction.effects) {
      if (effect.is(retireRichProjection)) return null;
      if (
        effect.is(installUnmeasuredProjection) &&
        projectionCurrent(effect.value, transaction.state)
      )
        next = effect.value;
      if (effect.is(installProjection) && projectionCurrent(effect.value, transaction.state))
        next = null;
    }
    return next;
  },
});
/** Reuse the actual unmeasured compilation only while every projection dependency remains current. */
export function currentUnmeasuredProjection(state: EditorState): PreparedProjection | null {
  const prepared = state.field(unmeasuredProjection, false);
  return prepared && projectionCurrent(prepared, state) ? prepared : null;
}
/** Reuse one complete current compilation independently of transient source exposure. */
export function currentCanonicalProjection(state: EditorState): PreparedProjection | null {
  const painted = state.field(richProjection);
  return currentUnmeasuredProjection(state) ?? (projectionCurrent(painted, state) ? painted : null);
}
/** Selection/reveal visibility delegates to the existing projection owner; input never compiles syntax or installs paint. */
export function currentInputProjection(state: EditorState): PreparedProjection | null {
  const painted = state.field(richProjection);
  const prepared = currentCanonicalProjection(state);
  if (!prepared) return null;
  return exposeSelectedSyntax(
    prepared.revealed === painted.revealed &&
      prepared.retainRevealOpening === painted.retainRevealOpening
      ? prepared
      : {
          ...prepared,
          revealed: painted.revealed,
          retainRevealOpening: painted.retainRevealOpening,
        },
    state,
  );
}
/** Cold readonly source paint delegates native exposure while excluding transient numeric input mounts. */
export function currentSourceDisplayProjection(state: EditorState): PreparedProjection | null {
  const prepared = currentCanonicalProjection(state);
  if (!prepared) return null;
  const painted = state.field(richProjection);
  return exposeSelectedSyntax(
    {
      ...prepared,
      decorations: prepared.all,
      atomic: prepared.allAtomic,
      expanded: [],
      revealed: painted.revealed,
      retainRevealOpening: painted.retainRevealOpening,
      weightDraft: null,
    },
    state,
  );
}
/** A prepared reveal is bound to the same exact-source identity as the native projection. */
export const revealProjectionToken = StateEffect.define<{
  readonly sourceIdentity: symbol;
  readonly range: SourceRange;
}>();
/** Logical UTF-16 restored bounds use the preceding document; a current-document anchor supports native history rebasing. */
interface RevealRestoration {
  readonly range: SourceRange;
  readonly anchor: number;
  /** Restore the actual source-authoring endpoint policy together with its native history range. */
  readonly retainOpening: boolean;
}
/** Native history restores only projection metadata, without retaining source or another history stack. */
const restoreRevealedRange = StateEffect.define<RevealRestoration | null>({
  /** Deleted source bounds can exceed the current document; map its surviving anchor through native history instead. */
  map(value, changes) {
    if (!value) return null;
    const anchor = changes.mapPos(value.anchor, -1),
      delta = anchor - value.anchor;
    return {
      anchor,
      range: { from: value.range.from + delta, to: value.range.to + delta },
      retainOpening: value.retainOpening,
    };
  },
});
/** Map authoring inside revealed source; host/source commands retire the transient editing intent. */
function mappedReveal(revealed: SourceRange | null, transaction: Transaction): SourceRange | null {
  if (!revealed || !changesExactSource(transaction)) return revealed;
  if (
    !transaction.isUserEvent('input.type') &&
    !transaction.isUserEvent('input.paste') &&
    !transaction.isUserEvent('delete')
  )
    return null;
  const from = transaction.changes.mapPos(revealed.from, -1),
    to = transaction.changes.mapPos(revealed.to, 1);
  return from < to ? { from, to } : null;
}
/** Raw presentation excludes all semantic widgets, independently of feature-specific permissions. */
export function projectsRichSyntax(state: EditorState): boolean {
  return state.facet(richRendering);
}
/** Empty rendering keeps current semantic/configuration identities and discovers no source during native lifetime reset. */
function initialProjection(state: EditorState): PreparedProjection {
  return emptyProjection(
    projectsRichSyntax(state) ? null : state.field(editorFreshness).source,
    featureEnabled(state.facet(featureProfile), 'emphasis'),
    featureEnabled(state.facet(featureProfile), 'lora_syntax'),
    featureEnabled(state.facet(featureProfile), 'wildcard_syntax'),
    state.field(loraPresentation),
    state.field(loraMedia),
    state.facet(wildcardCatalog),
    state.field(sceneErrors),
    state.facet(documentKind),
  );
}
/** Own current derived rendering and native atomic wrapper navigation. */
export const richProjection = StateField.define<PreparedProjection>({
  /** Initialize the intrinsic field before construction installs its complete source projection. */
  create: initialProjection,
  /** Map existing native ranges immediately and reject stale semantic publications. */
  update(prepared, transaction) {
    if (transaction.effects.some((effect) => effect.is(retireRichProjection)))
      return initialProjection(transaction.state);
    const identity = transaction.state.field(editorFreshness).source;
    const emphasisEnabled = featureEnabled(transaction.state.facet(featureProfile), 'emphasis');
    const loraEnabled = featureEnabled(transaction.state.facet(featureProfile), 'lora_syntax'),
      resources = transaction.state.field(loraPresentation),
      media = transaction.state.field(loraMedia);
    const wildcardEnabled = featureEnabled(
      transaction.state.facet(featureProfile),
      'wildcard_syntax',
    );
    const catalog = transaction.state.facet(wildcardCatalog);
    const errors = transaction.state.field(sceneErrors);
    const kind = transaction.state.facet(documentKind);
    if (!projectsRichSyntax(transaction.state)) {
      if (
        prepared.sourceIdentity === identity &&
        prepared.documentKind === kind &&
        prepared.emphasisEnabled === emphasisEnabled &&
        prepared.loraEnabled === loraEnabled &&
        prepared.wildcardEnabled === wildcardEnabled &&
        prepared.loraPresentation === resources &&
        prepared.loraMedia === media &&
        prepared.wildcardCatalog === catalog &&
        prepared.sceneErrors === errors &&
        prepared.all.size === 0
      )
        return prepared;
      return emptyProjection(
        identity,
        emphasisEnabled,
        loraEnabled,
        wildcardEnabled,
        resources,
        media,
        catalog,
        errors,
        kind,
      );
    }
    let next = prepared;
    let revealed = mappedReveal(prepared.revealed, transaction);
    let retainRevealOpening = prepared.retainRevealOpening;
    if (
      !projectsRichSyntax(transaction.startState) ||
      prepared.documentKind !== kind ||
      prepared.emphasisEnabled !== emphasisEnabled ||
      prepared.loraEnabled !== loraEnabled ||
      prepared.wildcardEnabled !== wildcardEnabled
    )
      next = emptyProjection(
        null,
        emphasisEnabled,
        loraEnabled,
        wildcardEnabled,
        resources,
        media,
        catalog,
        errors,
        kind,
      );
    if (next !== prepared) {
      revealed = null;
      retainRevealOpening = false;
    }
    if (next.wildcardCatalog !== catalog)
      next = { ...next, sourceIdentity: null, wildcardCatalog: catalog };
    if (next.sceneErrors !== errors) next = { ...next, sourceIdentity: null, sceneErrors: errors };
    if (next.loraPresentation !== resources || next.loraMedia !== media)
      next = {
        ...next,
        sourceIdentity: next.sourceIdentity === identity && !next.hasLoras ? identity : null,
        loraPresentation: resources,
        loraMedia: media,
      };
    if (changesExactSource(transaction)) {
      const all = next.all.map(transaction.changes);
      const atomic = next.allAtomic.map(transaction.changes);
      next = {
        ...next,
        sourceIdentity: null,
        emphasisEnabled,
        all,
        allAtomic: atomic,
        tokens: next.tokens.map(transaction.changes),
        wildcardDiagnostics: next.wildcardDiagnostics.map(transaction.changes),
        expanded: [],
        decorations: all,
        atomic,
        revealed,
        // Interior emphasis bounds include inserted body text; atomic outer bounds follow replacement exclusion.
        accented: next.accented.map((subject) => ({
          kind: subject.kind,
          range: {
            from: transaction.changes.mapPos(
              subject.range.from,
              subject.kind === 'emphasis' ? -1 : 1,
            ),
            to: transaction.changes.mapPos(subject.range.to, subject.kind === 'emphasis' ? 1 : -1),
          },
        })),
      };
    }
    for (const effect of transaction.effects)
      if (effect.is(installProjection) && projectionCurrent(effect.value, transaction.state))
        if (
          effect.value.sourceIdentity !== next.sourceIdentity ||
          effect.value.all !== next.all ||
          effect.value.tokens !== next.tokens
        )
          next =
            effect.value.revealed === revealed &&
            effect.value.retainRevealOpening === retainRevealOpening
              ? effect.value
              : { ...effect.value, revealed, retainRevealOpening };
    for (const effect of transaction.effects) {
      if (
        effect.is(revealProjectionToken) &&
        effect.value.sourceIdentity === identity &&
        (next.sourceIdentity === identity ||
          currentUnmeasuredProjection(transaction.state) !== null)
      ) {
        const range = effect.value.range;
        (currentUnmeasuredProjection(transaction.state) ?? next).tokens.between(
          range.from,
          range.to,
          (from, to) => {
            if (from === range.from && to === range.to) {
              revealed = range;
              retainRevealOpening = transaction.isUserEvent('select.structural');
            }
          },
        );
      } else if (
        effect.is(restoreRevealedRange) &&
        (transaction.isUserEvent('undo') || transaction.isUserEvent('redo')) &&
        (emphasisEnabled || loraEnabled || wildcardEnabled)
      ) {
        revealed = effect.value?.range ?? null;
        retainRevealOpening = effect.value?.retainOpening ?? false;
      }
    }
    const held = transaction.state.field(heldProjection);
    if (
      held?.exposesSyntax &&
      transaction.isUserEvent('select.pointer') &&
      transaction.state.selection.ranges.every(
        (range) => held.range.from <= range.from && range.to <= held.range.to,
      )
    ) {
      revealed = held.range;
      retainRevealOpening = false;
    }
    if (
      revealed &&
      (!transaction.state.selection.ranges.some((range) =>
        [range.anchor, range.head].some(
          (position) =>
            (requireValue(revealed, 'rich-projection').from < position ||
              (retainRevealOpening &&
                requireValue(revealed, 'rich-projection').from === position)) &&
            position <= requireValue(revealed, 'rich-projection').to,
        ),
      ) ||
        revealed.from < 0 ||
        revealed.from >= revealed.to ||
        revealed.to > transaction.newDoc.length)
    )
      revealed = null;
    if (!revealed) retainRevealOpening = false;
    if (next.revealed !== revealed || next.retainRevealOpening !== retainRevealOpening)
      next = { ...next, revealed, retainRevealOpening };
    return projectWeightInput(
      projectWeightFeedback(
        projectWildcardDiagnosticVisibility(
          exposeSelectedSyntax(next, transaction.state),
          transaction.state,
        ),
        transaction.state,
      ),
      transaction.state,
    );
  },
  /** Every projection configuration includes its intrinsic held-token exposure state. */
  provide: (field) => [
    heldProjection,
    EditorView.decorations.from(field, (value) => value.decorations),
    EditorView.atomicRanges.of((view) => view.state.field(field).atomic),
  ],
});
/** Required native rendering state and source mapping share one finite owner. */
export const richProjectionExtension: Extension = [
  unmeasuredProjection,
  richProjection,
  EditorView.editorAttributes.compute([richProjection, editorFreshness], (state) => ({
    'data-diagnostic-source':
      state.field(richProjection).sourceIdentity === state.field(editorFreshness).source
        ? 'ready'
        : 'pending',
  })),
  invertedEffects.of((transaction) => {
    if (!changesExactSource(transaction)) return [];
    const previous = transaction.startState.field(richProjection),
      current = transaction.state.field(richProjection),
      before = previous.revealed,
      after = current.revealed;
    return before === after && previous.retainRevealOpening === current.retainRevealOpening
      ? []
      : [
          restoreRevealedRange.of(
            before
              ? {
                  range: before,
                  anchor: transaction.changes.mapPos(before.from, -1),
                  retainOpening: previous.retainRevealOpening,
                }
              : null,
          ),
        ];
  }),
];
