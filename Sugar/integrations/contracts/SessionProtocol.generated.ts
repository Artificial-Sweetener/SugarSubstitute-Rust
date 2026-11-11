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
/** Generated from session.schema.json by scripts/integrations/generate-session-contract.ts. */
/** Describe the schema-defined SessionScope wire value. */
export type SessionScope = {
  readonly installation: string;
  readonly profile: string;
  readonly workspace: string;
  readonly backend: string;
};
/** Describe the schema-defined SessionSource wire value. */
export type SessionSource = {
  readonly id: string;
  readonly nodeId: string;
  readonly executionPath: string;
  readonly label: string;
  readonly graphRevision: number;
};
/** Describe the schema-defined SessionScene wire value. */
export type SessionScene = {
  readonly groupId: string;
  readonly sceneId: string;
  readonly order: number;
  readonly count: number;
  readonly title: string;
};
/** Describe the schema-defined SessionFile wire value. */
export type SessionFile = {
  readonly storage: 'output' | 'input' | 'temp';
  readonly filename: string;
  readonly subfolder: string;
  readonly managedArtifact: string | null;
};
/** Describe the schema-defined SessionAsset wire value. */
export type SessionAsset = {
  readonly version: 1;
  readonly digest: string;
  readonly bytes: number;
  readonly extension: 'png' | 'jpg' | 'webp' | 'gif' | 'mp4' | 'webm';
  readonly media: 'image' | 'video';
};
/** Describe the schema-defined SessionOutput wire value. */
export type SessionOutput = {
  readonly id: string;
  readonly entryId: string;
  readonly backend: string;
  readonly promptId: string | null;
  readonly runId: string;
  readonly runOrder: number;
  readonly sourceId: string;
  readonly scene: SessionScene | null;
  readonly listIndex: number;
  readonly batchIndex: number;
  readonly imageIndex: number;
  readonly file: SessionFile;
  readonly assetDigest: string | null;
  readonly digest: string;
  readonly bytes: number;
  readonly width: number;
  readonly height: number;
  readonly media: 'image' | 'video';
  readonly mime: string;
  readonly metadataEmbedded: boolean;
};
/** Describe the schema-defined SessionSelection wire value. */
export type SessionSelection = {
  readonly outputId: string;
  readonly sourceId: string;
  readonly sceneId: string | null;
  readonly batchIndex: number;
  readonly listIndex: number;
};
/** Describe the schema-defined SessionCamera wire value. */
export type SessionCamera = { readonly outputId: string; readonly inspection: SessionInspection };
/** Describe the schema-defined SessionComparison wire value. */
export type SessionComparison = {
  readonly enabled: boolean;
  readonly selection: SessionSelection | null;
  readonly orientation: 'horizontal' | 'vertical';
  readonly split: number;
};
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
};
/** Describe the schema-defined SessionGraphView wire value. */
export type SessionGraphView = { readonly x: number; readonly y: number; readonly scale: number };
/** Describe the schema-defined SessionDocument wire value. */
export type SessionDocument = {
  readonly graphJson: string;
  readonly graphRevision: number;
  readonly path: string | null;
  readonly name: string;
  readonly temporary: boolean;
  readonly dirty: boolean;
  readonly sourceIdentity: string | null;
  readonly view: SessionGraphView;
  readonly baselineGraphJson: string | null;
};
/** Describe the schema-defined SessionInput wire value. */
export type SessionInput = {
  readonly documentId: string;
  readonly generation: number;
  readonly signature: string;
  readonly surface: string | null;
  readonly mask: string | null;
};
/** Describe the schema-defined SessionSubmission wire value. */
export type SessionSubmission = {
  readonly entryId: string;
  readonly backend: string;
  readonly promptId: string;
  readonly runId: string;
  readonly runOrder: number;
  readonly graphRevision: number;
  readonly scene: SessionScene | null;
  readonly sources: readonly SessionSource[];
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
/** Describe the schema-defined SessionRect wire value. */
export type SessionRect = {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
};
/** Describe the schema-defined SessionPlacement wire value. */
export type SessionPlacement = {
  readonly version: 1;
  readonly bounds: SessionRect;
  readonly maximized: boolean;
  readonly monitor: string;
  readonly workArea: SessionRect;
  readonly dpi: number;
};
/** Describe the schema-defined SessionCheckpoint wire value. */
export type SessionCheckpoint = {
  readonly version: 1;
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
/** Describe the schema-defined SessionBinding wire value. */
export type SessionBinding = {
  readonly entryId: string;
  readonly runtimeSession: string;
  readonly generation: number;
  readonly graphRevision: number;
  readonly sourceRevision: number;
  readonly navigationRevision: number;
};
/** Describe the schema-defined SessionFence wire value. */
export type SessionFence = {
  readonly webview: string;
  readonly revision: number;
  readonly bindings: readonly SessionBinding[];
};
/** Describe the schema-defined SessionCapabilities wire value. */
export type SessionCapabilities = {
  readonly capture: boolean;
  readonly materialize: boolean;
  readonly chunkTransfer: boolean;
  readonly artifactResolution: boolean;
};
/** Describe the schema-defined SessionTransfer wire value. */
export type SessionTransfer = {
  readonly id: string;
  readonly checkpointId: string;
  readonly revision: number;
  readonly bytes: number;
  readonly chunks: number;
  readonly digest: string;
  readonly direction: 'capture' | 'materialize';
};
/** Describe the schema-defined SessionFailure wire value. */
export type SessionFailure = {
  readonly code:
    | 'unsupported'
    | 'invalid'
    | 'too-large'
    | 'stale'
    | 'cancelled'
    | 'untrusted-origin'
    | 'unavailable'
    | 'identity-mismatch'
    | 'quota'
    | 'storage'
    | 'migration-required'
    | 'opted-out'
    | 'not-committed';
  readonly retryable: boolean;
  readonly entryId: string | null;
  readonly outputId: string | null;
};
/** Describe the schema-defined SessionOperation wire value. */
export type SessionOperation =
  | {
      readonly kind: 'ready';
      readonly capabilities: SessionCapabilities;
      readonly committed: boolean;
      readonly workflowPersistence: boolean;
      readonly fence: SessionFence;
    }
  | {
      readonly kind: 'capture';
      readonly fence: SessionFence;
      readonly checkpointId: string;
      readonly revision: number;
    }
  | { readonly kind: 'materialize'; readonly fence: SessionFence; readonly transferId: string }
  | { readonly kind: 'begin'; readonly transfer: SessionTransfer }
  | {
      readonly kind: 'chunk';
      readonly transferId: string;
      readonly index: number;
      readonly text: string;
    }
  | { readonly kind: 'commit'; readonly transferId: string }
  | { readonly kind: 'cancel'; readonly targetRequestId: string }
  | {
      readonly kind: 'acknowledged';
      readonly checkpointId: string;
      readonly revision: number;
      readonly phase: 'captured' | 'materialized' | 'durable' | 'restored' | 'cancelled';
      readonly bindings: readonly SessionBinding[];
    }
  | { readonly kind: 'error'; readonly failure: SessionFailure }
  | { readonly kind: 'resolve'; readonly output: SessionOutput }
  | {
      readonly kind: 'resolved';
      readonly outputId: string;
      readonly digest: string;
      readonly bytes: number;
      readonly file: SessionFile;
    };
/** Describe the schema-defined SessionMessage wire value. */
export type SessionMessage = {
  readonly protocol: 'sugarcubes.desktop-session';
  readonly version: 1;
  readonly requestId: string;
  readonly scope: SessionScope;
  readonly origin: string;
  readonly operation: SessionOperation;
};
/** Describe the schema-defined SessionNavigation wire value. */
export type SessionNavigation = {
  readonly sceneOverview: boolean;
  readonly sceneId: string | null;
  readonly sourceId: string | null;
  readonly batchIndex: number;
  readonly batchOverview: boolean;
};
/** Describe the schema-defined SessionInspection wire value. */
export type SessionInspection = {
  readonly mode: 'fit' | 'native' | 'custom';
  readonly centerX: number;
  readonly centerY: number;
  readonly spanX: number;
  readonly spanY: number;
};
/** Describe a version-one desktop checkpoint or session message. */
export type SessionValue = SessionCheckpoint | SessionMessage;
