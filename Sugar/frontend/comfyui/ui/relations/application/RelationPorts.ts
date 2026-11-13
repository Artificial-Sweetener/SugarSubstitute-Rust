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
/** Describe host collaborators without importing graph or presentation implementations. */
import type {
  FieldContract,
  FieldValue,
  NodeContract,
  RelationState,
  RelationSeedMode,
} from '../domain/RelationContracts.js';

export interface EditableRelationField {
  readonly contract: FieldContract;
  read(): FieldValue;
  write(value: FieldValue): void;
  writeSeedMode?(mode: RelationSeedMode): void;
}
export interface RelationInventory {
  readonly fields: readonly EditableRelationField[];
  readonly nodes: readonly NodeContract[];
  readonly editableNodes?: readonly {
    readonly id: string;
    readMode?(): number;
    writeMode(mode: number): void;
  }[];
}
export interface RelationWorkflowPort {
  inventory(): RelationInventory;
  load(): RelationState;
  save(state: RelationState): void;
  transaction(operation: () => void): void;
  refresh(nodeIds: readonly string[]): void;
}
