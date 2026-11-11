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
/** Duplicate commands consume current canonical diagnostics, Core recipes and genuinely issued native value edits. */
import { requireValue } from '../../core/src/required.js';
import type { EditorState } from '@codemirror/state';
import {
  duplicateDiagnostics,
  currentDuplicateDiagnostic,
  type DuplicateDiagnosticValue,
} from './duplicate-diagnostics.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { semanticStructures } from './structure.js';
import { nativeDocumentValueAt } from './document-values.js';
import { prepareDocumentValueDuplicate } from './document-value-ingestion.js';
import { prepareDocumentValueEdit } from './document-value-edit-issuance.js';
import type { DocumentValueSelectionRange } from './document-value-edits.js';
import type { ContextMenuItem } from './context-menu-contracts.js';
/** Logical-document UTF-16 hit bounds retain the actual indexed finding. */
export interface DuplicateMenuTarget {
  readonly from: number;
  readonly to: number;
  readonly finding: DuplicateDiagnosticValue;
}
/** One current native menu retains exact actionable leaves and its source/context identity. */
export interface PreparedDuplicateMenu extends DuplicateMenuTarget {
  readonly kind: 'duplicate-segment';
  readonly freshness: EditorFreshness;
  readonly items: readonly ContextMenuItem[];
  readonly editLeaves: ReadonlySet<
    Extract<
      ContextMenuItem,
      {
        kind: 'source-edit';
      }
    >
  >;
  readonly ignoreLeaf: Extract<
    ContextMenuItem,
    {
      kind: 'ignore-duplicate';
    }
  >;
}
/** Bounded half-open lookup retains duplicate inspection under the caret and rejects ignored identities. */
export function duplicateMenuTargetAt(
  state: EditorState,
  position: number,
): DuplicateMenuTarget | null {
  const current = state.field(duplicateDiagnostics, false);
  if (!current || current.freshness !== state.field(editorFreshness)) return null;
  let target: DuplicateMenuTarget | null = null;
  current.findings.between(position, position, (from, to, finding) => {
    if (from <= position && position < to && currentDuplicateDiagnostic(state, finding))
      target = { from, to, finding };
  });
  return target;
}
/** Prepare both independent recipes off input; unsafe cross-value selection refuses an edit rather than inventing a caret. */
export function prepareDuplicateMenu(
  state: EditorState,
  target: DuplicateMenuTarget,
): PreparedDuplicateMenu | null {
  if (duplicateMenuTargetAt(state, target.from)?.finding !== target.finding) return null;
  const native = state.field(semanticStructures).native;
  if (!native) return null;
  const context = {
    values: native.nativeValues,
    semantics: native.documentSemantics,
    enabled: !state.readOnly,
    composing: false,
  };
  const admitted = nativeDocumentValueAt(state, context, {
    from: target.from,
    to: target.to,
  });
  const items: ContextMenuItem[] = [];
  const leaves = new Set<
    Extract<
      ContextMenuItem,
      {
        kind: 'source-edit';
      }
    >
  >();
  for (const action of ['remove', 'emphasize-first'] as const) {
    const label = action === 'remove' ? 'Remove duplicate' : 'Emphasize first';
    let leaf: Extract<
      ContextMenuItem,
      {
        kind: 'source-edit';
      }
    > | null = null;
    if (admitted.status === 'ready') {
      const value = admitted.target;
      const provenance = prepareDocumentValueDuplicate(
        state,
        context,
        value,
        target.finding,
        action,
      );
      const mapping = requireValue(value.entry.semanticMapping, 'duplicate-menu');
      const ranges: DocumentValueSelectionRange[] = [];
      let safe = provenance !== null;
      for (const [index, range] of state.selection.ranges.entries()) {
        const anchor = mapping.logicalToDecoded[range.anchor - mapping.logicalRange.from];
        const head = mapping.logicalToDecoded[range.head - mapping.logicalRange.from];
        if (anchor != null && head != null && provenance)
          ranges.push({
            kind: 'decoded',
            anchor: requireValue(
              provenance.normalization.boundaryPositions[anchor],
              'duplicate-menu',
            ),
            head: requireValue(provenance.normalization.boundaryPositions[head], 'duplicate-menu'),
          });
        else if (
          (range.anchor < mapping.logicalRange.from || range.anchor > mapping.logicalRange.to) &&
          (range.head < mapping.logicalRange.from || range.head > mapping.logicalRange.to)
        )
          ranges.push({ kind: 'outside', index });
        else safe = false;
      }
      if (safe && provenance) {
        const result = prepareDocumentValueEdit(state, {
          context,
          target: value,
          range: { from: 0, to: value.scope.mapping.text.length },
          text: provenance.normalization.text,
          publication: { purpose: 'duplicate', normalization: provenance },
          selection: { ranges, mainIndex: state.selection.mainIndex },
        });
        if (result.status !== 'rejected')
          leaf = Object.freeze({
            kind: 'source-edit',
            purpose: 'duplicate',
            label,
            title: target.finding.diagnostic.message,
            replacement: result.edit,
          });
      }
    }
    if (leaf) {
      leaves.add(leaf);
      items.push(leaf);
    } else items.push(Object.freeze({ kind: 'disabled', label }));
  }
  const ignoreLeaf = Object.freeze({
    kind: 'ignore-duplicate' as const,
    label: 'Ignore duplicate',
    finding: target.finding,
  });
  items.push(ignoreLeaf);
  return Object.freeze({
    ...target,
    kind: 'duplicate-segment',
    freshness: state.field(editorFreshness),
    items: Object.freeze(items),
    editLeaves: leaves,
    ignoreLeaf,
  });
}
/** Current source, context, feature, permission, finding identity and visible token are rechecked before any activation. */
export function currentDuplicateMenu(state: EditorState, menu: PreparedDuplicateMenu): boolean {
  const target = duplicateMenuTargetAt(state, menu.from);
  return (
    state.field(editorFreshness) === menu.freshness &&
    target?.finding === menu.finding &&
    target.to === menu.to
  );
}
