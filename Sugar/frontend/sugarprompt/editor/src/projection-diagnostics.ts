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
/** Native caret visibility derives from prepared findings without parsing, catalog work or whole-source traversal. */
import type { EditorState } from '@codemirror/state';
import { Decoration } from '@codemirror/view';
import { diagnosticIsActive } from '../../core/src/index.js';
import { editorFreshness } from './freshness.js';
import type { PreparedProjection } from './projection-contracts.js';
import type { WildcardDiagnosticValue } from './wildcard-diagnostic-value.js';

/** Inspect the indexed caret interval and at most two prior/current body marks; actual descriptors are reused when visibility remains unchanged. */
export function projectWildcardDiagnosticVisibility(
  prepared: PreparedProjection,
  state: EditorState,
): PreparedProjection {
  if (prepared.sourceIdentity !== state.field(editorFreshness).source) return prepared;
  const caret = state.selection.main.head;
  let muted: WildcardDiagnosticValue | null = null;
  prepared.wildcardDiagnostics.between(caret, caret, (from, to, value) => {
    if (diagnosticIsActive(from, to, caret, value.committedAtEnd)) muted = value;
  });
  let decorations = prepared.decorations;
  for (const value of new Set([prepared.mutedWildcard, muted])) {
    if (!value) continue;
    const active = String(value === muted);
    let replacement: Decoration | null = null;
    decorations.between(value.bodyFrom, value.bodyTo, (_from, _to, mark) => {
      if (
        mark.spec.wildcardDiagnostic === value &&
        mark.spec.attributes?.['data-diagnostic-active'] !== active
      )
        replacement = Decoration.mark({
          ...mark.spec,
          attributes: {
            ...mark.spec.attributes,
            'data-diagnostic-active': active,
          },
        });
    });
    if (replacement)
      decorations = decorations.update({
        filterFrom: value.bodyFrom,
        filterTo: value.bodyTo,
        filter: (_from, _to, mark) => mark.spec.wildcardDiagnostic !== value,
        add: [(replacement as Decoration).range(value.bodyFrom, value.bodyTo)],
      });
  }
  return decorations === prepared.decorations && muted === prepared.mutedWildcard
    ? prepared
    : { ...prepared, decorations, mutedWildcard: muted };
}
