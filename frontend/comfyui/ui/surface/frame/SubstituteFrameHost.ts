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
/** Bind reversible host frame mounts to the existing workflow, mode and palette authorities. */
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import type { AccentThemeHost } from '../appearance/AccentThemeHost.js';
import { InputModal } from '../../dialogs/InputModal.js';
import { subscribeVisualStyle } from '../appearance/VisualStyleBoundary.js';
import type { WorkflowSessionStore } from '../../execution/application/WorkflowSessionStore.js';
import type { WorkflowOutputActivity } from '../../execution/application/WorkflowOutputActivity.js';
import type { WorkflowProgressStore } from '../../execution/application/WorkflowProgressStore.js';
import type { CubeStackFrameControls } from '../stack/CubeStackHost.js';
import { SubstituteFrameMode } from './SubstituteFrameMode.js';
import { ComfyFrameWorkflowAdapter } from './ComfyFrameWorkflowAdapter.js';
import { SubstituteFrameView } from './SubstituteFrameView.js';
import { substituteFrameCss } from './SubstituteFrameStyles.js';
import { GenerationStripHost } from './GenerationStripHost.js';
import { SubstituteToolbarLayout } from './SubstituteToolbarLayout.js';
import { DesktopFramePresentation, type DesktopFrameReceipt } from './DesktopFramePresentation.js';
import { desktopFrameCss } from './DesktopFrameStyles.js';
import { readPluginMode, subscribePluginMode } from '../../core/PluginMode.js';
import { PluginToolbarPlacement } from './PluginToolbarPlacement.js';
import { pluginFrameCss } from './PluginFrameStyles.js';

export interface SubstituteFrameHostOptions {
  readonly document: Document;
  readonly app: unknown;
  readonly palette: Pick<AccentThemeHost, 'theme' | 'subscribe'>;
  readonly sessions: WorkflowSessionStore;
  readonly activity: WorkflowOutputActivity;
  readonly progress: Pick<WorkflowProgressStore, 'read' | 'subscribe'>;
  stackControls(): CubeStackFrameControls;
  openOverrides(trigger: HTMLElement): void;
  setOverridesInline(inline: boolean): void;
  report(error: unknown): void;
}

/** Keep one frame lifetime independent of graph rendering, workflow contents and Rust chrome. */
export class SubstituteFrameHost {
  readonly mode = new SubstituteFrameMode();
  readonly #stops: (() => void)[] = [];
  readonly #style: HTMLStyleElement;
  readonly #desktop = new DesktopFramePresentation();
  #presence: HostSurfaceObserver | null = null;
  #mount: HTMLElement | null = null;
  #native: HTMLElement | null = null;
  #view: SubstituteFrameView | null = null;
  readonly #renamePrompt: InputModal;
  #generation: GenerationStripHost | null = null;
  #toolbarLayout: SubstituteToolbarLayout | null = null;
  #pluginToolbar: PluginToolbarPlacement | null = null;
  #adapter: ComfyFrameWorkflowAdapter | null = null;
  #stopWorkflow: (() => void) | null = null;
  #queued = false;
  #disposed = false;

