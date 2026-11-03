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
/** Decide which input surfaces and masks a workflow-surface canvas must hold for one workflow. */
import type {
  CanvasWorkflowMask,
  CanvasWorkflowSurface,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type { CanvasSurfaceFileReference } from '../../../../../integrations/contracts/CanvasSurfaceProtocol.generated.js';
import type { DimensionAuthority, DimensionAuthorityNode } from './CanvasDimensionAuthority.js';
import { maskColor, regionMaskColor } from './MaskColorScheme.js';
import type { RegionalPromptDemand } from './RegionalPromptDemand.js';

/** At most this many surfaces cross the wire in one sync, per the canvas contract. */
export const MAX_WORKFLOW_SURFACES = 64;
/** At most this many masks belong to one surface, per the canvas contract. */
export const MAX_SURFACE_MASKS = 32;

/** An image loader field whose IMAGE output is used. */
export interface WorkflowImageEndpoint {
  /** Stable workflow identity of the loader field; it becomes the surface key. */
  readonly key: string;
  readonly label: string;
  readonly file: CanvasSurfaceFileReference | null;
}
/** One mask a mask loader contributes; ordered loaders contribute one per region entry. */
export interface WorkflowMaskEntry {
  /** Region entry identity, or null for a scalar loader's single mask. */
  readonly entry: string | null;
  readonly label: string;
  readonly initial: CanvasSurfaceFileReference | null;
}
/** A mask loader field whose MASK output is used, with its resolved image context. */
export interface WorkflowMaskEndpoint {
  readonly key: string;
  readonly label: string;
  readonly opacity: number;
  readonly entries: readonly WorkflowMaskEntry[];
  /** The associated image endpoint, `ambiguous` when pairing is unsafe, or null when unpaired. */
  readonly image: string | 'ambiguous' | null;
  /** Sizing for an unpaired mask; ignored for paired and ambiguous masks. */
  readonly authority: DimensionAuthority;
  /** Regions declared by related prompts for an ordered loader, or null. */
  readonly regions: RegionalPromptDemand | null;
  /** The channel the loader reads from its files; alpha means inverted alpha. */
  readonly channel: CanvasWorkflowMask['channel'];
}
/** Graph-derived facts for one workflow, already translated to stable keys. */
export interface WorkflowSurfaceFacts {
  readonly images: readonly WorkflowImageEndpoint[];
  readonly masks: readonly WorkflowMaskEndpoint[];
}
/** One planned mask and the loader entry it binds. */
export interface PlannedSurfaceMask {
  readonly key: string;
  readonly endpoint: string;
  readonly entry: string | null;
  readonly label: string;
  readonly color: string;
  readonly opacity: number;
  readonly initial: CanvasSurfaceFileReference | null;
  readonly channel: CanvasWorkflowMask['channel'];
}
/** One planned surface with the endpoint or authority that owns it. */
export interface PlannedSurface {
  readonly key: string;
  readonly label: string;
  /** The image endpoint for authored surfaces; null for mask-only surfaces. */
  readonly image: string | null;
  readonly authority: readonly DimensionAuthorityNode[] | null;
  readonly base: CanvasWorkflowSurface['base'];
  readonly masks: readonly PlannedSurfaceMask[];
}
/** Explain why a used mask endpoint received no surface. */
export interface RejectedMask {
  readonly endpoint: string;
  readonly reason: 'ambiguous_image' | 'unsized' | 'ambiguous_size' | 'limit';
}
export interface WorkflowSurfacePlan {
  readonly surfaces: readonly PlannedSurface[];
  readonly rejected: readonly RejectedMask[];
}

/**
 * Plan one surface per image endpoint and one per shared mask size authority.
 *
 * A mask paired with an image joins that image's surface. An unpaired mask joins the mask-only
 * surface of its resolved size authority, so masks sized by the same nodes share one surface.
 * Ambiguous pairings and unresolved sizes are rejected rather than guessed. Scalar mask colours
 * rotate over each surface's complete mask list, starting from the accent; the regions of an
 * ordered loader take the regional palette over that loader's own regions, as their prompts do.
 */
export function planWorkflowSurfaces(
  facts: WorkflowSurfaceFacts,
  accent: string,
  labels: { readonly synthetic: string },
): WorkflowSurfacePlan {
  const rejected: RejectedMask[] = [];
  const authored = new Map(
    facts.images.flatMap((image) =>
      image.file ? [[image.key, { image, masks: [] as WorkflowMaskEndpoint[] }] as const] : [],
    ),
  );
  const synthetic = new Map<
    string,
    {
      authority: Extract<DimensionAuthority, { kind: 'resolved' }>;
      masks: WorkflowMaskEndpoint[];
    }
  >();
  for (const mask of facts.masks) {
    if (mask.image === 'ambiguous') {
      rejected.push({ endpoint: mask.key, reason: 'ambiguous_image' });
      continue;
    }
    if (mask.image !== null) {
      authored.get(mask.image)?.masks.push(mask);
      continue;
    }
    const authority = mask.authority;
    if (authority.kind !== 'resolved') {
      rejected.push({
        endpoint: mask.key,
        reason: authority.kind === 'ambiguous' ? 'ambiguous_size' : 'unsized',
      });
      continue;
    }
    const group = synthetic.get(authority.key) ?? { authority, masks: [] };
    group.masks.push(mask);
    synthetic.set(authority.key, group);
  }
  const surfaces: PlannedSurface[] = [];
  const place = (surface: Omit<PlannedSurface, 'masks'>, masks: WorkflowMaskEndpoint[]): void => {
    if (surfaces.length >= MAX_WORKFLOW_SURFACES) {
      for (const mask of masks) rejected.push({ endpoint: mask.key, reason: 'limit' });
      return;
    }
    const entries = masks.flatMap((mask) =>
      mask.entries.map(
        (entry, index) =>
          ({
            mask,
            entry,
            label: mask.regions?.names[index] ?? entry.label,
            region: mask.regions ? regionMaskColor(index, mask.entries.length) : null,
          }) as const,
      ),
    );
    const admitted = entries.slice(0, MAX_SURFACE_MASKS);
    for (const mask of new Set(entries.slice(MAX_SURFACE_MASKS).map(({ mask }) => mask.key)))
      rejected.push({ endpoint: mask, reason: 'limit' });
    surfaces.push({
      ...surface,
      masks: admitted.map(({ mask, entry, label, region }, index) => ({
        key: workflowMaskKey(mask.key, entry.entry),
        endpoint: mask.key,
        entry: entry.entry,
        label,
        color: region ?? maskColor(accent, index, admitted.length),
        opacity: mask.opacity,
        initial: entry.initial,
        channel: mask.channel,
      })),
    });
  };
  for (const { image, masks } of authored.values())
    if (image.file)
      place(
        {
          key: image.key,
          label: image.label,
          image: image.key,
          authority: null,
          base: { kind: 'file', file: image.file },
        },
        masks,
      );
  for (const { authority, masks } of synthetic.values())
    place(
      {
        key: '@synthetic/' + stableHash(authority.key),
        label: masks[0]?.label ?? labels.synthetic,
        image: null,
        authority: authority.nodes,
        base: { kind: 'blank', width: authority.width, height: authority.height },
      },
      masks,
    );
  return { surfaces, rejected };
}

/** Key a scalar mask by its loader field and a region by its persisted entry identity. */
export function workflowMaskKey(endpoint: string, entry: string | null): string {
  return entry === null ? endpoint : JSON.stringify([endpoint, entry]);
}

/** Project a plan onto the `workflow.sync` surface list. */
export function workflowSyncSurfaces(plan: WorkflowSurfacePlan): readonly CanvasWorkflowSurface[] {
  return plan.surfaces.map((surface) => ({
    key: surface.key,
    label: surface.label,
    base: surface.base,
    masks: surface.masks.map((mask) => ({
      key: mask.key,
      label: mask.label,
      color: mask.color,
      opacity: mask.opacity,
      initial: mask.initial,
      channel: mask.channel,
    })),
  }));
}

/** Parse a Comfy loader value such as `sub/dir/name.png [output]` into a file reference. */
export function comfyFileReference(value: string): CanvasSurfaceFileReference | null {
  const match = /^(.*?)(?: \[(input|output|temp)\])?$/.exec(value.trim());
  const path = (match?.[1] ?? '').replaceAll('\\', '/');
  const parts = path.split('/'),
    filename = parts.pop() ?? '';
  if (!filename || parts.some((part) => part === '..')) return null;
  const type = match?.[2];
  return {
    filename,
    subfolder: parts.join('/'),
    type: type === 'output' || type === 'temp' ? type : 'input',
  };
}

/** Shorten a structural identity into a fixed-width key with 64-bit FNV-1a over UTF-16 units. */
function stableHash(value: string): string {
  let hash = 0xcbf29ce484222325n;
  for (let index = 0; index < value.length; index++) {
    const unit = value.charCodeAt(index);
    for (const byte of [unit & 0xff, unit >> 8]) {
      hash ^= BigInt(byte);
      hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
    }
  }
  return hash.toString(16).padStart(16, '0');
}
