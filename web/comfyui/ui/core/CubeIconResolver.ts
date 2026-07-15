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
 * Own cube definition icon resolution and placeholder derivation.
 */

import {
  FALLBACK_ICON_STYLE,
  createFallbackIconModel,
  drawFallbackInitialsElementCanvas,
} from './CubeFallbackIconRenderer.js';
import type { FallbackIconModel, FallbackIconStyle } from './CubeFallbackIconRenderer.js';
import { isRecord } from '../types/common.js';

interface AssetIconDescriptor {
  kind: 'asset';
  url?: string;
  path?: string;
  media_type: string;
}

export interface AssetIconModel {
  kind: 'asset';
  url: string;
  mediaType: string;
  initials: string;
  fallback: FallbackIconStyle;
}

export type CubeIconModel = AssetIconModel | FallbackIconModel;

export interface CubeAliasParts {
  prefix: string;
  body: string;
}

interface InitialsOptions {
  fallbackText?: unknown;
}

type ImageStatus = 'loading' | 'ready' | 'error' | 'unavailable';

export interface CubeIconImageEntry {
  status: ImageStatus;
  image: HTMLImageElement | null;
}

interface CubeIconResolverOptions {
  imageFactory?: (() => HTMLImageElement) | null;
  onImageLoad?: ((url: string, entry: CubeIconImageEntry) => void) | null;
}

const SUPPORTED_MEDIA_TYPES = new Set(['image/png', 'image/svg+xml']);
export { FALLBACK_ICON_STYLE };