  /** Prepare collaborators without choosing presentation before settings are restored. */
  constructor(private readonly options: SubstituteFrameHostOptions) {
    this.#renamePrompt = new InputModal({
      adapter: {
        getDocument: () => options.document,
        getWindow: () => options.document.defaultView,
      },
    });
    this.#style = options.document.createElement('style');
    this.#style.id = 'sugarcubes-substitute-frame';
    this.#style.textContent = substituteFrameCss + desktopFrameCss + pluginFrameCss;
  }

  /** Configure one desktop document through the presentation protocol without persisting preferences. */
  configureDesktopFrame(configuration: unknown): DesktopFrameReceipt {
    if (this.#disposed) throw new Error('The SugarCubes frame lifetime has ended.');
    this.#desktop.configure(configuration);
    this.mode.setDesktop(true);
    this.#reconcile();
    const receipt = this.#desktop.read();
    if (!receipt) throw new Error('Desktop frame configuration was not retained.');
    return receipt;
  }

  /** Read accepted desktop presentation without changing DOM, preferences, or workflow state. */
  readDesktopFrame(): DesktopFrameReceipt | null {
    const receipt = this.#desktop.read();
    return receipt
      ? Object.freeze({
          ...receipt,
          mounted: receipt.mounted && this.#view?.element.isConnected === true,
        })
      : null;
  }

  /** Subscribe to named host mounts and existing workflow and appearance owners only. */
  setup(): void {
    this.#stops.push(
      subscribePluginMode(this.options.document, () => this.#reconcile()),
      this.mode.subscribe(() => this.#reconcile()),
      this.options.palette.subscribe(() => this.#palette()),
      this.options.sessions.subscribe(() => this.#queue()),
      this.options.activity.subscribe(() => this.#queue()),
      this.options.progress.subscribe(() => this.#refreshProgress()),
      subscribeVisualStyle(this.options.document, () => {
        this.#unmount();
        this.#reconcile();
      }),
    );
    this.#presence = new HostSurfaceObserver(
      this.options.document.body,
      '.workflow-tabs-container',
      () => this.#queue(),
      { content: true, exclude: '.sc-substitute-frame' },
    );
    this.mode.setup(this.options.app);
    this.#reconcile();
    this.options.document.defaultView?.addEventListener('pagehide', this.dispose);
  }

  /** Restore native presentation and release only owned subscriptions and mounts. */
  readonly dispose = (): void => {
    this.#disposed = true;
    this.#unmount();
    this.#desktop.dispose();
    this.mode.setDesktop(false);
    this.#presence?.disconnect();
    for (const stop of this.#stops.splice(0)) stop();
    this.mode.dispose();
    this.#style.remove();
    this.options.document.documentElement.removeAttribute('data-sc-substitute-frame');
    this.options.document.defaultView?.removeEventListener('pagehide', this.dispose);
  };

  /** Coalesce host and workflow events without promoting popup activity into graph discovery. */
  #queue(): void {
    if (this.#queued || this.#disposed) return;
    this.#queued = true;
    queueMicrotask(() => {
      this.#queued = false;
      this.#reconcile();
    });
  }

  /** Replace visual presentation while retaining Comfy's mounted action and workflow owners. */
  #reconcile(): void {
    if (this.#disposed) return;
    const { document } = this.options;
    const plugin = readPluginMode(document);
    if (!this.mode.enabled && !plugin) {
      this.#unmount();
      this.#style.remove();
      document.documentElement.removeAttribute('data-sc-substitute-frame');
      return;
    }
    const native = document.querySelector<HTMLElement>(
      '.workflow-tabs-container .workflow-tabs-container',
    );
    const mount = native?.parentElement?.parentElement ?? null;
    if (!native || !mount) return;
    try {
      if (this.#mount !== mount || this.#native !== native) {
        this.#unmount();
        this.#attach(mount, native);
      }
      if (!this.#style.isConnected) document.head.append(this.#style);
      this.#view?.setPluginMode(plugin);
      if (plugin && !this.#pluginToolbar && this.#view && this.#generation)
        this.#pluginToolbar = new PluginToolbarPlacement(
          document,
          this.#view.element,
          this.#view.applicationMenu,
          this.#generation.element,
          () => this.#generation?.refreshPlacement(),
        );
      else if (!plugin && this.#pluginToolbar) {
        this.#pluginToolbar.dispose();
        this.#pluginToolbar = null;
      }
      if (!document.documentElement.hasAttribute('data-sc-substitute-frame'))
        document.documentElement.setAttribute('data-sc-substitute-frame', '');
      this.options.setOverridesInline(true);
      this.#view?.update(this.#adapter?.read(this.options.sessions.read().open) ?? [], (id) =>
        this.options.activity.unread(id),
      );
      this.#palette();
      this.#refreshProgress();
    } catch (error: unknown) {
      this.options.report(error);
      this.#unmount();
      document.documentElement.removeAttribute('data-sc-substitute-frame');
    }
  }

  /** Bind typed intents once per native frame replacement. */
  #attach(mount: HTMLElement, native: HTMLElement): void {
    this.#mount = mount;
    this.#native = native;
    this.#adapter = new ComfyFrameWorkflowAdapter(this.options.app, native, (name, validate) =>
      this.#renamePrompt.open({
        title: 'Rename workflow',
        label: 'Name',
        initialValue: name,
        confirmLabel: 'Rename',
        normalizeValue: (value) => value.trim(),
        validate,
      }),
    );
    this.#stopWorkflow = this.#adapter.subscribe(() => this.#queue());
    const command = (id: string): void => {
      void this.#adapter?.command(id).catch(this.options.report);
    };
    this.#view = new SubstituteFrameView(this.options.document, {
      workflows: {
        activate: (id) => this.#adapter?.activate(id, this.options.sessions.read().open),
        close: (id) => this.#adapter?.close(id, this.options.sessions.read().open),
        menu: (id) =>
          this.#adapter?.menu(id, this.options.sessions.read().open, this.options.report) ??
          Promise.reject(new Error('Comfy workflow controls are unavailable.')),
        reorder: (from, to) => this.#adapter?.reorder(from, to),
        create: () => command('Comfy.NewBlankWorkflow'),
        report: this.options.report,
      },
      command,
      stack: this.options.stackControls(),
      openOverrides: this.options.openOverrides,
      report: this.options.report,
    });
    native.parentElement?.setAttribute('data-sc-frame-native', '');
    mount.append(this.#view.element);
    this.#desktop.attach(mount);
    this.#generation = new GenerationStripHost(
      this.options.document,
      this.#view.element,
      this.options.app,
      this.options.report,
    );
    this.#toolbarLayout = new SubstituteToolbarLayout(this.options.document);
  }

  /** Refresh only the retained progress tracks for the currently presented workflow. */
  #refreshProgress(): void {
    this.#view?.updateProgress(this.options.progress.read(this.options.sessions.presentation()));
  }

  /** Project accepted palette facts without adding a theme observer or renderer dependency. */
  #palette(): void {
    const { color, dark } = this.options.palette.theme;
    this.#view?.setPalette(color, dark);
    this.#generation?.setPalette(dark);
    this.#pluginToolbar?.setPalette(dark);
  }

  /** Retire the view while preserving host controls, workflow state and saved preferences. */
  #unmount(): void {
    this.#renamePrompt.close(null);
    this.#desktop.detach();
    this.#pluginToolbar?.dispose();
    this.#pluginToolbar = null;
    this.options.setOverridesInline(false);
    this.#toolbarLayout?.dispose();
    this.#toolbarLayout = null;
    this.#generation?.dispose();
    this.#generation = null;
    this.#stopWorkflow?.();
    this.#stopWorkflow = null;
    this.#view?.dispose();
    this.#native?.parentElement?.removeAttribute('data-sc-frame-native');
    this.#mount = null;
    this.#native = null;
    this.#view = null;
    this.#adapter = null;
  }
}
