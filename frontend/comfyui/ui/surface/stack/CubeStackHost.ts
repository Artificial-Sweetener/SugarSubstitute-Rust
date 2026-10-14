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
/** Bind one Vue rail to masonry, accepted appearance, persisted preferences and graph use cases. */
import type { CubeGraphReorderService } from '../../workflow/application/CubeGraphReorderService.js';
import type { UnknownRecord } from '../../types/common.js';
import type { CubeIdentitySource } from '../../cube/CubeIdentityPresentation.js';
import type { WorkflowMasonrySection } from '../masonry/WorkflowMasonryProjection.js';
import type { FluentAppearanceSource } from '../appearance/FluentAppearanceState.js';
import { applyOwnedFluentTheme } from '../appearance/FluentThemeRuntime.js';
import { CubeStackPreferences } from './CubeStackPreferences.js';
import type { ContextMenuEntry } from '../../menus/ContextMenu.js';
import { projectCubeStackRows } from './CubeStackProjection.js';
import { ensureCubeStackStyles } from './CubeStackStyles.js';
import { mountCubeStackView } from './vue/CubeStackVueRuntime.js';
import { CubeStackScrollSelection } from './CubeStackScrollSelection.js';
import { CubeStackDrawerMotion } from './CubeStackDrawerMotion.js';
import type {
  CubeStackViewActions,
  CubeStackViewState,
  MountedCubeStackView,
} from './CubeStackView.js';

type StackReorder = Pick<
  CubeGraphReorderService,
  'analysis' | 'available' | 'pending' | 'subscribe' | 'move' | 'reorder' | 'refresh'
>;

/** Address only already mounted masonry presentation through its explicit layout boundary. */
export interface CubeStackSurface {
  readonly element: HTMLElement;
  readonly sections: readonly WorkflowMasonrySection[];
  previewCubeOrder(order: readonly string[] | null): void;
  setCubeOrder(order: readonly string[]): void;
  setStackWidth(width: number): void;
  scrollToSection(id: string): void;
}

/** Expose the existing rail preference authority to alternative frame controls. */
export interface CubeStackFrameControls {
  readonly compact: boolean;
  toggleCompact(): Promise<void>;
  menu(): readonly ContextMenuEntry[];
  subscribe(listener: () => void): () => void;
}

/** Keep host actions and external collaboration explicit without giving the rail mutation ownership. */
export interface CubeStackHostOptions {
  readonly frame?: { readonly enabled: boolean; subscribe(listener: () => void): () => void };
  readonly document: Document;
  readonly app: unknown;
  readonly appearance: FluentAppearanceSource;
  getReorder(): StackReorder | null;
  resolveSource(metadata: UnknownRecord): CubeIdentitySource | null;
  refreshSurface(): Promise<void>;
  revealSearchResults(): void;
  openCart(): void;
  remove(instanceId: string): void;
  contextMenu(instanceId: string, event: MouseEvent): void;
  closeTransient(): void;
  reportError(error: unknown): void;
}

/** Retain one presentation application for a mounted surface and release it before native teardown. */
export class CubeStackHost {
  readonly frameControls: CubeStackFrameControls;
  readonly #options: CubeStackHostOptions;
  readonly #preferences: CubeStackPreferences;
  readonly #actions: CubeStackViewActions;
  readonly #subscriptions: (() => void)[] = [];
  #view: CubeStackSurface | null = null;
  #element: HTMLElement | null = null;
  #mounted: MountedCubeStackView | null = null;
  #motion: CubeStackDrawerMotion | null = null;
  #surfaceVisible = true;
  #reorder: StackReorder | null = null;
  #unsubscribeReorder: (() => void) | null = null;
  #selectedId: string | null = null;
  #scrollSelection: CubeStackScrollSelection | null = null;
  #started = false;
  #disposed = false;

