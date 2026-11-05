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
import type { CanvasCatalogEntry } from './CanvasCatalog.js';
import type { CanvasAttachmentAddress } from './CanvasIdentity.js';
import type { CanvasSelection, CanvasViewState } from './CanvasViewState.js';
import type { CanvasRevealTarget } from './CanvasReveal.js';
import type { CanvasMediaTarget } from './CanvasMedia.js';
import type {
  OutputCopyFormat,
  OutputMediaProfile,
} from '../../output_saving/domain/OutputMedia.js';
import type { CanvasSurfaceGeometry } from '../../../../../integrations/contracts/CanvasSurfaceProtocol.generated.js';

export type { CanvasSurfaceGeometry } from '../../../../../integrations/contracts/CanvasSurfaceProtocol.generated.js';

export interface CanvasSourceSummary {
  readonly id: OutputSourceId;
  readonly label: string;
  readonly breadcrumb: readonly string[];
  /** Count available source positions, including its live first-position placeholder. */
  readonly batchCount: number;
  readonly preview: boolean;
  readonly previousRun: boolean;
}

/** One prompt scene of the current scene run, as the scene picker offers it. */
export interface CanvasSceneSummary {
  readonly key: string;
  readonly title: string;
  /** Whether the scene has an image or live preview to show yet. */
  readonly available: boolean;
}

/**
 * The image a native surface paints beneath the overlay, where it lies on screen.
 *
 * Web content cannot sample native pixels, so glass chrome over a native surface blurs this copy
 * of the image instead of its own backdrop.
 */
export interface CanvasBackdrop {
  readonly url: string;
  /** The whole image's rectangle, in client CSS pixels. */
  readonly rect: {
    readonly left: number;
    readonly top: number;
    readonly width: number;
    readonly height: number;
  };
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
  /** Scenes of the current scene run in authored order; empty unless it has several. */
  readonly scenes: readonly CanvasSceneSummary[];
  /** Publish resource-free progressive batch descriptors for any renderer. */
  readonly grid: readonly CanvasCatalogEntry[];
  readonly comparisonAvailable: boolean;
  readonly selected: WorkflowOutputItem | null;
  readonly secondary: WorkflowOutputItem | null;
  readonly preview: boolean;
  /** Null until the surface reports an authoritative camera for the current image. */
  readonly zoom: CanvasZoomStatus | null;
  readonly resource: CanvasResourceStatus;
  readonly secondaryResource: CanvasResourceStatus | null;
  readonly runStatus: 'idle' | 'queued' | 'running' | 'complete' | 'cancelled' | 'error';
  readonly connected: boolean;
  /** Describe shared transfer preference and qualified local file action without platform handles. */
  readonly mediaActions: OutputMediaProfile | null;
  /** The natively painted image beneath the overlay; absent where the page paints its own. */
  readonly backdrop?: CanvasBackdrop | null;
}

export type CanvasCommand =
  | { readonly type: 'reveal'; readonly target: CanvasRevealTarget }
  | { readonly type: 'select'; readonly selection: CanvasSelection }
  | { readonly type: 'select-source'; readonly selection: CanvasSelection }
  | { readonly type: 'select-preview'; readonly sourceId: OutputSourceId }
  | { readonly type: 'select-scene'; readonly scene: string | null }
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
  | { readonly type: 'open-media'; readonly target: 'primary' | 'secondary' }
  | {
      readonly type: 'copy-media';
      readonly target: CanvasMediaTarget;
      readonly format: OutputCopyFormat;
    }
  | { readonly type: 'file-media'; readonly target: CanvasMediaTarget };

export interface CanvasCommandRequest {
  readonly schema: 1;
  readonly address: CanvasAttachmentAddress;
  readonly requestId: number;
  readonly expectedRevision: number;
  readonly command: CanvasCommand;
}

/** Retain one operation's cancellation and current attachment admission across asynchronous work. */
export interface CanvasOperationScope {
  readonly signal: AbortSignal;
  /** Recheck immediately before an asynchronous operation changes its owning surface. */
  current(): boolean;
}

/** Carry the complete admitted command identity to the surface that will perform it.
 * The surface checks its own revision before applying delayed work; current() only
 * confirms that this request and attachment have not been superseded or retired.
 * Neither the request nor this scope grants native privileges.
 */
export interface CanvasCommandScope extends CanvasOperationScope {
  readonly request: CanvasCommandRequest;
}

export type CanvasCommandOutcome =
  | { readonly status: 'applied'; readonly revision: number }
  | { readonly status: 'stale' | 'unsupported' | 'cancelled'; readonly revision: number }
  | { readonly status: 'error'; readonly revision: number; readonly message: string };

export interface CanvasCapabilities {
  readonly schema: 1;
  readonly commands: readonly CanvasCommand['type'][];
}

export interface CanvasViewPort {
  readonly capabilities: CanvasCapabilities;
  /** Read an immutable snapshot from the surface's authority. */
  read(): CanvasViewSnapshot;
  /** Observe this attachment without subscribing to another window's view. */
  subscribe(listener: (snapshot: CanvasViewSnapshot) => void): () => void;
  /** Allocate an attachment-wide identity for the snapshot revision that produced this intent. */
  createRequest(command: CanvasCommand, expectedRevision: number): CanvasCommandRequest;
  /** Apply a qualified intent or return an explicit stale/unsupported/error outcome. */
  dispatch(request: CanvasCommandRequest): Promise<CanvasCommandOutcome>;
  /** Publish client geometry with explicit CSS units and physical display density. */
  updateSurface(address: CanvasAttachmentAddress, geometry: CanvasSurfaceGeometry): void;
  /** Release one surface's resources without retiring its view or session. */
  releaseSurface(address: CanvasAttachmentAddress): void;
}
