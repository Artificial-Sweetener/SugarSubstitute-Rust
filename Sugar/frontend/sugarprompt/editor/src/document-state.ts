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
/** Immutable document fields are captured independently of EditorState configuration and derived resources. */
import type { EditorSelection, EditorState, Extension, StateField, Text } from '@codemirror/state';
import { historyField } from '@codemirror/commands';
import { decodeSource, type LineEnding } from '../../core/src/line-endings.js';
import { lineEndings } from './exact-source.js';
import { authorship } from './authorship.js';
import type { Authorship } from './authorship-ranges.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';

/** Native fields retain their actual opaque history and immutable text; no EditorState is retained. */
export interface NativeDocumentSeed {
  readonly doc: Text;
  readonly selection: EditorSelection;
  readonly endings: readonly LineEnding[];
  readonly history: typeof historyField extends StateField<infer Value> ? Value : never;
  readonly authorship: Authorship;
}

/** Capture only CodeMirror document, selection, history and exact-source authoring metadata. */
export function nativeDocumentSeed(state: EditorState): NativeDocumentSeed {
  return Object.freeze({
    doc: state.doc,
    selection: state.selection,
    endings: state.field(lineEndings),
    history: state.field(historyField),
    authorship: state.field(authorship),
  });
}

/** Borrow a derived exact-source string without retaining a competing mutable text buffer. */
export function documentSeedSource(seed: NativeDocumentSeed): string {
  return decodeSource(seed.doc.toString(), seed.endings);
}

/** Supported field initialization adopts the retained native history effects under fresh current configuration. */
export function documentSeedExtensions(seed: NativeDocumentSeed): readonly Extension[] {
  return [
    lineEndings.init(() => seed.endings),
    historyField.init(() => seed.history),
    authorship.init(() => seed.authorship),
  ];
}

/** Capture admission binds immutable document fields and current command epochs, excluding derived publication. */
export interface NativeDocumentAuthority {
  readonly document: NativeDocumentSeed;
  readonly freshness: EditorFreshness | undefined;
}

/** Borrow current native document authority without retaining EditorState or configuration resources. */
export function nativeDocumentAuthority(state: EditorState): NativeDocumentAuthority {
  return Object.freeze({
    document: nativeDocumentSeed(state),
    freshness: state.field(editorFreshness, false),
  });
}

/** Derived semantic/rendering transactions preserve every admitted field and command-context epoch. */
export function documentAuthorityMatches(
  state: EditorState,
  captured: NativeDocumentAuthority,
): boolean {
  const current = nativeDocumentAuthority(state),
    before = captured.document,
    after = current.document;
  return (
    current.freshness === captured.freshness &&
    after.doc === before.doc &&
    after.selection === before.selection &&
    after.endings === before.endings &&
    after.history === before.history &&
    after.authorship === before.authorship
  );
}
