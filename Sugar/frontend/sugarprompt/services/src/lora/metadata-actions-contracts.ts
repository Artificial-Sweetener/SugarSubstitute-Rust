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
/** Host-managed metadata and final-output image assignment preserve one canonical catalog publication. */
import type {
  LoraCatalogSnapshot,
  LoraCompletionOptions,
  LoraModelIdentity,
  LoraResourceAdapter,
} from './contracts.js';

/** Prepared final output identity and literal grouping metadata; hosts retain the image bytes. */
export interface LoraOutputImage {
  /** Canonical hyphenated UUID; duplicate identities retain the first descriptor. */
  readonly imageId: string;
  /** Literal scene identity, up to 256 UTF-16 units; an empty key retains the ungrouped scene. */
  readonly sceneKey: string;
  /** Literal scene label, up to 512 UTF-16 units; an empty label displays Scene. */
  readonly sceneTitle: string;
  /** Integral scene display order from 0 through 1000000. */
  readonly sceneOrder: number;
  /** Nonempty literal output-source identity, up to 256 UTF-16 units. */
  readonly sourceKey: string;
  /** Literal source label, up to 512 UTF-16 units; an empty label displays Output. */
  readonly sourceLabel: string;
  /** Integral batch identity from 0 through 1000000, displayed in ascending order. */
  readonly setIndex: number;
  /** Nullable intrinsic pixel width; known dimensions are integers from 1 through 16384. */
  readonly width: number | null;
  /** Nullable intrinsic pixel height; known dimensions are integers from 1 through 16384. */
  readonly height: number | null;
}

/** Requests carry canonical model and catalog identity; no prompt text, alias or acquired image enters this boundary. */
export type LoraMetadataRequest = {
  readonly model: LoraModelIdentity;
  readonly catalogRevision: string;
} & ({ readonly action: 'refresh' } | { readonly action: 'thumbnail'; readonly imageId: string });

/** Successful host mutation supplies its complete replacement catalog and deliberate completion configuration. */
export type LoraMetadataResult =
  | {
      readonly status: 'updated';
      readonly catalog: LoraCatalogSnapshot;
      readonly completion: LoraCompletionOptions | null;
    }
  | { readonly status: 'cancelled' | 'unavailable' };

/** Replace this immutable capability when choices, active image or permissions change; host acquisition/storage stays external. */
export interface LoraMetadataAdapter {
  /** Nonempty publication revision, up to 256 UTF-16 units; replace the record when its snapshot changes. */
  readonly revision: string;
  readonly refresh: boolean;
  readonly thumbnail: boolean;
  /** At most 512 final-output descriptors; image bytes and acquisition remain host-owned. */
  readonly choices: readonly LoraOutputImage[];
  /** Null or an image UUID present in this publication's prepared choices. */
  readonly activeImageId: string | null;
  /** Perform the accepted host mutation and return a complete replacement or an explicit expected outcome. */
  execute(request: LoraMetadataRequest, signal: AbortSignal): Promise<LoraMetadataResult>;
}

/** Accepted catalog identity lets the host publish compatible scheduling context; source and editable aliases remain absent. */
export type LoraMetadataOutcome =
  | { readonly status: 'updated'; readonly catalogRevision: string }
  | { readonly status: 'cancelled' | 'unavailable' | 'disposed' };

/** Services prepare the same supported catalog adapter before native freshness acceptance and resource-owner replacement. */
export type LoraMetadataAcceptance =
  | { readonly status: 'updated'; readonly adapter: LoraResourceAdapter }
  | { readonly status: 'cancelled' | 'unavailable' | 'disposed' };

/** Literal prepared menu trees preserve disabled, direct, grouped and active-image behavior without live catalog traversal. */
export type LoraMetadataMenuItem =
  | {
      readonly kind: 'action';
      readonly label: string;
      readonly request: LoraMetadataRequest;
    }
  | { readonly kind: 'disabled'; readonly label: string }
  | {
      readonly kind: 'submenu';
      readonly label: string;
      readonly children: readonly LoraMetadataMenuItem[];
    }
  | { readonly kind: 'separator' };
