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
/** Accepted-input transaction protocol observes native state; its finite lifetime belongs to typed normalization. */
import { requireValue } from '../../core/src/required.js';
import { Annotation, Transaction, type EditorSelection, type Text } from '@codemirror/state';
import type { FeatureProfile } from '../../services/src/index.js';
import { featureProfile } from './features.js';
import { authorship } from './authorship.js';
import { hasLiteralIntent } from './authorship-ranges.js';
/** Semantic follow-up belongs to its already accepted native edit, including metadata-only manual intent. */
export const normalizationJoin = Annotation.define<boolean>();
/** A pending committed-input observation owns no text, caret or history; positions are logical UTF-16. */
export interface PendingEdit {
  readonly from: number;
  readonly to: number;
  readonly deletion: boolean;
  readonly manual: boolean;
  readonly protected: boolean;
  readonly time: number;
  readonly userEvent: string;
  readonly doc: Text;
  readonly selection: EditorSelection;
  readonly profile: FeatureProfile;
}
/** Observe one contiguous input transaction without source parsing or normalization. */
export function inputEdit(transaction: Transaction): PendingEdit | null {
  if (
    !transaction.docChanged ||
    (!transaction.isUserEvent('input.type') && !transaction.isUserEvent('delete'))
  )
    return null;
  let pending: PendingEdit | null = null,
    count = 0;
  transaction.changes.iterChanges((from, to, a, b, inserted) => {
    count++;
    const before = transaction.startState.doc;
    const manual =
      (inserted.toString() === '\\' && /[()]/u.test(before.sliceString(from, from + 1))) ||
      (before.sliceString(from, to) === '\\' && /[()]/u.test(before.sliceString(to, to + 1)));
    pending = {
      from: a,
      to: b,
      deletion: inserted.length === 0,
      manual,
      protected: hasLiteralIntent(transaction.startState.field(authorship), from, to),
      time: requireValue(transaction.annotation(Transaction.time), 'input-descriptor'),
      userEvent: requireValue(transaction.annotation(Transaction.userEvent), 'input-descriptor'),
      doc: transaction.newDoc,
      selection: transaction.newSelection,
      profile: transaction.startState.facet(featureProfile),
    };
  });
  return count === 1 ? pending : null;
}
