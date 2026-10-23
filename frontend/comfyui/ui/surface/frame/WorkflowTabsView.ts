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
/** Present stable Microsoft Fluent workflow tabs while Comfy retains workflow actions. */
import {
  createWorkflowTab,
  createWorkflowTablist,
  createButton,
  workflowTabAccessorySlot,
} from '../appearance/ControlFactory.js';
import { ContextMenu, type ContextMenuEntry } from '../../menus/ContextMenu.js';
import { WORKFLOW_TAB_WIDTH } from './WorkflowTabStyles.js';
import { scrollWheelDistance } from '../ScrollWheelDistance.js';
import { WorkflowTabPointerDrag } from './WorkflowTabPointerDrag.js';
import type { FrameWorkflowTab } from './ComfyFrameWorkflowAdapter.js';
import type { WorkflowSessionId } from '../../execution/domain/ExecutionIdentity.js';

export interface WorkflowTabsActions {
  activate(id: WorkflowSessionId): void;
  close(id: WorkflowSessionId): void;
  menu(id: WorkflowSessionId): Promise<readonly ContextMenuEntry[]>;
  reorder(from: number, to: number): void;
  create(): void;
  report(error: unknown): void;
}
interface TabView {
  readonly element: HTMLElement;
  readonly title: HTMLElement;
  readonly close: HTMLElement;
  unread: boolean;
}

/** Retain keyed controls, literal titles, native keyboard navigation and one-shot activity animation. */
export class WorkflowTabsView {
  readonly element: HTMLElement & { activeid: string };
  readonly #tabs = new Map<WorkflowSessionId, TabView>();
  readonly #menu: ContextMenu;
  #order: readonly FrameWorkflowTab[] = [];
  readonly #drag: WorkflowTabPointerDrag;
  #projecting = false;
  #selection = '';
  #tabWidth = WORKFLOW_TAB_WIDTH;
  #menuRequest = 0;
  #menuTarget: WorkflowSessionId | null = null;
  #paintKey = '';
  #orbVisible = true;

