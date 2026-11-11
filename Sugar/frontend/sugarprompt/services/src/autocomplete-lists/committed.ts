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
/** A validated atomic mutation remains accepted even when later catalog preparation or observation fails. */
import type { AutocompleteListSnapshot } from './contracts.js';

/** A one-shot authenticated accepted receipt is independent of current mutable publication. */
export interface AutocompleteListCommitReceipt {
  readonly value: AutocompleteListSnapshot;
  readonly priorRevision: string;
}

/** Exact accepted snapshot accompanies the original internal preparation or observer cause; consumers acknowledge only their still-owned native operands. */
export class AutocompleteListCommittedError extends Error {
  readonly value: AutocompleteListSnapshot;
  readonly priorRevision: string;
  /** The canonical session creates this receipt after host acknowledgement and full mutation validation, when later preparation or observation fails. */
  constructor(value: AutocompleteListSnapshot, priorRevision: string, cause: unknown) {
    super('Autocomplete committed publication failed', { cause });
    this.name = 'AutocompleteListCommittedError';
    this.value = value;
    this.priorRevision = priorRevision;
  }
}
