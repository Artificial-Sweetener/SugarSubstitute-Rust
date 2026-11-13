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
/** Compose scoped native thumbnail actions around cached workflow output membership. */
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import {
  createComfyRendererModeChangeSource,
  resolveComfyRendererMode,
} from '../../core/ComfyRendererMode.js';
import { isRecord, type UnknownRecord } from '../../types/common.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasRevealTarget } from '../domain/CanvasReveal.js';
import { resolveComfyImageReveal } from './ComfyImageRevealTarget.js';
import { ComfyImageRevealDom, ensureImageRevealStyles } from './ComfyImageRevealDom.js';
import { ComfyImageRevealMenus } from './ComfyImageRevealMenus.js';
import { ComfyLegacyImageReveal } from './ComfyLegacyImageReveal.js';

export interface ComfyImageRevealHostOptions {
  readonly document: Document;
  readonly app: unknown;
  readonly logger: Pick<Console, 'error'>;
  catalog(): CanvasCatalogSnapshot | null;
  sources(): readonly { readonly path: string; readonly node: UnknownRecord }[];
  subscribe(listener: () => void): () => void;
  reveal(target: CanvasRevealTarget): Promise<boolean>;
}

/** Translate native surface mounts into local actions without rediscovering or refreshing unrelated cards. */
export class ComfyImageRevealHost {
  readonly #dom = new Map<HTMLElement, ComfyImageRevealDom>();
  readonly #menus: ComfyImageRevealMenus;
  readonly #legacy: ComfyLegacyImageReveal;
  readonly #stops: (() => void)[] = [];
  readonly #media = new Map<string, string>();
  #observer: HostSurfaceObserver | null = null;

  /** Compose focused adapter owners with one shared reveal authority. */
  constructor(private readonly options: ComfyImageRevealHostOptions) {
    const target = (path: string, url: string, batchIndex: number | null) =>
      resolveComfyImageReveal(
        options.catalog(),
        path,
        url,
        options.document.location.origin,
        batchIndex,
      );
    const reveal = (target: CanvasRevealTarget): void => {
      void options
        .reveal(target)
        .catch((error: unknown) =>
          options.logger.error('[SugarCubes] Image reveal failed', { target, error }),
        );
    };
    this.#menus = new ComfyImageRevealMenus(target, reveal);
    this.#legacy = new ComfyLegacyImageReveal(
      options.document,
      () => {
        const app = isRecord(options.app) ? options.app : {};
        return isRecord(app.canvas) ? app.canvas : null;
      },
      target,
      reveal,
    );
  }

  /** Observe only mounted native previews and authoritative output/renderer lifecycle changes. */
  setup(): void {
    if (this.#observer) return;
    ensureImageRevealStyles(this.options.document);
    this.#observer = new HostSurfaceObserver(
      this.options.document.body,
      '.image-preview',
      (changes) => {
        for (const { element, kind, relocated } of changes) {
          if (kind === 'removed' && !relocated) {
            this.#dom.get(element)?.dispose();
            this.#dom.delete(element);
          } else this.#mount(element);
        }
        this.#observer?.discardPending();
      },
      { content: true, attributes: ['src'], exclude: '[data-sc-image-reveal]' },
    );
    for (const root of this.options.document.querySelectorAll<HTMLElement>('.image-preview'))
      this.#mount(root);
    this.#stops.push(
      this.options.subscribe(() => this.#refresh()),
      createComfyRendererModeChangeSource(this.options.app).subscribe(() => this.#refresh()),
    );
    this.#refresh();
  }

  /** Release native callbacks, local actions, and subscriptions without touching workflow data. */
  dispose(): void {
    this.#observer?.disconnect();
    this.#observer = null;
    for (const stop of this.#stops.splice(0)) stop();
    for (const binding of this.#dom.values()) binding.dispose();
    this.#dom.clear();
    this.#media.clear();
    this.#menus.dispose();
    this.#legacy.dispose();
  }

  /** Bind an identified preview root once, preserving control identity during unrelated popup activity. */
  #mount(root: HTMLElement): void {
    if (this.#dom.has(root)) {
      this.#dom.get(root)?.refresh();
      return;
    }
    const reveal = (target: CanvasRevealTarget): void => {
      void this.options
        .reveal(target)
        .catch((error: unknown) =>
          this.options.logger.error('[SugarCubes] Image reveal failed', { target, error }),
        );
    };
    this.#dom.set(
      root,
      new ComfyImageRevealDom(root, {
        target: (image) => this.#target(root, image),
        reveal,
        context: (image) => {
          const target = this.#target(root, image);
          const path = this.#path(root);
          if (target && path) this.#menus.context(path, target);
        },
      }),
    );
  }

  /** Refresh cached preview roots only when their owning result session or native renderer changes. */
  #refresh(): void {
    const sources = this.options.sources().filter(({ node }) => !isRecord(node.subgraph));
    this.#menus.reconcile(sources);
    this.#legacy.reconcile(
      resolveComfyRendererMode(this.options.app, null, this.options.document) === 'litegraph'
        ? sources
        : [],
    );
    const catalog = this.options.catalog();
    const changed = new Set<string>();
    const paths = new Set(sources.map(({ path }) => path));
    for (const path of new Set([...paths, ...this.#media.keys()])) {
      const entry = catalog?.result.outputs.find((entry) => entry.source.executionPath === path);
      const signature = JSON.stringify([
        catalog?.sessionId,
        catalog?.generation,
        entry?.items.map((item) => item.id),
      ]);
      if (this.#media.get(path) === signature) continue;
      changed.add(path);
      if (paths.has(path)) this.#media.set(path, signature);
      else this.#media.delete(path);
    }
    for (const [root, binding] of this.#dom) {
      const path = this.#path(root);
      if (!path || changed.has(path)) binding.refresh();
    }
    this.#observer?.discardPending();
  }

  /** Resolve a mounted node identity against existing source handles, refusing ambiguous nested IDs. */
  #path(root: HTMLElement): string | null {
    const id = root.closest('[data-node-id]')?.getAttribute('data-node-id');
    if (!id) return null;
    const matches = this.options
      .sources()
      .filter(({ node, path }) => String(node.id) === id || path === id);
    return matches.length === 1 ? (matches[0]?.path ?? null) : null;
  }

  /** Admit only a known completed output file for this exact mounted native source. */
  #target(root: HTMLElement, image: HTMLImageElement): CanvasRevealTarget | null {
    const path = this.#path(root);
    const grid = image.closest('[data-testid="image-grid"]');
    const node = this.options.sources().find((source) => source.path === path)?.node;
    const index = grid ? [...grid.querySelectorAll('img')].indexOf(image) : node?.imageIndex;
    const batchIndex =
      typeof index === 'number' && Number.isSafeInteger(index) && index >= 0 ? index : null;
    return path
      ? resolveComfyImageReveal(
          this.options.catalog(),
          path,
          image.src,
          this.options.document.location.origin,
          batchIndex,
        )
      : null;
  }
}
