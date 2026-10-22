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
/** Feedback reads the exact accepted token from prepared semantics without grammar scans or same-kind fallbacks. */
import type { EditorState } from '@codemirror/state';
import { editorFreshness } from './freshness.js';
import { semanticStructures } from './structure.js';
import { weightSession, type WeightSession } from './weight-session.js';

/** Return current literal weight/group spelling for one accepted session; bounds use logical-document UTF-16. */
export function weightPreviewText(state: EditorState, session: WeightSession): string | null {
  const prepared = state.field(semanticStructures, false)?.native;
  if (
    state.field(weightSession, false) !== session ||
    !prepared ||
    prepared.sourceIdentity !== state.field(editorFreshness).source
  )
    return null;
  if (session.kind === 'emphasis' && session.neutral) return '1.00';
  let result: string | null = null;
  prepared.pointerTargets.between(session.range.from, session.range.from, (from, to, target) => {
    const content =
      target.kind === 'emphasis'
        ? { from: from + target.contentFrom, to: from + target.contentTo }
        : { from, to };
    if (
      target.kind !== session.kind ||
      content.from !== session.range.from ||
      content.to !== session.range.to
    )
      return;
    const scope = prepared.nativeValues.byId[target.scopeId]?.scope;
    if (!scope) return;
    result =
      session.kind === 'emphasis'
        ? (scope.document.emphasisSpans[target.ordinal]?.weight ?? null)
        : session.kind === 'lora'
          ? (scope.document.loraSpans[target.ordinal]?.firstWeight ?? null)
          : (scope.wildcardGroups[target.ordinal]?.displayTag ?? null);
  });
  return result;
}
