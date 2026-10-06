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
/** Narrow discovery responses before any dynamic provider values enter Vue presentation. */

import { isRecord } from '../types/common.js';
import { modelProviderPage } from './ModelContextMenu.js';
import type {
  ModelDiscoveryContext,
  ModelDiscoveryOffer,
  ModelDiscoveryPlan,
  ModelDiscoverySuggestion,
} from './ModelDiscovery.js';

/** Decode an authoritative context without duplicating the server's supported-family policy. */
export function decodeDiscoveryContext(value: unknown): ModelDiscoveryContext | null {
  if (!isRecord(value) || value.schemaVersion !== 1)
    throw new Error('Invalid model discovery context.');
  return value.context === null ? null : readContext(value.context);
}

/** Validate bounded cards, retained plan identity, provider links, and local preview routes. */
export function decodeDiscoveryPlan(value: unknown): ModelDiscoveryPlan {
  if (!isRecord(value) || value.schemaVersion !== 1 || typeof value.publicOnly !== 'boolean')
    throw new Error('Invalid model discovery plan.');
  const context = readContext(value);
  const planId = text(value.planId, 32);
  if (!/^[a-f0-9]{32}$/.test(planId)) throw new Error('Invalid model discovery identity.');
  const suggestions = list(value.suggestions, 8).map((item): ModelDiscoverySuggestion => {
    if (!isRecord(item)) throw new Error('Invalid model suggestion.');
    const identity = text(item.identity, 160);
    if (!identity.startsWith(context.artifactKind + ':') || !/:[a-f0-9]{64}$/.test(identity))
      throw new Error('Invalid model suggestion identity.');
    const offers = list(item.offers, 8).map((offer) => readOffer(offer, planId));
    if (!offers.length || new Set(offers.map((offer) => offer.providerId)).size !== offers.length)
      throw new Error('Invalid model provider offers.');
    return {
      identity,
      modelName: text(item.modelName, 4096),
      versionName: text(item.versionName, 4096),
      creator: item.creator === null ? null : text(item.creator, 4096),
      offers,
    };
  });
  if (new Set(suggestions.map((item) => item.identity)).size !== suggestions.length)
    throw new Error('Duplicate model suggestion.');
  const browseUrls = list(value.browseUrls, 8).map((item) => {
    if (!isRecord(item)) throw new Error('Invalid model browse source.');
    return { providerId: text(item.providerId, 64), url: page(item.url) };
  });
  return { ...context, planId, publicOnly: value.publicOnly, suggestions, browseUrls };
}

/** Validate one source while keeping alternate acquisition offers separate. */
function readOffer(value: unknown, planId: string): ModelDiscoveryOffer {
  if (
    !isRecord(value) ||
    (value.access !== 'public' && value.access !== 'api_key_required') ||
    typeof value.sizeBytes !== 'number' ||
    !Number.isSafeInteger(value.sizeBytes) ||
    value.sizeBytes < 1 ||
    value.sizeBytes > 128 * 1024 ** 3
  )
    throw new Error('Invalid model offer.');
  const providerId = text(value.providerId, 64);
  const previewUrl = value.previewUrl === null ? null : text(value.previewUrl, 512);
  if (
    previewUrl !== null &&
    (!previewUrl.startsWith(`/sugarcubes/v1/models/discovery/${planId}/preview/`) ||
      !/^\/sugarcubes\/v1\/models\/discovery\/[a-f0-9]{32}\/preview\/[0-9]{1,3}\/(?:civitai|openmodeldb)$/.test(
        previewUrl,
      ) ||
      !previewUrl.endsWith('/' + providerId))
  )
    throw new Error('Invalid model preview route.');
  return {
    providerId,
    providerName: text(value.providerName, 128),
    modelId: text(value.modelId, 256),
    versionId: text(value.versionId, 256),
    modelPageUrl: page(value.modelPageUrl),
    fileName: text(value.fileName, 1024),
    sizeBytes: value.sizeBytes,
    access: value.access,
    previewUrl,
  };
}

/** Keep compatibility decisions server-owned while rejecting malformed context fields. */
function readContext(value: unknown): ModelDiscoveryContext {
  if (!isRecord(value)) throw new Error('Invalid model discovery context.');
  return {
    artifactKind: text(value.artifactKind, 64),
    familyId: value.familyId === null ? null : text(value.familyId, 128),
  };
}

/** Admit only the reviewed provider-page protocols and hosts shared with context menus. */
function page(value: unknown): string {
  const url = modelProviderPage(text(value, 2048));
  if (url === null) throw new Error('Invalid model provider link.');
  return url;
}

/** Bound external string metadata without parsing markup or coercing primitive values. */
function text(value: unknown, limit: number): string {
  if (typeof value !== 'string' || !value.trim() || value.length > limit)
    throw new Error('Invalid model discovery text.');
  return value;
}

/** Bound provider-controlled collection sizes before mapping card or offer state. */
function list(value: unknown, limit: number): readonly unknown[] {
  if (!Array.isArray(value) || value.length > limit)
    throw new Error('Invalid model discovery collection.');
  return value;
}
