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
/** Observe the authoritative native editor without retaining a second document or interaction state. */
import { undoDepth, redoDepth } from '@codemirror/commands';
import type { Text } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { decodeSource, documentToSource, type LineEnding } from '../../core/src/line-endings.js';
import type { EditorSnapshot } from './contracts.js';
import { lineEndings } from './exact-source.js';
import { editorFreshness } from './freshness.js';
import { richRendering } from './presentation.js';
import { richProjection } from './rich-projection.js';
import { semanticStructures } from './structure.js';
import { documentKind } from './document-kind.js';
import { nativeCompositionStarted } from './input-settlement.js';

/** Every source-free public value is prepared in this owner; future snapshot fields must be projected here. */
export type NativeSnapshotMetadata = Omit<EditorSnapshot, 'source' | 'anchor' | 'head'>;

/** Immutable canonical inputs and complete public metadata admit decoding without retaining another source string. */
export interface PreparedNativeSnapshot {
  readonly document: Text;
  readonly endings: readonly LineEnding[];
  readonly metadata: NativeSnapshotMetadata;
}

/** Observe all public values cheaply, retaining only immutable native document/spelling identities and scalar metadata. */
export function prepareNativeSnapshot(view: EditorView, revision: number): PreparedNativeSnapshot {
  const state = view.state,
    selection = state.selection.main,
    composing = nativeCompositionStarted(view);
  const metadata: NativeSnapshotMetadata = {
    documentKind: state.facet(documentKind),
    documentAnchor: selection.anchor,
    documentHead: selection.head,
    revision,
    composing,
    focused: view.hasFocus,
    readOnly: state.readOnly,
    undoDepth: undoDepth(state),
    redoDepth: redoDepth(state),
    richRendering: state.facet(richRendering),
    semantics: composing
      ? 'composing'
      : state.field(semanticStructures).native
        ? 'ready'
        : 'pending',
    rendering: !state.facet(richRendering)
      ? 'raw'
      : composing
        ? 'composing'
        : state.field(richProjection).sourceIdentity === state.field(editorFreshness).source
          ? 'ready'
          : 'pending',
  };
  return Object.freeze({
    document: state.doc,
    endings: state.field(lineEndings),
    metadata: Object.freeze(metadata),
  });
}

/** Compare complete prepared public inputs without decoding source or consuming publication eligibility. */
export function sameNativeSnapshot(
  left: PreparedNativeSnapshot,
  right: PreparedNativeSnapshot,
): boolean {
  return (
    left.document === right.document &&
    left.endings === right.endings &&
    (Object.keys(left.metadata) as (keyof NativeSnapshotMetadata)[]).every(
      (key) => left.metadata[key] === right.metadata[key],
    )
  );
}

/** Materialize a fresh public observation with exact-source UTF-16 selection; returned values never alias private metadata. */
export function materializeNativeSnapshot(prepared: PreparedNativeSnapshot): EditorSnapshot {
  const logical = prepared.document.toString(),
    endings = prepared.endings;
  return {
    ...prepared.metadata,
    source: decodeSource(logical, endings),
    anchor: documentToSource(logical, endings, prepared.metadata.documentAnchor),
    head: documentToSource(logical, endings, prepared.metadata.documentHead),
  };
}
