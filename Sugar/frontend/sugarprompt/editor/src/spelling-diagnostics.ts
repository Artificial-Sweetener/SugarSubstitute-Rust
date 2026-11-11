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
/** Genuine service findings project current canonical source into native marks; the host session owns word ignores. */
import { requireValue } from '../../core/src/required.js';
import {
  Facet,
  RangeSet,
  type Transaction,
  StateEffect,
  StateField,
  type EditorState,
} from '@codemirror/state';
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view';
import type { SpellcheckSession } from '../../services/src/diagnostics/spelling-session.js';
import type {
  PreparedSpellingSnapshot,
  PreparedSpellingCandidates,
  SpellingIssue,
  SpellingUnavailableCode,
} from '../../services/src/diagnostics/spelling-contracts.js';
import { isPreparedSpellingCandidates } from '../../services/src/diagnostics/spelling-preparation.js';
import { featureEnabled } from '../../services/src/features/profile.js';
import { sourcePositionsToDocument } from '../../core/src/line-endings.js';
import {
  diagnosticIsActive,
  isDiagnosticCommitBoundary,
} from '../../core/src/diagnostics/visibility.js';
import { lineEndings } from './exact-source.js';
import {
  SpellingPaint,
  spellingMark,
  mapSpellingPaint,
  spellingPaintVisibility,
  reconcileSpellingPaint,
  type SpellingPaintState,
} from './spelling-paint.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { featureProfile } from './features.js';
import { semanticStructures } from './structure.js';
/** One reconfigurable public host input supplies the shared service session; editor teardown never disposes it. */
export const spellingSession = Facet.define<SpellcheckSession | null, SpellcheckSession | null>({
  combine(values) {
    if (values.length > 1) throw new Error('Multiple spelling session owners');
    return values[0] ?? null;
  },
});
/** An actual service issue and its exact mapped logical-document UTF16 bounds form one immutable hit identity. */
export class SpellingDiagnosticValue extends SpellingPaint {
  readonly session: SpellcheckSession;
  readonly snapshot: PreparedSpellingSnapshot;
  readonly issue: SpellingIssue;
  readonly from: number;
  readonly to: number;
  /** Retain actual service membership, not a reconstructed spelling DTO. */
  constructor(
    session: SpellcheckSession,
    snapshot: PreparedSpellingSnapshot,
    issue: SpellingIssue,
    from: number,
    to: number,
    committedAtEnd: boolean,
  ) {
    super(issue.candidate.text, committedAtEnd);
    this.session = session;
    this.snapshot = snapshot;
    this.issue = issue;
    this.from = from;
    this.to = to;
    Object.freeze(this);
  }
}
/** One current source/session receipt owns only bounded derived marks and interval membership. */
export interface NativeSpellingDiagnostics extends SpellingPaintState {
  readonly status: 'pending' | 'ready' | 'unavailable' | 'failed';
  readonly freshness: EditorFreshness;
  readonly session: SpellcheckSession;
  readonly revision: number;
  readonly snapshot: PreparedSpellingSnapshot | null;
  readonly code: SpellingUnavailableCode | null;
  readonly findings: RangeSet<SpellingDiagnosticValue>;
  readonly decorations: DecorationSet;
  readonly lexicon: PreparedSpellingCandidates['lexicon'];
}
const issued = new WeakSet<NativeSpellingDiagnostics>();
/** The service authenticates its frozen complete snapshot through its first actual issue; indexed derived members need no repeated vector scan. */
function current(state: EditorState, value: NativeSpellingDiagnostics): boolean {
  return (
    state.field(editorFreshness) === value.freshness &&
    state.facet(spellingSession) === value.session &&
    value.session.revision === value.revision &&
    featureEnabled(state.facet(featureProfile), 'spellcheck') &&
    (!value.snapshot ||
      (state.field(semanticStructures).native?.scopes === value.snapshot.prepared.scopes &&
        value.session.current(
          value.snapshot,
          requireValue(value.snapshot.issues[0], 'spelling-diagnostics'),
        )))
  );
}
/** Off-input preparation maps already converted exact-storage ranges in one canonical newline conversion batch. */
export function prepareNativeSpellingDiagnostics(
  state: EditorState,
  session: SpellcheckSession,
  snapshot: PreparedSpellingSnapshot | null,
  status: NativeSpellingDiagnostics['status'] = 'ready',
  code: SpellingUnavailableCode | null = null,
  prepared?: PreparedSpellingCandidates,
): NativeSpellingDiagnostics | null {
  if (
    session !== state.facet(spellingSession) ||
    !featureEnabled(state.facet(featureProfile), 'spellcheck')
  )
    return null;
  if (
    snapshot &&
    (state.field(semanticStructures).native?.scopes !== snapshot.prepared.scopes ||
      snapshot.revision !== session.revision ||
      (snapshot.issues.length > 0 &&
        !session.current(snapshot, requireValue(snapshot.issues[0], 'spelling-diagnostics'))))
  )
    return null;
  if (
    prepared &&
    (!isPreparedSpellingCandidates(prepared) ||
      prepared.scopes !== state.field(semanticStructures).native?.scopes)
  )
    return null;
  const issues = snapshot?.issues ?? [],
    positions = sourcePositionsToDocument(
      state.doc.toString(),
      state.field(lineEndings),
      issues.flatMap((issue) => [issue.range.from, issue.range.to]),
    );
  const values = issues.map((issue, index) => {
    const next = issue.group.scope.mapping.text.codePointAt(issue.candidate.range.to);
    return new SpellingDiagnosticValue(
      session,
      requireValue(snapshot, 'spelling-diagnostics'),
      issue,
      requireValue(positions[index * 2], 'spelling-diagnostics'),
      requireValue(positions[index * 2 + 1], 'spelling-diagnostics'),
      isDiagnosticCommitBoundary(next === undefined ? '' : String.fromCodePoint(next)),
    );
  });
  const findings = RangeSet.of(
    values.map((value) => value.range(value.from, value.to)),
    true,
  );
  const previous = state.field(spellingDiagnostics, false);
  const continuation =
    status === 'pending' &&
    prepared &&
    previous &&
    current(state, previous) &&
    previous.lexicon === prepared.lexicon
      ? reconcileSpellingPaint(previous, state, prepared)
      : null;
  const value: NativeSpellingDiagnostics = Object.freeze({
    status,
    freshness: state.field(editorFreshness),
    session,
    revision: session.revision,
    snapshot: issues.length ? snapshot : null,
    code,
    findings,
    paint: continuation?.paint ?? findings,
    nativeBoundaries: continuation?.nativeBoundaries ?? false,
    decorations:
      continuation?.decorations ??
      Decoration.set(
        values.map((value) => spellingMark(value).range(value.from, value.to)),
        true,
      ),
    muted: continuation?.muted ?? null,
    activeRange: continuation?.activeRange ?? null,
    lexicon: prepared?.lexicon ?? snapshot?.prepared.lexicon ?? null,
  });
  issued.add(value);
  return value;
}
/** Native composition explicitly retires paint before any platform preedit mutation. */
export const retireSpellingPaint = StateEffect.define<null>();
/** Only unchanged native ranges may outlive source authority; context/session changes never carry paint. */
function continuePaint(
  value: NativeSpellingDiagnostics,
  transaction: Transaction,
): NativeSpellingDiagnostics | null {
  const state = transaction.state;
  if (
    !transaction.docChanged ||
    transaction.reconfigured ||
    value.freshness.context !== state.field(editorFreshness).context ||
    value.session !== state.facet(spellingSession) ||
    value.revision !== value.session.revision ||
    !featureEnabled(state.facet(featureProfile), 'spellcheck') ||
    transaction.isUserEvent('input.type.compose')
  )
    return null;
  const mapped = mapSpellingPaint(value, transaction);
  if (!mapped.paint.size) return null;
  return Object.freeze({
    ...value,
    ...mapped,
    status: 'pending',
    freshness: state.field(editorFreshness),
    snapshot: null,
    findings: RangeSet.empty,
    code: null,
  });
}
/** Only issued source/session-current preparations can replace the native diagnostic field. */
export const installSpellingDiagnostics = StateEffect.define<NativeSpellingDiagnostics>();
/** Current source/context/session changes immediately remove stale findings, independently of provider completion. */
export const spellingDiagnostics = StateField.define<NativeSpellingDiagnostics | null>({
  create: () => null,
  update(value, transaction) {
    if (
      transaction.effects.some((effect) => effect.is(retireSpellingPaint)) ||
      (transaction.reconfigured && value?.snapshot === null)
    )
      return null;
    if (value && !current(transaction.state, value)) value = continuePaint(value, transaction);
    for (const effect of transaction.effects)
      if (
        effect.is(installSpellingDiagnostics) &&
        issued.has(effect.value) &&
        current(transaction.state, effect.value)
      )
        value = effect.value;
    return value ? spellingPaintVisibility(value, transaction.state) : null;
  },
  provide: (field) =>
    EditorView.decorations.from(field, (value) => value?.decorations ?? Decoration.none),
});
/** Hit testing and command admission share actual indexed identity and service revision authority. */
export function currentSpellingDiagnostic(
  state: EditorState,
  value: SpellingDiagnosticValue,
): boolean {
  const actual = state.field(spellingDiagnostics);
  if (
    !actual ||
    !current(state, actual) ||
    value.session !== actual.session ||
    value.snapshot !== actual.snapshot
  )
    return false;
  let found = false;
  actual.findings.between(value.from, value.to, (_from, _to, item) => {
    if (item === value) found = true;
  });
  return found;
}
/** Active token visibility uses the sole Core rule, preserving all other source-backed marks. */
export function spellingDiagnosticAt(
  state: EditorState,
  position: number,
): SpellingDiagnosticValue | null {
  let result: SpellingDiagnosticValue | null = null;
  state
    .field(spellingDiagnostics, false)
    ?.findings.between(position, position, (from, to, item) => {
      if (
        currentSpellingDiagnostic(state, item) &&
        !diagnosticIsActive(from, to, state.selection.main.head, item.committedAtEnd)
      )
        result = item;
    });
  return result;
}
