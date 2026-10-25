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
/** Share bounded source rasters between consumers and refresh only after committed canvas changes. */
import type { CanvasSourceHost } from '../../../../../integrations/contracts/CanvasSources.js';
import type {
  CanvasInputSource,
  CanvasPixelRaster,
  CanvasDocument,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import { CanvasSourceUnavailable } from '../domain/CanvasSourceAvailability.js';
import type { CanvasSourceCapture } from './CanvasSourceCapture.js';
import {
  MAX_CANVAS_RASTER_BYTES,
  canvasInputSourceKey,
} from '../../core/embedded/CanvasRuntime.js';

/** Keep source capture failures explicit so consumers never present stale pixels as a current preview. */
export type CanvasSourcePreview =
  | { readonly kind: 'loading' }
  | {
      readonly kind: 'pixels';
      readonly pixels: CanvasPixelRaster;
      readonly document: CanvasDocument;
      readonly revision: string;
    }
  | { readonly kind: 'error'; readonly message: string; readonly unavailable?: boolean };

interface PreviewEntry {
  readonly source: Exclude<CanvasInputSource, { kind: 'file' }>;
  readonly representation: 'raw' | 'image';
  maximumSize: number;
  readonly listeners: Set<(value: CanvasSourcePreview) => void>;
  value: CanvasSourcePreview;
  serial: number;
}

/** Own preview coalescing and subscription lifetime independently of any mask interpretation or view. */
export class CanvasSourcePreviews {
  readonly #entries = new Map<string, PreviewEntry>();
  readonly #hosts = new Map<CanvasSourceHost, () => void>();
  readonly #dirty = new Set<string>();
  #pending = false;
  #disposed = false;

  /** Reuse the same validated capture path as full-resolution queue preparation. */
  constructor(
    private readonly capture: CanvasSourceCapture,
    private readonly host: (canvasId: string) => CanvasSourceHost | null,
    private readonly report: (error: unknown) => void,
  ) {}

  /** Share raw source pixels so a channel change can reinterpret them without a native round trip. */
  observe(
    source: Exclude<CanvasInputSource, { kind: 'file' }>,
    listener: (value: CanvasSourcePreview) => void,
    maximumSize = 288,
    representation: 'raw' | 'image' = 'raw',
  ): () => void {
    if (this.#disposed) return () => {};
    const key = JSON.stringify([
      canvasInputSourceKey(source),
      maximumSize === 0 ? 'full' : 'display',
      representation,
    ]);
    let entry = this.#entries.get(key);
    if (!entry) {
      entry = {
        source,
        maximumSize,
        representation,
        listeners: new Set(),
        value: { kind: 'loading' },
        serial: 0,
      };
      this.#entries.set(key, entry);
    }
    // Keep the peak while mounted so smaller consumers can reuse a larger completed capture.
    if (maximumSize > entry.maximumSize) {
      entry.maximumSize = maximumSize;
      this.#invalidate(key, entry);
    }
    entry.listeners.add(listener);
    listener(entry.value);
    const host = this.host(source.canvasId);
    if (host && !this.#hosts.has(host))
      this.#hosts.set(
        host,
        host.subscribe(() => this.refresh(host.canvasId)),
      );
    if (entry.value.kind === 'loading') this.#invalidate(key, entry);
    this.#schedule();
    return () => {
      entry.listeners.delete(listener);
      if (!entry.listeners.size) {
        this.#entries.delete(key);
        this.#dirty.delete(key);
      }
      for (const [host, stop] of this.#hosts)
        if (![...this.#entries.values()].some((value) => value.source.canvasId === host.canvasId)) {
          stop();
          this.#hosts.delete(host);
        }
    };
  }

  /** Mark only the changed canvas's mounted sources dirty without graph discovery or file uploads. */
  refresh(canvasId: string): void {
    for (const [key, entry] of this.#entries)
      if (entry.source.canvasId === canvasId) this.#invalidate(key, entry);
    this.#schedule();
  }

  /** Stop accepting results before releasing every native metadata subscription. */
  dispose(): void {
    this.#disposed = true;
    for (const stop of this.#hosts.values()) stop();
    this.#hosts.clear();
    this.#entries.clear();
    this.#dirty.clear();
  }

  /** Fence in-flight results whenever a later committed edit supersedes their native revision. */
  #invalidate(key: string, entry: PreviewEntry): void {
    entry.serial++;
    this.#dirty.add(key);
  }

  /** Admit one coalesced read at a time and keep changes arriving during capture queued. */
  #schedule(): void {
    if (this.#pending || this.#disposed || !this.#dirty.size) return;
    this.#pending = true;
    void Promise.resolve()
      .then(() => this.#read())
      .finally(() => {
        this.#pending = false;
        this.#schedule();
      });
  }

  /** Preserve per-canvas failure isolation and suppress obsolete results after unmount or a later edit. */
  async #read(): Promise<void> {
    const requested = [...this.#dirty];
    this.#dirty.clear();
    const groups = new Map<string, { key: string; entry: PreviewEntry; serial: number }[]>();
    for (const key of requested) {
      const entry = this.#entries.get(key);
      if (!entry) continue;
      const groupKey = JSON.stringify([entry.source.canvasId, entry.maximumSize]);
      const group = groups.get(groupKey) ?? [];
      group.push({ key, entry, serial: entry.serial });
      groups.set(groupKey, group);
    }
    for (const group of groups.values()) {
      const size = group[0]?.entry.maximumSize ?? 0;
      const count = size
        ? Math.max(1, Math.min(64, Math.floor(MAX_CANVAS_RASTER_BYTES / (size * size * 5))))
        : 1;
      for (let offset = 0; offset < group.length; offset += count) {
        let pending = group.slice(offset, offset + count);
        while (pending.length) {
          const batch = pending;
          pending = [];
          try {
            const snapshots = await this.capture.capture(
              batch.map(({ key, entry }) => ({
                key,
                source: entry.source,
                representation: entry.representation,
              })),
              batch[0]?.entry.maximumSize ?? 288,
            );
            for (const request of batch) {
              const snapshot = snapshots.get(request.key);
              if (!snapshot) throw new Error('Missing canvas source preview.');
              this.#publish(request, {
                kind: 'pixels',
                pixels: snapshot.pixels,
                document: snapshot.document,
                revision: snapshot.revision,
              });
            }
          } catch (error: unknown) {
            if (
              this.#disposed ||
              !batch.some(
                (request) =>
                  this.#entries.get(request.key) === request.entry &&
                  request.serial === request.entry.serial,
              )
            )
              continue;
            const unavailable = error instanceof CanvasSourceUnavailable;
            if (!unavailable) this.report(error);
            const sourceKey =
              unavailable && error.source ? canvasInputSourceKey(error.source) : null;
            const affected = sourceKey
              ? batch.filter((request) => canvasInputSourceKey(request.entry.source) === sourceKey)
              : batch;
            if (unavailable && affected.length)
              pending = batch.filter((request) => !affected.includes(request));
            for (const request of affected)
              this.#publish(request, {
                kind: 'error',
                unavailable,
                message:
                  error instanceof Error ? error.message : 'Canvas source preview unavailable.',
              });
          }
        }
      }
    }
  }

  /** Publish only to the exact still-mounted source generation that requested this snapshot. */
  #publish(
    request: { key: string; entry: PreviewEntry; serial: number },
    value: CanvasSourcePreview,
  ): void {
    const { key, entry, serial } = request;
    if (this.#disposed || this.#entries.get(key) !== entry || entry.serial !== serial) return;
    entry.value = value;
    for (const listener of entry.listeners) listener(value);
  }
}
