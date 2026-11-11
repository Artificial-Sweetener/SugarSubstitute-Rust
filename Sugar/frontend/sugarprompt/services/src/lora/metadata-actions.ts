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
/** Prepared metadata requests and validated replacement catalogs share one finite external host lane. */
import { LatestRequest } from '../requests/latest.js';
import { createLoraCatalogAdapter } from './catalog-adapter.js';
import { readLoraCatalog } from './catalog-validation.js';
import { loraRecord, loraText, readLoraModelIdentity } from './metadata-validation.js';
import {
  isPreparedLoraMetadata,
  validatePreparedLoraMetadata,
  type PreparedLoraMetadata,
} from './metadata-capability.js';
import { prepareThumbnailMenu } from './thumbnail-menu.js';
import type { LoraModelIdentity, LoraCompletionOptions } from './contracts.js';
import type {
  LoraMetadataAcceptance,
  LoraMetadataMenuItem,
  LoraMetadataRequest,
} from './metadata-actions-contracts.js';

const requests = new WeakMap<LoraMetadataRequest, PreparedLoraMetadata>();

/** Freeze the finite prepared hierarchy and its actual child arrays, without retaining another catalog. */
function freezeMenu(item: LoraMetadataMenuItem): LoraMetadataMenuItem {
  return item.kind === 'submenu'
    ? Object.freeze({
        ...item,
        children: Object.freeze(item.children.map(freezeMenu)),
      })
    : Object.freeze(item);
}

/** Compile immutable requests outside input/paint, tied to exact host output choices and the base catalog revision. */
export function prepareLoraMetadataMenu(
  model: LoraModelIdentity,
  catalogRevision: string,
  capability: PreparedLoraMetadata,
): readonly LoraMetadataMenuItem[] {
  if (!isPreparedLoraMetadata(capability))
    throw new TypeError('Invalid prepared metadata capability');
  const canonical = readLoraModelIdentity(model),
    revision = loraText(catalogRevision, 256);
  /** Only this factory brands enabled canonical requests; labels remain literal prepared display data. */
  function action(
    label: string,
    intent:
      | { readonly action: 'refresh' }
      | { readonly action: 'thumbnail'; readonly imageId: string },
  ): LoraMetadataMenuItem {
    const request = Object.freeze({
      model: canonical,
      catalogRevision: revision,
      ...intent,
    });
    requests.set(request, capability);
    return Object.freeze({ kind: 'action', label, request });
  }
  const items: LoraMetadataMenuItem[] = [];
  if (capability.refresh) items.push(action('Refresh CivitAI metadata', { action: 'refresh' }));
  if (capability.thumbnail)
    items.push(
      prepareThumbnailMenu(capability.choices, capability.activeImageId, (label, image) =>
        action(label, { action: 'thumbnail', imageId: image.imageId }),
      ),
    );
  return Object.freeze(items.map(freezeMenu));
}

/** One service lifetime validates host mutation replies and prepares a replacement through the existing catalog owner. */
export class LoraMetadataSession {
  private readonly lane = new LatestRequest();
  private capability: PreparedLoraMetadata | undefined;
  private disposed = false;

  /** The native capability publication already owns copied immutable host choices and method identity. */
  constructor(capability: PreparedLoraMetadata | undefined) {
    validatePreparedLoraMetadata(capability);
    this.capability = capability;
  }

  /** Retired/foreign/disabled requests cannot invoke a host; current unknown replies and failures remain observable. */
  async execute(request: LoraMetadataRequest): Promise<LoraMetadataAcceptance> {
    if (this.disposed) return { status: 'disposed' };
    const capability = this.capability;
    if (!capability) return { status: 'unavailable' };
    if (requests.get(request) !== capability)
      throw new TypeError('Invalid prepared metadata request');
    const result = await this.lane.run((signal) => capability.execute(request, signal));
    if (result.status !== 'ready') return result;
    const value: unknown = result.value;
    const raw = loraRecord(
      value,
      value && typeof value === 'object' && 'status' in value && value.status === 'updated'
        ? ['status', 'catalog', 'completion']
        : ['status'],
    );
    if (raw.status === 'cancelled' || raw.status === 'unavailable') return { status: raw.status };
    if (raw.status !== 'updated') throw new TypeError('Invalid metadata action outcome');
    const catalog = readLoraCatalog(raw.catalog);
    const model = catalog.items.find(
      (item) => item.model.backendValue === request.model.backendValue,
    )?.model;
    if (
      catalog.revision === request.catalogRevision ||
      !catalog.authoritative ||
      !model ||
      model.promptName !== request.model.promptName ||
      model.basename !== request.model.basename
    )
      throw new TypeError('Invalid metadata replacement identity');
    // The factory validates the unknown completion payload before preparing the shared index.
    const adapter = createLoraCatalogAdapter(
      catalog,
      raw.completion === null ? undefined : (raw.completion as LoraCompletionOptions),
    );
    return { status: 'updated', adapter };
  }

  /** Retire stale source/profile/catalog work and settle hosts ignoring abort. */
  cancel(): void {
    this.lane.cancel();
  }

  /** Release the captured host method and settle owned work before native teardown. */
  dispose(): void {
    this.disposed = true;
    this.capability = undefined;
    this.lane.dispose();
  }
}
