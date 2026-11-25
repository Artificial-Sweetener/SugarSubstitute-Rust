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
/** Ask SugarCubes for the saved images of the run a dropped generated image came from. */
import { isRecord } from '../../types/common.js';
import type { WorkflowOutputSource } from '../domain/WorkflowOutput.js';

/** One saved image of a run: its Comfy image reference and the output that made it. */
export interface RunImage {
  readonly image: Readonly<Record<string, unknown>>;
  readonly sourceNodeId: string;
  readonly imageIndex: number;
}

interface RunImagesApi {
  fetchApi(path: string, init?: RequestInit): Promise<Response>;
}

/**
 * Put each run image under the workflow output that made it, in the order the run made them.
 * An output the run left no image for, and an image of no output in this workflow, are skipped.
 */
export function runImagesBySource(
  sources: readonly WorkflowOutputSource[],
  images: readonly RunImage[],
): readonly { readonly source: WorkflowOutputSource; readonly images: readonly unknown[] }[] {
  return sources.flatMap((source) => {
    const own = images
      .filter((image) => image.sourceNodeId === source.executionPath)
      .sort((left, right) => left.imageIndex - right.imageIndex)
      .map((image) => image.image);
    return own.length ? [{ source, images: own }] : [];
  });
}

/**
 * Find the run's images beside the saved copy of `file`. The file is recognised by its name, size
 * and content, so a renamed copy or an image saved elsewhere brings back nothing.
 */
export async function findRunImages(
  api: RunImagesApi,
  file: File,
  runId: string,
): Promise<readonly RunImage[]> {
  const digest = await crypto.subtle.digest('SHA-256', await file.arrayBuffer());
  const sha256 = [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
  const response = await api.fetchApi('/sugarcubes/output-saving/run-images', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ filename: file.name, size: file.size, sha256, run_id: runId }),
  });
  if (!response.ok) return [];
  const body: unknown = await response.json().catch(() => null);
  const listed: unknown = isRecord(body) ? body.images : null;
  return (Array.isArray(listed) ? listed : []).flatMap((entry: unknown): RunImage[] =>
    isRecord(entry) &&
    isRecord(entry.image) &&
    typeof entry.source_node_id === 'string' &&
    typeof entry.image_index === 'number'
      ? [
          {
            image: entry.image,
            sourceNodeId: entry.source_node_id,
            imageIndex: entry.image_index,
          },
        ]
      : [],
  );
}
