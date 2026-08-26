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
/**
 * Define Cube chrome collaboration boundaries.
 */

import type { CubeGroupMetadataRecord } from '../graph/GroupMetadata.js';
import type { CubeFaceGraphSummary } from '../surface/CubeFaceChromeActions.js';
import type { ComfyApplication, ComfyCanvas, ComfyGraph } from '../types/graph.js';
import type { RectBounds, UnknownRecord, Vec2 } from '../types/common.js';
import type { WorkflowCubeLibraryClass } from '../workflow/CubeWorkflowLibraryState.js';
import type { WorkflowCubeClassification } from '../workflow/CubeWorkflowLibraryState.js';

export interface ChromeMetadata extends CubeGroupMetadataRecord {
  managed?: boolean;
  instance_id?: string;
  cube_id?: string;
  cube_version?: string;
  default_alias?: string;
  font_size?: number;
  has_saveable_changes?: boolean;
  markers?: { inputs?: unknown[]; outputs?: unknown[] };
  bounds?: { header?: { height?: number } };
  graphSummary?: CubeFaceGraphSummary;
}
export interface BadgeSource {
  sourceKind?: string;
  author?: string;
  pack?: string;
  namespace?: string;
  libraryClass?: WorkflowCubeLibraryClass;
}
export interface HitRegion {
  key: string;
  tooltip: string;
  instanceId: string;
  metadata: ChromeMetadata;
  flavorOptions: unknown;
  rect: RectBounds;
}
export interface BadgeRegion {
  key: string;
  instanceId: string;
  rect: RectBounds;
  truncated: boolean;
  fullText: string;
}
export interface ChromeActions extends UnknownRecord {
  onSaveImplementation?(metadata: ChromeMetadata): void;
  onSaveDraft?(metadata: ChromeMetadata): void;
  onSwapLeft?(metadata: ChromeMetadata): void;
  onSwapRight?(metadata: ChromeMetadata): void;
  canSwap?(metadata: ChromeMetadata, direction: 'left' | 'right'): boolean;
  onOpenMenu?(metadata: ChromeMetadata, options: unknown[]): void;
  getLibraryClassification?(metadata: ChromeMetadata): WorkflowCubeClassification | null;
  onKeepWorkflowCube?(metadata: ChromeMetadata): void;
  onSaveWorkflowCubeToStable?(metadata: ChromeMetadata): void;
  onSyncWorkflowCubeSource?(metadata: ChromeMetadata): void;
  onForkWorkflowCube?(metadata: ChromeMetadata): void;
}
export interface ChromeDebugState {
  actions: ChromeActions;
  hitRegions: readonly HitRegion[];
  badgeRegions: readonly BadgeRegion[];
  hoveredKey: string | null;
  hoveredBadgeInstance: string | null;
  hoveredBadgeKey: string | null;
}
export interface ChromeCanvas extends ComfyCanvas {
  graph?: ComfyGraph;
  editor_alpha?: number;
  canvas?: HTMLCanvasElement;
  ds?: { scale?: number; offset?: number[] };
  convertEventToCanvasOffset?(event: MouseEvent | PointerEvent): unknown;
  convertCanvasToOffset?(point: Vec2): unknown;
}
export interface ChromeAdapter {
  getLiteGraph?(): LiteGraphHost | null;
  getApp?(): ComfyApplication | null;
}
export interface ChromeOverlayOptions {
  adapter?: ChromeAdapter | null;
  actions?: ChromeActions;
  resolveSource?: ((metadata: ChromeMetadata) => BadgeSource | null) | null;
}
export interface ChromeInstanceState {
  dirty: boolean;
  appearAt: number;
  savedAt?: number;
}
export interface ChromeHoverState {
  hoveredKey: string | null;
  hoveredInstance: string | null;
  hoveredBadgeInstance: string | null;
  hoveredBadgeKey: string | null;
}