  /** Compose typed intent handlers while preference and graph services keep their own authority. */
  constructor(options: CubeStackHostOptions) {
    this.#options = options;
    this.#preferences = new CubeStackPreferences(options.app);
    const preferences = this.#preferences;
    this.frameControls = {
      get compact() {
        return preferences.snapshot.compact;
      },
      toggleCompact: () => preferences.setCompact(!preferences.snapshot.compact),
      menu: () => this.presentationMenu(),
      subscribe: (listener) => preferences.subscribe(listener),
    };
    this.#actions = {
      navigate: (id) => this.#navigate(id),
      previewOrder: (order) =>
        this.#view?.previewCubeOrder(order?.map((id) => `cube:${id}`) ?? null),
      reorder: (id, order) => this.#mutate((service) => service.reorder(id, order)),
      move: (id, delta) => this.#mutate((service) => service.move(id, delta)),
      openCart: options.openCart,
      setCompact: (compact) =>
        void this.#preferences.setCompact(compact).catch(options.reportError),
      hide: () => void this.#preferences.setVisible(false).catch(options.reportError),
      remove: options.remove,
      contextMenu: options.contextMenu,
      reportError: options.reportError,
    };
  }

  /** Restore persisted preferences and subscribe only to the authoritative presentation owners. */
  setup(): void {
    if (this.#started || this.#disposed) return;
    this.#preferences.setup();
    this.#started = true;
    ensureCubeStackStyles(this.#options.document);
    this.#subscriptions.push(
      this.#preferences.subscribe(() => this.#reconcile()),
      this.#options.appearance.subscribeAppearance(() => this.#reconcile()),
    );
    if (this.#options.frame)
      this.#subscriptions.push(this.#options.frame.subscribe(() => this.#reconcile(false)));
  }

  /** Expose rail preference intents to the frame without creating another persistence owner. */
  presentationMenu(): readonly ContextMenuEntry[] {
    const { visible, compact } = this.#preferences.snapshot;
    return [
      {
        label: 'Show Cube stack',
        checked: visible,
        selection: 'multiple',
        select: () => {
          void this.#preferences.setVisible(!visible).catch(this.#options.reportError);
        },
      },
      {
        label: 'Compact Cube stack',
        checked: compact,
        selection: 'multiple',
        select: () => {
          void this.#preferences.setCompact(!compact).catch(this.#options.reportError);
        },
      },
    ];
  }

  /** Attach to already projected sections without triggering graph candidate discovery. */
  ready(view: CubeStackSurface, preparing = false): void {
    if (this.#disposed) return;
    const restoring = this.#view !== view && !preparing;
    if (this.#view !== view) {
      this.release();
      this.#view = view;
      this.#surfaceVisible = !preparing;
      this.#scrollSelection = new CubeStackScrollSelection(view.element, (id) => {
        this.#selectedId = id;
        this.#mounted?.update(this.#state());
      });
    }
    this.#reconcile(!restoring);
  }

  /** Start the rail on the owning masonry transition and retain it through reverse travel. */
  async transition(entering: boolean): Promise<void> {
    this.#surfaceVisible = entering;
    await this.#present(entering && this.#preferences.snapshot.visible);
  }

  /** Refresh literal row presentation after an identified card or canonical permission change. */
  valuesChanged(): void {
    this.#reconcile();
  }

  /** Release preview, reserved width, Vue controls and graph subscriptions with the current surface. */
  release(): void {
    this.#options.closeTransient();
    this.#unmount();
    this.#unsubscribeReorder?.();
    this.#unsubscribeReorder = null;
    this.#reorder = null;
    this.#view = null;
    this.#selectedId = null;
    this.#scrollSelection?.dispose();
    this.#scrollSelection = null;
  }

  /** Restore every retained subscription before extension or document teardown. */
  dispose(): void {
    if (this.#disposed) return;
    this.release();
    this.#disposed = true;
    for (const unsubscribe of this.#subscriptions.splice(0)) unsubscribe();
    this.#preferences.dispose();
  }

  /** Match one rail lifetime to the current mounted surface and accepted visibility preference. */
  #reconcile(animate = true): void {
    const view = this.#view;
    if (!this.#started || this.#disposed || !view) return;
    const reorder = this.#options.getReorder();
    if (this.#reorder !== reorder) {
      this.#unsubscribeReorder?.();
      this.#reorder = reorder;
      this.#unsubscribeReorder = reorder?.subscribe(() => this.#reconcile()) ?? null;
    }
    if (!this.#preferences.snapshot.visible) {
      this.#view?.previewCubeOrder(null);
      this.#view?.setStackWidth(0);
      if (!this.#motion?.canAnimate) this.#unmount();
      else {
        const element = this.#element;
        void this.#present(false)
          .then((completed) => {
            if (completed && this.#element === element && !this.#preferences.snapshot.visible)
              this.#unmount();
          })
          .catch(this.#options.reportError);
      }
      return;
    }
    const container = view.element.parentElement;
    if (!container) return;
    const state = this.#state();
    view.setCubeOrder(state.rows.map((row) => `cube:${row.id}`));
    if (!this.#element) {
      this.#element = this.#options.document.createElement('aside');
      this.#element.className = 'sc-cube-stack-host';
      container.append(this.#element);
    }
    const appearance = this.#options.appearance.appearance;
    this.#element.dataset.appearance = state.appearance;
    this.#element.dataset.colorScheme = appearance.dark ? 'dark' : 'light';
    applyOwnedFluentTheme(
      this.#element,
      appearance.fluent || this.#options.frame?.enabled ? appearance.dark : null,
    );
    if (this.#mounted) this.#mounted.update(state);
    else {
      this.#mounted = mountCubeStackView(this.#element, state, this.#actions);
      this.#motion = new CubeStackDrawerMotion(this.#element);
      this.#mounted.setInteractive(false);
    }
    view.setStackWidth(
      this.#options.frame?.enabled ? (state.compact ? 58 : 224) : state.compact ? 82 : 248,
    );
    if (this.#surfaceVisible) void this.#present(true, animate).catch(this.#options.reportError);
    this.#scrollSelection?.refresh(state.rows.map((row) => row.id));
  }

  /** Derive all row facts from the current projection and exact canonical analysis snapshot. */
  #state(): CubeStackViewState {
    return {
      rows: projectCubeStackRows(
        this.#view?.sections ?? [],
        this.#reorder?.analysis ?? null,
        this.#selectedId,
        this.#options.resolveSource,
        this.#reorder?.available === true,
      ),
      compact: this.#preferences.snapshot.compact,
      pending: this.#reorder?.pending === true,
      appearance:
        this.#options.appearance.appearance.fluent || this.#options.frame?.enabled
          ? 'fluent'
          : 'comfy',
    };
  }

  /** Restore filtered sections through the search owner before scrolling to stable identity. */
  #navigate(instanceId: string): void {
    if (!this.#state().rows.some((row) => row.id === instanceId)) return;
    this.#options.revealSearchResults();
    this.#scrollSelection?.navigate(instanceId);
    this.#view?.scrollToSection(`cube:${instanceId}`);
    this.#reconcile();
  }

  /** Settle the accepted graph projection before Vue clears its temporary drag permutation. */
  async #mutate(work: (service: StackReorder) => Promise<void>): Promise<void> {
    const reorder = this.#reorder;
    if (!reorder) throw new Error('Cube graph reorder is unavailable.');
    await work(reorder);
    await reorder.refresh();
    await this.#options.refreshSurface();
    this.#reconcile();
  }

  /** Suspend captured drag intent while motion owns geometry and re-enable only the latest entry. */
  async #present(visible: boolean, animate = true): Promise<boolean> {
    const motion = this.#motion;
    const mounted = this.#mounted;
    if (!motion || !mounted) return false;
    if (!animate) {
      motion.restore(visible);
      mounted.setInteractive(visible);
      return true;
    }
    if (visible && motion.settledVisible) {
      mounted.setInteractive(true);
      return true;
    }
    if (!motion.canAnimate) mounted.setInteractive(visible);
    else mounted.setInteractive(false);
    const completed = await motion.setVisible(visible);
    if (completed && motion === this.#motion && visible) mounted.setInteractive(true);
    return completed;
  }

  /** Remove owned presentation while retaining preferences for the next eligible masonry surface. */
  #unmount(): void {
    this.#motion?.dispose();
    this.#motion = null;
    this.#view?.previewCubeOrder(null);
    this.#view?.setStackWidth(0);
    this.#mounted?.dispose();
    this.#mounted = null;
    this.#element?.remove();
    this.#element = null;
  }
}
