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
/** Public observation compatibility changes only for transactions certified by their internal presentation author. */
import { Annotation, type Transaction } from '@codemirror/state';

/** Ordinary native/host signals remain observable even when public snapshot values repeat. */
export type NativeSnapshotPublicationMode = 'current' | 'presentation';

/** Authors apply this only to complete source-neutral presentation transactions; changed public metadata still publishes. */
export const snapshotPresentation = Annotation.define<true>();

/** Mixed, empty and unclassified transaction bundles preserve normal public notification behavior. */
export function snapshotPublicationMode(
  transactions: readonly Transaction[],
): NativeSnapshotPublicationMode {
  return transactions.length > 0 &&
    transactions.every((transaction) => transaction.annotation(snapshotPresentation) === true)
    ? 'presentation'
    : 'current';
}
