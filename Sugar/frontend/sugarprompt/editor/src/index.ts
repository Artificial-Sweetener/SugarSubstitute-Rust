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
/** Native editor entry point; hosts normally consume the Vue facade. */
export type {
  SearchContext,
  SearchRequest,
  SearchMatch,
  SearchResults,
  SearchHostIntent,
  SearchRevealAdapter,
  SearchNavigation,
  SearchOutcome,
  SearchSnapshot,
} from '../../services/src/search/contracts.js';
export type {
  ClipboardAction,
  ClipboardCommandOutcome,
  ClipboardObserver,
} from './native-clipboard.js';
export type { LoraPageHost, LoraMetadataHost } from './lora-action-hosts.js';
export { mountEditor } from './controller.js';
export type {
  DocumentCheckpoint,
  DocumentCaptureOutcome,
  DocumentCheckpointRejection,
} from './document-checkpoint.js';
export type {
  RegionNameChange,
  RegionNameInspection,
  RegionNameTarget,
  RegionNameSyncOutcome,
} from './region-host-contracts.js';
export type { SceneQueueHost } from './scene-queue.js';
export type { EditorOptions } from './editor-options.js';
export type { LoraSchedulingHost } from './lora-schedule.js';
export type { WheelMode } from './wheel-mode.js';
export type {
  SourceLineChromeOptions,
  SourceLineRect,
  SourceLineSnapshot,
} from './source-line-contracts.js';
export type {
  PreparedSourceEdit,
  SourceEditPreparation,
  SourceEditOutcome,
  ReservedSourceEdit,
  SourceEditReservation,
} from './prepared-edits.js';
export type {
  PromptEditorApi,
  EditorSnapshot,
  EditorLayoutMetrics,
  SceneContextInspection,
  ConditioningInspection,
  StructureInspection,
} from './contracts.js';

export type {
  HostUpdate,
  HostConfiguration,
  HostSelection,
  HostUpdateOutcome,
} from './host-update.js';
export type { SavedSegmentHost } from './saved-segment-menu.js';
export type {
  FieldActionContext,
  FieldActionContribution,
  FieldActionEntry,
  FieldActionOutcome,
  FieldActionPreparation,
} from './field-actions-contracts.js';
export type { FieldActionUtilities } from './field-action-utilities.js';
export type { FocusHandoff } from './focus-handoff-contracts.js';
