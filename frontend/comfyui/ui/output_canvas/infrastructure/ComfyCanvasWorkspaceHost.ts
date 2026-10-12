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
/** Coordinate identified workspace mounts and host modes without owning canvas view or image state. */

import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import { createComfyAppModeSource } from '../../core/ComfyAppMode.js';
import { createComfyRendererModeChangeSource } from '../../core/ComfyRendererMode.js';
import { CanvasWorkspaceLayout } from '../application/CanvasWorkspaceLayout.js';
import type { CanvasFrameClock } from '../application/CanvasCameraTransition.js';
import { BrowserCanvasWorkspace } from './BrowserCanvasWorkspace.js';
import type { ComfyCanvasPreferences } from './ComfyCanvasPreferences.js';

/** Retain content identity through temporary host suspension while retiring replaced native surfaces. */
export class ComfyCanvasWorkspaceHost {
  readonly #layout = new CanvasWorkspaceLayout();
  readonly #stops: (() => void)[] = [];
  #observer: HostSurfaceObserver | null = null;
  #workspace: BrowserCanvasWorkspace | null = null;
  #panel: HTMLElement | null = null;
  #appMode = false;
  #configuring = false;
  #disposed = false;
  #started = false;

  /** Inject host lifecycle sources, accepted settings, and finite geometry scheduling. */
  constructor(
    private readonly app: unknown,
    private readonly document: Document,
    private readonly preferences: ComfyCanvasPreferences,
    private readonly frames: CanvasFrameClock,
    private readonly changed: () => void,
    private readonly report: (error: unknown) => void,
  ) {}

  /** Expose the allocated remaining rectangle as the sole masonry mount parent. */
  get content(): HTMLElement | null {
    return this.#workspace?.content ?? null;
  }

  /** Expose the primary canvas allocation only while its supported host surface is visible. */
  get canvas(): HTMLElement | null {
    return this.#workspace?.visible ? this.#workspace.canvas : null;
  }

  /** Observe only identified graph workspace mounting and public app/renderer lifecycles. */
  setup(): void {
    if (this.#started || this.#disposed) return;
    this.#started = true;
    const mode = createComfyAppModeSource(this.app);
    this.#appMode = mode.active;
    this.#stops.push(
      mode.subscribe((active) => {
        this.#appMode = active;
        this.#workspace?.suspend(active || this.#configuring);
        this.changed();
      }),
      this.preferences.subscribe(() => {
        this.#workspace?.preferences(this.preferences.snapshot);
        this.changed();
      }),
      createComfyRendererModeChangeSource(this.app).subscribe(() => {
        // Native renderer replacement retires its pixels before the new renderer becomes interactive.
        this.changed();
        this.#reconcile();
      }),
    );
    this.#observer = new HostSurfaceObserver(
      this.document.body,
      '#graph-canvas-container, .graph-canvas-panel',
      () => this.#reconcile(),
    );
    this.#reconcile();
  }

  /** Preserve lightweight view state while primary graph configuration has no deliverable surface. */
  beforeConfigureGraph(): void {
    this.#configuring = true;
    this.#workspace?.suspend(true);
    this.changed();
  }

  /** Restore allocation before masonry asks for its existing content parent. */
  afterConfigureGraph(): void {
    this.#configuring = false;
    this.#reconcile();
    this.#workspace?.suspend(this.#appMode);
    this.changed();
  }

  /** Apply explicit drawer intent immediately; host setting callbacks own accepted visibility. */
  toggle(): void {
    this.#visibility(this.canvas === null);
  }

  /** Relinquish only this primary workspace and leave independently owned future windows unaffected. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#observer?.disconnect();
    for (const stop of this.#stops.splice(0)) stop();
    const workspace = this.#workspace;
    this.#workspace = null;
    this.changed();
    workspace?.dispose();
    this.#panel = null;
  }

  /** Route one local action through public Comfy persistence and preserve host-owned rollback semantics. */
  #visibility(visible: boolean): void {
    if (this.#disposed) return;
    const pending = this.preferences.visible(visible);
    this.#workspace?.drawer(visible);
    this.changed();
    void pending.catch(this.report);
  }

  /** Mount only qualified siblings, releasing the old attachment before removing its allocation. */
  #reconcile(): void {
    if (this.#disposed) return;
    const root = this.document.getElementById('graph-canvas-container');
    const panel = root?.querySelector<HTMLElement>('.graph-canvas-panel') ?? null;
    if (panel === this.#panel && this.#workspace) return;
    const previous = this.#workspace;
    this.#workspace = null;
    this.#panel = null;
    this.changed();
    previous?.dispose();
    if (!root || !panel) return;
    try {
      this.#layout.preferences(this.preferences.snapshot);
      this.#workspace = new BrowserCanvasWorkspace(root, panel, this.#layout, this.frames, {
        width: (width) => {
          void this.preferences.width(width).catch(this.report);
        },
        close: () => this.#visibility(false),
        changed: this.changed,
      });
      this.#panel = panel;
      this.#workspace.suspend(this.#appMode || this.#configuring);
      this.changed();
    } catch (error: unknown) {
      this.report(error);
    }
  }
}
