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
/** Define the versioned overlay contract shared by browser and native surfaces. */

import type { OutputSourceId } from '../../execution/domain/ExecutionIdentity.js';
import type { ImageDimensions, WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasAttachmentAddress } from './CanvasIdentity.js';
import type { CanvasSelection, CanvasViewState } from './CanvasViewState.js';

export interface CanvasSourceSummary {
  readonly id: OutputSourceId;
  readonly label: string;
  readonly breadcrumb: readonly string[];
  readonly batchCount: number;
  readonly preview: boolean;
  readonly previousRun: boolean;
}

export interface CanvasZoomStatus {
  readonly mode: 'fit' | 'native' | 'custom';
  readonly physicalScale: number;
  readonly dimensions: ImageDimensions | null;
  readonly secondaryPhysicalScale: { readonly x: number; readonly y: number } | null;
  readonly secondaryDimensions: ImageDimensions | null;
}

export interface CanvasResourceStatus {
  readonly state: 'empty' | 'loading' | 'ready' | 'unavailable' | 'error' | 'budget';
  readonly message: string;
  readonly canRetry: boolean;
  readonly canAdmitLargeImage: boolean;
}

export interface CanvasViewSnapshot {
  readonly schema: 1;
  readonly address: CanvasAttachmentAddress;
  readonly revision: number;
  readonly view: CanvasViewState;
  readonly sources: readonly CanvasSourceSummary[];
  readonly selected: WorkflowOutputItem | null;
  readonly secondary: WorkflowOutputItem | null;
  readonly preview: boolean;
  readonly zoom: CanvasZoomStatus;
  readonly resource: CanvasResourceStatus;
  readonly secondaryResource: CanvasResourceStatus | null;
  readonly runStatus: 'idle' | 'queued' | 'running' | 'complete' | 'cancelled' | 'error';
  readonly connected: boolean;
}

export type CanvasCommand =
  | { readonly type: 'select'; readonly selection: CanvasSelection }
  | { readonly type: 'select-source'; readonly selection: CanvasSelection }
  | { readonly type: 'batch'; readonly index: number }
  | { readonly type: 'follow'; readonly mode: 'auto' | 'manual' }
  | { readonly type: 'overview'; readonly enabled: boolean }
  | { readonly type: 'link'; readonly enabled: boolean }
  | { readonly type: 'fit' }
  | { readonly type: 'native' }
  | { readonly type: 'zoom'; readonly factor: number }
  | { readonly type: 'pan'; readonly x: number; readonly y: number; readonly units: 'viewport' }
  | { readonly type: 'comparison'; readonly enabled: boolean }
  | { readonly type: 'select-comparison'; readonly selection: CanvasSelection }
  | { readonly type: 'swap' }
  | {
      readonly type: 'wipe';
      readonly orientation: 'vertical' | 'horizontal';
      readonly split: number;
    }
  | { readonly type: 'retry' }
  | { readonly type: 'admit-large-image' }
  | { readonly type: 'open-media'; readonly target: 'primary' | 'secondary' };

export interface CanvasCommandRequest {
  readonly schema: 1;
  readonly address: CanvasAttachmentAddress;
  readonly requestId: number;
  readonly expectedRevision: number;
  readonly command: CanvasCommand;
}

export type CanvasCommandOutcome =
  | { readonly status: 'applied'; readonly revision: number }
  | { readonly status: 'stale' | 'unsupported' | 'cancelled'; readonly revision: number }
  | { readonly status: 'error'; readonly revision: number; readonly message: string };

export interface CanvasCapabilities {
  readonly schema: 1;
  readonly commands: readonly CanvasCommand['type'][];
}

export interface CanvasSurfaceGeometry {
  readonly coordinateSpace: 'client';
  readonly units: 'css-pixels';
  readonly origin: { readonly x: number; readonly y: number };
  readonly viewport: { readonly width: number; readonly height: number };
  readonly clip: {
    readonly x: number;
    readonly y: number;
    readonly width: number;
    readonly height: number;
  };
  readonly physicalPixelsPerCssPixel: number;
  readonly visible: boolean;
  readonly focused: boolean;
}

export interface CanvasViewPort {
  readonly capabilities: CanvasCapabilities;
  /** Read an immutable snapshot from the surface's authority. */
  read(): CanvasViewSnapshot;
  /** Observe this attachment without subscribing to another window's view. */
  subscribe(listener: (snapshot: CanvasViewSnapshot) => void): () => void;
  /** Apply a qualified intent or return an explicit stale/unsupported/error outcome. */
  dispatch(request: CanvasCommandRequest): Promise<CanvasCommandOutcome>;
  /** Publish client geometry with explicit CSS units and physical display density. */
  updateSurface(address: CanvasAttachmentAddress, geometry: CanvasSurfaceGeometry): void;
  /** Release one surface's resources without retiring its view or session. */
  releaseSurface(address: CanvasAttachmentAddress): void;
}
