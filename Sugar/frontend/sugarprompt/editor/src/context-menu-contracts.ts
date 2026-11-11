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
/** Prepared native menu leaves preserve the identities owned by their domain action providers. */
import type { LoraMetadataMenuItem, LoraPageRequest } from '../../services/src/index.js';
import type { PreparedDocumentValueEdit } from './document-value-edits.js';
import type { SceneQueueRequest } from '../../services/src/scenes/queue.js';
import type { SpellingDiagnosticValue } from './spelling-diagnostics.js';
import type { DuplicateDiagnosticValue } from './duplicate-diagnostics.js';
import type { TextMenuItem } from './text-menu.js';
import type { SavedSegment } from '../../services/src/saved-segments/contracts.js';

/** Literal menu presentation delegates accepted source edits and external intents to their existing owners. */
export type ContextMenuItem =
  | { readonly kind: 'schedule-lora'; readonly label: 'Schedule LoRA…' }
  | { readonly kind: 'heading'; readonly label: string }
  | { readonly kind: 'save-segment'; readonly label: 'Save selected segment…' }
  | {
      readonly kind: 'insert-segment';
      readonly label: string;
      readonly segment: SavedSegment;
    }
  | LoraMetadataMenuItem
  | TextMenuItem
  | {
      readonly kind: 'rich-rendering';
      readonly label: 'Rich prompt rendering';
      readonly checked: boolean;
    }
  | {
      readonly kind: 'queue-scene';
      readonly label: 'Queue this scene';
      readonly request: SceneQueueRequest;
    }
  | {
      readonly kind: 'source-edit';
      readonly purpose: 'trigger' | 'duplicate' | 'spelling';
      readonly label: string;
      readonly title: string;
      readonly replacement: PreparedDocumentValueEdit;
    }
  | {
      readonly kind: 'spelling-action';
      readonly action: 'ignore' | 'add';
      readonly label: string;
      readonly finding: SpellingDiagnosticValue;
    }
  | {
      readonly kind: 'ignore-duplicate';
      readonly label: 'Ignore duplicate';
      readonly finding: DuplicateDiagnosticValue;
    }
  | {
      readonly kind: 'submenu';
      readonly label: string;
      readonly children: readonly ContextMenuItem[];
    }
  | {
      readonly kind: 'page';
      readonly label: string;
      readonly request: LoraPageRequest;
    };
