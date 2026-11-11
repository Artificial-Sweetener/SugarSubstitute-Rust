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
/** Mount host model rendering or native video controls for the exact selected output. */
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { ExecutionClock } from '../../execution/domain/WorkflowExecutionEvent.js';
import type { CanvasResourceStatus } from '../domain/CanvasViewPort.js';
import type { CanvasDisplayPlan } from '../application/CanvasDisplayPlan.js';
import type { CanvasModelRuntime, CanvasModelViewer } from './ComfyCanvasModelRuntime.js';
import { BrowserCanvasMediaSessions } from './BrowserCanvasMediaSessions.js';

export interface BrowserCanvasMediaDetailOptions {
  readonly models: CanvasModelRuntime;
  readonly sessions: BrowserCanvasMediaSessions;
  readonly clock: ExecutionClock;
  url(item: WorkflowOutputItem): string;
  changed(): void;
  report(error: unknown): void;
}

/** Own one detail surface; playback and model interaction remain with their existing runtimes. */
export class BrowserCanvasMediaDetail {
  readonly element: HTMLDivElement;
  #item: WorkflowOutputItem | null = null;
  #controller: AbortController | null = null;
  #video: HTMLVideoElement | null = null;
  #model: CanvasModelViewer | null = null;
  #status: CanvasResourceStatus | null = null;
  #disposed = false;
  #deadline: (() => void) | null = null;

  /** Allocate an empty presentation layer without initializing any media runtime. */
  constructor(
    parent: HTMLElement,
    private readonly options: BrowserCanvasMediaDetailOptions,
  ) {
    this.element = parent.ownerDocument.createElement('div');
    this.element.className = 'sc-output-media-detail';
    this.element.hidden = true;
    parent.append(this.element);
  }

  /** Select content at navigation boundaries while preserving an unchanged player's identity. */
  reconcile(plan: CanvasDisplayPlan, visible: boolean): void {
    const item =
      visible &&
      plan.view.mode === 'image' &&
      !plan.preview &&
      !plan.view.comparison &&
      plan.primary?.sourcePresent
        ? plan.primary.item
        : null;
    const selected =
      item &&
      (item.mediaKind === 'video' || item.mediaKind === 'animated' || item.mediaKind === 'model')
        ? item
        : null;
    if (this.#disposed || selected?.id === this.#item?.id) return;
    this.#release();
    this.#item = selected;
    this.element.hidden = selected === null;
    if (!selected) {
      this.options.changed();
      return;
    }
    this.element.setAttribute(
      'aria-label',
      selected.mediaKind === 'model'
        ? '3D output viewer'
        : selected.mediaKind === 'animated'
          ? 'Animated output viewer'
          : 'Video output viewer',
    );
    this.#status = this.#state('loading', 'Loading output…');
    const controller = (this.#controller = new AbortController());
    this.#deadline = this.options.clock.later(15000, () => {
      this.#failed(new Error('Output loading timed out. Retry this output.'), controller);
      controller.abort();
      if (this.#video) {
        this.#video.pause();
        this.#video.removeAttribute('src');
        this.#video.load();
      }
    });
    if (selected.mediaKind === 'model') void this.#showModel(selected, controller);
    else if (selected.mediaKind === 'animated') this.#showAnimation(selected, controller);
    else this.#showVideo(selected, controller);
    this.options.changed();
  }

