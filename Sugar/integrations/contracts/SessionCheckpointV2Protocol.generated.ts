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
/** Generated from session-checkpoint-v2.schema.json by scripts/integrations/generate-session-contract.ts. */
import type {
  SessionScope,
  SessionSource,
  SessionAsset,
  SessionOutput,
  SessionSelection,
  SessionCamera,
  SessionComparison,
  SessionDocument,
  SessionInput,
  SessionSubmission,
  SessionPlacement,
  SessionNavigation,
  SessionInspection,
} from './SessionProtocol.generated.js';
/** Describe the schema-defined SessionScroll wire value. */
export type SessionScroll = { readonly cube: string | null; readonly offset: number };
/** Describe the schema-defined SessionView wire value. */
export type SessionView = {
  readonly canvasEnabled: boolean;
  readonly masonryEnabled: boolean;
  readonly editorWidth: number;
  readonly route: 'input' | 'output';
  readonly follow: 'auto' | 'manual';
  readonly mode: 'image' | 'overview';
  readonly linked: boolean;
  readonly selection: SessionSelection | null;
  readonly comparison: SessionComparison;
  readonly cameras: readonly SessionCamera[];
  readonly navigation: SessionNavigation;
  readonly sourcePreview: boolean;
  readonly sharedInspection: SessionInspection | null;
  readonly scroll: SessionScroll | null;
};
/** Describe the schema-defined SessionEntry wire value. */
export type SessionEntry = {
  readonly id: string;
  readonly document: SessionDocument;
  readonly input: SessionInput | null;
  readonly view: SessionView;
  readonly sourceRevision: number;
  readonly navigationRevision: number;
  readonly sources: readonly SessionSource[];
  readonly outputs: readonly SessionOutput[];
  readonly submissions: readonly SessionSubmission[];
};
/** Describe the schema-defined SessionCheckpoint wire value. */
export type SessionCheckpoint = {
  readonly version: 2;
  readonly id: string;
  readonly revision: number;
  readonly previousId: string | null;
  readonly capturedAt: number;
  readonly scope: SessionScope;
  readonly producer: string;
  readonly frontend: string;
  readonly sugarcubes: string;
  readonly entries: readonly SessionEntry[];
  readonly activeEntry: string | null;
  readonly route: 'workflow' | 'settings';
  readonly placement: SessionPlacement | null;
  readonly assets: readonly SessionAsset[];
};
/** Describe a version-two desktop checkpoint. */
export type SessionCheckpointValue = SessionCheckpoint;