function readText(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

function readIconDescriptor(source: unknown): AssetIconDescriptor | null {
  const icon = isRecord(source) ? source : null;
  if (!icon || icon.kind !== 'asset') {
    return null;
  }
  const mediaType = readText(icon.media_type);
  if (mediaType && !SUPPORTED_MEDIA_TYPES.has(mediaType)) {
    return null;
  }
  const url = readText(icon.url);
  const path = readText(icon.path) || readText(icon.repo_relative_path);
  if (url) {
    return { kind: 'asset', url, media_type: mediaType || '' };
  }
  return path ? { kind: 'asset', path, media_type: mediaType || '' } : null;
}

function buildIconAssetUrl(cubeId: unknown): string {
  const normalized = readText(cubeId);
  return normalized ? `/sugarcubes/assets/icon?cube_id=${encodeURIComponent(normalized)}` : '';
}

/**
 * Split a styled cube alias into one optional leading prefix and display body.
 */
export function splitCubeAliasPrefix(alias: unknown): CubeAliasParts {
  const stripped = readText(alias);
  const slashIndex = stripped.indexOf('/');
  if (slashIndex <= 0 || slashIndex >= stripped.length - 1) {
    return { prefix: '', body: stripped };
  }

  const prefixBody = stripped.slice(0, slashIndex).trim();
  const body = stripped.slice(slashIndex + 1).trim();
  if (!prefixBody || !body) {
    return { prefix: '', body: stripped };
  }

  return {
    prefix: stripped.slice(0, slashIndex + 1),
    body,
  };
}

function fallbackInitialsLabel(label: unknown): string {
  const parts = splitCubeAliasPrefix(label);
  return parts.prefix ? parts.body : readText(label);
}

function initialWords(label: unknown): string[] {
  return fallbackInitialsLabel(label)
    .split(/[\s/_-]+/)
    .map((word) => word.replace(/[^0-9a-zA-Z]/g, ''))
    .filter(Boolean);
}

/**
 * Derive SugarSubstitute-compatible fallback initials for a cube label.
 */
export function deriveCubeInitials(
  label: unknown,
  { fallbackText = '?' }: InitialsOptions = {},
): string {
  let words = initialWords(label);
  if (!words.length) {
    words = initialWords(fallbackText);
  }
  if (!words.length) {
    return '?';
  }
  if (words.length === 1) {
    return (words[0] ?? '').slice(0, 2).toUpperCase();
  }
  const first = words[0] ?? '';
  const last = words.at(-1) ?? '';
  return `${first[0] || ''}${last[0] || ''}`.toUpperCase();
}

/**
 * Derive default icon initials from a cube definition default alias.
 */
export function deriveDefaultAliasInitials(
  defaultAlias: unknown,
  { fallbackText = '' }: InitialsOptions = {},
): string {
  return deriveCubeInitials(defaultAlias, { fallbackText });
}

/**
 * Resolve the display model for one cube definition icon.
 */
export function resolveCubeIconModel(source: unknown = {}): CubeIconModel {
  const metadata = isRecord(source) ? source : {};
  const defaultAlias = readText(metadata.default_alias);
  const cubeId = readText(metadata.cube_id);
  const iconSource =
    readIconDescriptor(metadata.icon) ||
    readIconDescriptor(isRecord(metadata.metadata) ? metadata.metadata.icon : null);
  const initials = deriveDefaultAliasInitials(defaultAlias, { fallbackText: cubeId });
  const fallback = { ...FALLBACK_ICON_STYLE };
  if (iconSource) {
    const url = iconSource.url || buildIconAssetUrl(cubeId);
    if (url) {
      return {
        kind: 'asset',
        url,
        mediaType: iconSource.media_type || '',
        initials,
        fallback,
      };
    }
  }
  return createFallbackIconModel(initials);
}

/**
 * Cache image objects for repeated chrome rendering.
 */
export class CubeIconResolver {
  private readonly imageFactory: (() => HTMLImageElement) | null;
  private readonly onImageLoad: ((url: string, entry: CubeIconImageEntry) => void) | null;
  private readonly images: Map<string, CubeIconImageEntry>;

  constructor({ imageFactory, onImageLoad }: CubeIconResolverOptions = {}) {
    this.imageFactory = typeof imageFactory === 'function' ? imageFactory : null;
    this.onImageLoad = typeof onImageLoad === 'function' ? onImageLoad : null;
    this.images = new Map();
  }

  resolve(source: unknown = {}): CubeIconModel {
    return resolveCubeIconModel(source);
  }

  getImage(model: CubeIconModel | null | undefined): CubeIconImageEntry {
    if (!model || model.kind !== 'asset' || !model.url) {
      return { status: 'unavailable', image: null };
    }
    const existing = this.images.get(model.url);
    if (existing) {
      return existing;
    }
    const ImageRef =
      this.imageFactory ||
      (typeof globalThis !== 'undefined' && typeof globalThis.Image === 'function'
        ? () => new globalThis.Image()
        : null);
    if (!ImageRef) {
      const unavailable: CubeIconImageEntry = { status: 'error', image: null };
      this.images.set(model.url, unavailable);
      return unavailable;
    }
    const image = ImageRef();
    const entry: CubeIconImageEntry = { status: 'loading', image };
    image.onload = () => {
      entry.status = 'ready';
      this.onImageLoad?.(model.url, entry);
    };
    image.onerror = () => {
      entry.status = 'error';
      this.onImageLoad?.(model.url, entry);
    };
    image.src = model.url;
    this.images.set(model.url, entry);
    return entry;
  }
}

/**
 * Build a DOM icon node for browser surfaces.
 */
export function createCubeIconElement(
  documentRef: Document,
  source?: unknown,
  className?: string,
): HTMLElement;
/** Return no element when a document boundary is unavailable. */
export function createCubeIconElement(
  documentRef: Document | null | undefined,
  source?: unknown,
  className?: string,
): HTMLElement | null;
/** Resolve and render a safe cube icon element for the supplied document. */
export function createCubeIconElement(
  documentRef: Document | null | undefined,
  source: unknown = {},
  className = 'sugarcubes-cube-icon',
): HTMLElement | null {
  const doc = documentRef || (typeof document !== 'undefined' ? document : null);
  if (!doc) {
    return null;
  }
  const model = resolveCubeIconModel(source);
  const root = doc.createElement('span');
  root.className = className;
  if (model.kind === 'asset' && model.url) {
    const image = doc.createElement('img');
    image.alt = '';
    image.decoding = 'async';
    image.loading = 'lazy';
    image.src = model.url;
    root.appendChild(image);
    return root;
  }
  if (model.kind === 'initials' && model.initials) {
    const canvas = doc.createElement('canvas');
    canvas.setAttribute('aria-hidden', 'true');
    canvas.className = 'sugarcubes-cube-icon__fallback';
    drawFallbackInitialsElementCanvas(canvas, model);
    root.dataset.initials = model.initials;
    root.setAttribute('aria-hidden', 'true');
    root.classList.add('is-initials');
    root.appendChild(canvas);
    return root;
  }
  root.classList.add('is-generic');
  root.textContent = '';
  return root;
}