  /** Bind interaction intents without retaining host workflow objects in presentation. */
  constructor(
    private readonly document: Document,
    private readonly actions: WorkflowTabsActions,
    private readonly viewport: HTMLElement,
  ) {
    this.element = createWorkflowTablist(document);
    this.#menu = new ContextMenu(document);
    this.#drag = new WorkflowTabPointerDrag(
      viewport,
      this.element,
      (from, to) => actions.reorder(from, to),
      (error) => actions.report(error),
      () => this.#updateOrbAdjacency(),
      () => this.#tabWidth,
    );
    this.element.setAttribute('aria-label', 'Workflows');
    this.viewport.addEventListener('scroll', this.#scrolled);
    this.element.addEventListener('change', () => {
      if (this.#projecting) return;
      const selected = [...this.#tabs].find(([, tab]) => tab.element.id === this.element.activeid);
      if (selected && selected[0] !== this.#order.find((tab) => tab.selected)?.id)
        this.#run(() => actions.activate(selected[0]));
    });
    this.viewport.addEventListener('wheel', this.#wheel, { passive: false });
  }

  /** Update values in place and animate only a newly accepted unread transition. */
  update(
    tabs: readonly FrameWorkflowTab[],
    unread: (id: WorkflowSessionId) => boolean,
    animate = true,
  ): void {
    const activity = tabs.map((tab) => unread(tab.id) && !tab.selected);
    const paintKey = JSON.stringify([tabs, activity]);
    if (paintKey === this.#paintKey) return;
    this.#paintKey = paintKey;
    const positions = this.#drag.beforeUpdate(tabs.map((tab) => `sc-workflow-${tab.id}`));
    this.#projecting = true;
    this.#order = tabs;
    const open = new Set(tabs.map((tab) => tab.id));
    if (this.#menuTarget && !open.has(this.#menuTarget)) {
      this.#menuRequest++;
      this.#menuTarget = null;
      this.#menu.close();
    }
    for (const [id, view] of this.#tabs)
      if (!open.has(id)) {
        view.element.remove();
        this.#tabs.delete(id);
      }
    for (const [index, tab] of tabs.entries()) {
      let view = this.#tabs.get(tab.id);
      if (!view) {
        view = this.#create(tab.id);
        this.#tabs.set(tab.id, view);
      }
      if (view.title.textContent !== tab.title) view.title.textContent = tab.title;
      view.element.title = tab.title;
      view.element.toggleAttribute('data-modified', tab.modified);
      const nextUnread = activity[index] ?? false;
      view.element.toggleAttribute('data-unread', nextUnread);
      if (nextUnread && !view.unread && animate) view.element.setAttribute('data-shimmer', '');
      if (!nextUnread) view.element.removeAttribute('data-shimmer');
      view.unread = nextUnread;
      const current = this.element.children.item(index);
      if (current !== view.element) this.element.insertBefore(view.element, current);
      if (tab.selected) this.element.activeid = view.element.id;
    }
    this.#projecting = false;
    this.#drag.afterUpdate(
      [...this.element.children].filter(
        (element): element is HTMLElement => element instanceof HTMLElement,
      ),
      positions,
    );
    const selected = tabs.findIndex((tab) => tab.selected);
    const selection = `${tabs[selected]?.id ?? ''}:${String(selected)}`;
    if (selection !== this.#selection) {
      this.#selection = selection;
      if (!this.#drag.active) this.#revealSelected(selected);
    }
    this.#updateOrbAdjacency();
  }

  /** Reveal newly selected or reordered tabs without changing scroll position on activity refresh. */
  #revealSelected(index: number): void {
    const width = this.viewport.clientWidth;
    if (index < 0 || width <= 0) return;
    const left = index * this.#tabWidth;
    const right = left + this.#tabWidth;
    if (left < this.viewport.scrollLeft) this.viewport.scrollLeft = left;
    else if (right > this.viewport.scrollLeft + width)
      this.viewport.scrollLeft = Math.max(0, right - width);
  }

  /** Keep selection and the orb cutout aligned after the enclosing strip changes width. */
  resize(width: number): void {
    this.#drag.cancelForResize();
    this.#tabWidth = width;
    if (!this.#drag.active) this.#revealSelected(this.#order.findIndex((tab) => tab.selected));
    this.#updateOrbAdjacency();
  }

  /** Retire orb geometry in Plugin Mode while keeping the same tab and drag owners. */
  setOrbVisible(visible: boolean): void {
    if (this.#orbVisible === visible) return;
    this.#drag.cancelForResize();
    this.#orbVisible = visible;
    this.viewport.toggleAttribute('data-sc-orb-hidden', !visible);
    this.#updateOrbAdjacency();
  }

  /** Preserve captured pointer alignment when endpoint arrows change the viewport allocation. */
  refreshScrollLayout(): void {
    this.#drag.viewportChanged();
    this.#updateOrbAdjacency();
  }

  /** Project scroll position without rebuilding controls or changing selected workflow policy. */
  readonly #scrolled = (): void => this.#updateOrbAdjacency();

  /** Scroll horizontally using either wheel axis and normalize line and page deltas. */
  readonly #wheel = (event: WheelEvent): void => {
    if (this.viewport.scrollWidth <= this.viewport.clientWidth) return;
    event.preventDefault();
    this.viewport.scrollLeft += scrollWheelDistance(event, 'horizontal', this.viewport.clientWidth);
  };

  /** Keep the cutout aligned to the orb when the uniform-width tab strip scrolls or reorders. */
  #updateOrbAdjacency(): void {
    if (this.#drag.active) return;
    const index = Math.floor(Math.max(0, this.viewport.scrollLeft) / this.#tabWidth);
    const adjacent =
      !this.#orbVisible || this.viewport.hasAttribute('data-sc-overflow')
        ? undefined
        : this.#order[index]?.id;
    for (const [id, tab] of this.#tabs) {
      tab.element.toggleAttribute('data-orb-adjacent', id === adjacent);
      if (id === adjacent)
        tab.element.style.setProperty(
          '--sc-tab-cutout-x',
          `${String(-17 + this.viewport.scrollLeft - index * this.#tabWidth)}px`,
        );
    }
  }

  /** Release floating interaction and official Fluent focus bindings through normal disconnection. */
  dispose(): void {
    this.#menuRequest++;
    this.#drag.dispose();
    this.#menu.close();
    this.viewport.removeEventListener('scroll', this.#scrolled);
    this.viewport.removeEventListener('wheel', this.#wheel);
    this.element.remove();
    this.#tabs.clear();
  }

  /** Create one official tab with a reserved close slot and literal title content. */
  #create(id: WorkflowSessionId): TabView {
    const element = createWorkflowTab(this.document);
    element.id = `sc-workflow-${id}`;
    element.setAttribute('value', element.id);
    element.draggable = false;
    const title = this.document.createElement('span');
    title.className = 'sc-workflow-title';
    const shimmer = this.document.createElement('span');
    shimmer.className = 'sc-workflow-shimmer';
    shimmer.slot = workflowTabAccessorySlot(this.document, 'start');
    shimmer.setAttribute('aria-hidden', 'true');
    const close = createButton(this.document);
    close.className = 'sc-workflow-close';
    close.slot = workflowTabAccessorySlot(this.document, 'end');
    close.textContent = '×';
    close.setAttribute('appearance', 'subtle');
    close.setAttribute('aria-label', 'Close workflow');
    close.addEventListener('click', (event) => {
      event.stopPropagation();
      this.#run(() => this.actions.close(id));
    });
    close.addEventListener('focusin', (event) => event.stopPropagation());
    // Fluent selects a tab on focus; a context press must preserve the active workflow.
    for (const type of ['pointerdown', 'mousedown'])
      element.addEventListener(type, (event) => {
        if (event instanceof MouseEvent && event.button === 2) {
          event.preventDefault();
          event.stopPropagation();
        }
      });
    // Match Comfy's mouse-release boundary; embedded hosts may omit auxclick.
    element.addEventListener('mouseup', (event) => {
      if (event.button === 1) {
        event.preventDefault();
        this.#run(() => this.actions.close(id));
      }
    });
    element.addEventListener('contextmenu', (event) => {
      event.preventDefault();
      event.stopPropagation();
      void this.#openMenu(id, element, event);
    });
    element.addEventListener('keydown', (event) => {
      if (event.altKey && ['ArrowLeft', 'ArrowRight'].includes(event.key)) {
        event.preventDefault();
        event.stopPropagation();
        const from = this.#order.findIndex((tab) => tab.id === id);
        const to = from + (event.key === 'ArrowLeft' ? -1 : 1);
        if (to >= 0 && to < this.#order.length) this.#run(() => this.actions.reorder(from, to));
      }
    });
    element.addEventListener('animationend', () => element.removeAttribute('data-shimmer'));
    element.addEventListener('animationcancel', () => element.removeAttribute('data-shimmer'));
    element.append(shimmer, title, close);
    return { element, title, close, unread: false };
  }

  /** Keep shared dismissal authoritative while Comfy lazily publishes the requested tab's actions. */
  async #openMenu(id: WorkflowSessionId, element: HTMLElement, event: MouseEvent): Promise<void> {
    const request = ++this.#menuRequest;
    this.#menuTarget = id;
    const options = {
      label: 'Workflow actions',
      trigger: element,
      focusReturn: this.document.getElementById(this.element.activeid),
      returnFocusOnSelect: false,
      anchor: { kind: 'pointer' as const, x: event.clientX, y: event.clientY },
    };
    this.#menu.open({
      ...options,
      entries: [{ label: 'Loading workflow actions…', disabled: true }],
    });
    try {
      const entries = await this.actions.menu(id);
      if (request === this.#menuRequest && element.isConnected && this.#menu.isOpen())
        this.#menu.updateEntries(entries);
    } catch (error: unknown) {
      if (request === this.#menuRequest) this.#menu.close();
      this.actions.report(error);
    }
  }

  /** Report host action failure without resetting any workflow or canvas state. */
  #run(action: () => void): void {
    try {
      action();
    } catch (error: unknown) {
      this.actions.report(error);
    }
  }
}
