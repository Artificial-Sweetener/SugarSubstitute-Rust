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
/** Resolve a native thumbnail's file URL against the authoritative workflow result catalog. */
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import { canvasRevealTarget, type CanvasRevealTarget } from '../domain/CanvasReveal.js';

/** Match files within one source; ignore thumbnail encoding and cache-busting parameters. */
export function resolveComfyImageReveal(
  catalog: CanvasCatalogSnapshot | null,
  path: string,
  value: string,
  origin: string,
  batchIndex: number | null,
): CanvasRevealTarget | null {
  if (!catalog || !path) return null;
  let url: URL;
  try {
    url = new URL(value, origin);
  } catch {
    return null;
  }
  if (url.origin !== origin || !['/view', '/api/view'].includes(url.pathname)) return null;
  const source = catalog.sources.find((source) => source.executionPath === path);
  const entry = source
    ? catalog.result.outputs.find((output) => output.source.id === source.id)
    : null;
  const matches = entry?.items.filter(
    (item) =>
      item.mediaKind === 'image' &&
      item.file.filename === url.searchParams.get('filename') &&
      item.file.subfolder === (url.searchParams.get('subfolder') ?? '') &&
      item.file.type === (url.searchParams.get('type') ?? 'output'),
  );
  const item =
    matches?.length === 1
      ? matches[0]
      : batchIndex === null
        ? null
        : matches?.find((item) => item.batchIndex === batchIndex);
  return item ? canvasRevealTarget(item) : null;
}
