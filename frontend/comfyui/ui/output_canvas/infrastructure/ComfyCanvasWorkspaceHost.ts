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
import type { WorkflowViewSource } from '../../workflow/domain/WorkflowView.js';
import { readPluginMode, subscribePluginMode } from '../../core/PluginMode.js';

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
  #accepted: { visible: boolean; width: number } | null = null;
  #userVisibilityChange = false;

  /** Inject host lifecycle sources, accepted settings, and finite geometry scheduling. */
  constructor(
    private readonly app: unknown,
    private readonly document: Document,
    private readonly preferences: ComfyCanvasPreferences,
    private readonly frames: CanvasFrameClock,
    private readonly changed: () => void,
    private readonly report: (error: unknown) => void,
    private readonly presentation: WorkflowViewSource,
  ) {}

  /** Expose the allocated remaining rectangle as the sole masonry mount parent. */
  get content(): HTMLElement | null {
    return this.#workspace?.content ?? null;
  }

  /** Expose the primary canvas allocation only while its supported host surface is visible. */
  get canvas(): HTMLElement | null {
    return this.#workspace?.presented ? this.#workspace.canvas : null;
  }

  /** Read accepted open state independently of pixels retained during an exit slide. */
  get visible(): boolean {
    return this.#workspace?.visible ?? false;
  }

  /** Observe only identified graph workspace mounting and public app/renderer lifecycles. */
  setup(): void {
    if (this.#started || this.#disposed) return;
    this.#started = true;
    const mode = createComfyAppModeSource(this.app);
    this.#appMode = mode.active;
    this.#stops.push(
      subscribePluginMode(this.document, () => this.#refreshPreferences()),
      mode.subscribe((active) => {
        this.#appMode = active;
        this.#workspace?.suspend(this.#suspension());
        this.changed();
      }),
      this.preferences.subscribe(() => {
        this.#refreshPreferences();
      }),
      this.presentation.subscribe(() => {
        this.#refreshPreferences();
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

  /** Preserve a qualified history surface while suspending primary pixels for graph replacement. */
  beforeConfigureGraph(retainCanvas = false): void {
    this.#configuring = !retainCanvas;
    this.#workspace?.suspend(this.#suspension());
    this.changed();
  }

  /** Restore allocation before masonry asks for its existing content parent. */
  afterConfigureGraph(): void {
    this.#configuring = false;
    this.#reconcile();
    this.#workspace?.suspend(this.#suspension());
    this.changed();
  }

  /** Toggle accepted open state while allowing the outgoing image to finish its slide. */
  toggle(): void {
    this.#visibility(!this.visible);
  }

  /** Open the current workflow's pane idempotently for explicit image inspection. */
  show(): void {
    if (!this.visible) this.#visibility(true);
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

  /** Animate one accepted local action through the authoritative workflow preference owner. */
  #visibility(visible: boolean): void {
    if (this.#disposed || readPluginMode(this.document)) return;
    this.#userVisibilityChange = true;
    try {
      this.presentation.change({ canvas: visible });
      this.#workspace?.drawer(visible, true);
    } finally {
      this.#userVisibilityChange = false;
    }
    this.changed();
  }

  /** Combine global sizing with the workflow's authoritative visibility choice. */
  #preferences() {
    return {
      ...this.preferences.snapshot,
      visible: !readPluginMode(this.document) && this.presentation.snapshot.canvas,
    };
  }

  /** Hide replacement pixels without collapsing allocation; app mode relinquishes the whole frame. */
  #suspension(): 'app' | 'configure' | null {
    return this.#appMode ? 'app' : this.#configuring ? 'configure' : null;
  }

  /** Avoid layout and resize resets when session readiness changes without a preference change. */
  #refreshPreferences(): void {
    const next = this.#preferences();
    if (this.#accepted?.visible === next.visible && this.#accepted.width === next.width) return;
    this.#accepted = next;
    this.#workspace?.preferences(next, this.#userVisibilityChange);
    this.changed();
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
      this.#accepted = this.#preferences();
      this.#layout.preferences(this.#accepted);
      this.#workspace = new BrowserCanvasWorkspace(root, panel, this.#layout, this.frames, {
        width: (width) => {
          void this.preferences.width(width).catch(this.report);
        },
        close: () => this.#visibility(false),
        changed: this.changed,
        report: this.report,
      });
      this.#panel = panel;
      this.#workspace.suspend(this.#suspension());
      this.changed();
    } catch (error: unknown) {
      this.report(error);
    }
  }
}