  /** Report the detail resource without publishing time updates into source navigation. */
  status(): CanvasResourceStatus | null {
    return this.#status;
  }
  /** Reflow only the active model renderer after a settled canvas resize. */
  resize(): void {
    this.#model?.resize();
  }
  /** Retry only this exact failed selection. */
  retry(plan: CanvasDisplayPlan, visible: boolean): void {
    this.#release();
    this.reconcile(plan, visible);
  }
  /** Count this attachment's actual expensive detail resources. */
  diagnostics(): { videos: number; models: number } {
    return { videos: this.#video ? 1 : 0, models: this.#model ? 1 : 0 };
  }
  /** Release native resources before the owning tab attachment disappears. */
  dispose(): void {
    this.#disposed = true;
    this.#release();
    this.element.remove();
  }

  /** Use the same native player and controls used by Comfy's video surfaces. */
  #showVideo(item: WorkflowOutputItem, controller: AbortController): void {
    const video = (this.#video = this.element.ownerDocument.createElement('video'));
    const saved = this.options.sessions.video(item.id);
    video.controls = true;
    video.playsInline = true;
    video.preload = 'metadata';
    video.loop = saved?.loop ?? true;
    video.muted = saved?.muted ?? true;
    video.volume = saved?.volume ?? 1;
    video.setAttribute('aria-label', 'Output video');
    video.addEventListener(
      'loadedmetadata',
      () => {
        if (saved && saved.time > 0 && Number.isFinite(video.duration))
          video.currentTime = Math.min(saved.time, video.duration);
      },
      { signal: controller.signal },
    );
    video.addEventListener('loadeddata', () => this.#ready(controller), {
      signal: controller.signal,
    });
    video.addEventListener(
      'error',
      () => this.#failed(new Error('The browser could not play this output format.'), controller),
      { signal: controller.signal },
    );
    video.src = this.options.url(item);
    this.element.replaceChildren(video);
  }

  /** Preserve animated-image playback without mounting a still-image decoder. */
  #showAnimation(item: WorkflowOutputItem, controller: AbortController): void {
    const image = this.element.ownerDocument.createElement('img');
    image.alt = 'Animated output';
    image.addEventListener('load', () => this.#ready(controller), { signal: controller.signal });
    image.addEventListener(
      'error',
      () => this.#failed(new Error('The browser could not display this animation.'), controller),
      { signal: controller.signal },
    );
    image.src = this.options.url(item);
    this.element.replaceChildren(image);
  }

  /** Delegate loading and camera interaction to Comfy's viewer behind a retirement fence. */
  async #showModel(item: WorkflowOutputItem, controller: AbortController): Promise<void> {
    const container = this.element.ownerDocument.createElement('div');
    container.className = 'sc-output-model-container';
    this.element.replaceChildren(container);
    try {
      const viewer = await this.options.models.load(
        container,
        this.options.url(item),
        item.file.filename,
        controller.signal,
      );
      if (controller.signal.aborted) {
        viewer.dispose();
        return;
      }
      this.#model = viewer;
      viewer.restore(this.options.sessions.model(item.id));
      viewer.resize();
      this.#ready(controller);
    } catch (error: unknown) {
      this.#failed(error, controller);
    }
  }

  /** Publish readiness only for the still-owned selection. */
  #ready(controller: AbortController): void {
    if (controller.signal.aborted) return;
    this.#deadline?.();
    this.#deadline = null;
    this.#status = this.#state('ready', '');
    this.options.changed();
  }
  /** Preserve failures for retry and report unexpected host exceptions with their context. */
  #failed(error: unknown, controller: AbortController): void {
    if (controller.signal.aborted) return;
    this.#deadline?.();
    this.#deadline = null;
    this.#status = this.#state(
      'error',
      error instanceof Error ? error.message : 'Output viewing failed.',
    );
    this.options.report(error);
    this.options.changed();
  }
  /** Publish the existing resource contract for the shared overlay. */
  #state(state: CanvasResourceStatus['state'], message: string): CanvasResourceStatus {
    return { state, message, canRetry: state === 'error', canAdmitLargeImage: false };
  }
  /** Save lightweight inspection and stop every retired decoder before removing its DOM. */
  #release(): void {
    this.#deadline?.();
    this.#deadline = null;
    const item = this.#item;
    if (item && this.#video) {
      if (this.#status?.state === 'ready' || this.#video.readyState >= 1)
        this.options.sessions.rememberVideo(item.id, {
          time: this.#video.currentTime,
          volume: this.#video.volume,
          muted: this.#video.muted,
          loop: this.#video.loop,
        });
      this.#video.pause();
      this.#video.removeAttribute('src');
      this.#video.load();
    }
    if (item && this.#model) this.options.sessions.rememberModel(item.id, this.#model.camera());
    this.#controller?.abort();
    for (const image of this.element.querySelectorAll('img')) image.removeAttribute('src');
    this.#model?.dispose();
    this.#item = null;
    this.#controller = null;
    this.#video = null;
    this.#model = null;
    this.#status = null;
    this.element.replaceChildren();
    this.element.hidden = true;
  }
}
