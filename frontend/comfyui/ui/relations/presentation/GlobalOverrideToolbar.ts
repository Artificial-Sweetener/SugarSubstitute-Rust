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
/** Render one persistent toolbar across graph, masonry, and native renderer changes. */
import type { WorkflowRelations } from '../application/WorkflowRelations.js';
import type { OverrideGroup } from '../domain/RelationContracts.js';
import { ContextMenu } from '../../menus/ContextMenu.js';
import type { RelationWidgetRenderer } from './RelationWidgetRenderer.js';
import {
  OverrideToolbarDisclosure,
  type OverrideToolbarDisclosureStore,
} from './OverrideToolbarDisclosure.js';
import { OverrideToolbarDrag } from './OverrideToolbarDrag.js';
import { OverrideToolbarPlacement } from './OverrideToolbarPlacement.js';
import type { OverrideToolbarHost } from './OverrideToolbarPlacement.js';
import type { OverrideToolbarPlacementStore } from './OverrideToolbarPlacementState.js';
import { OverrideToolbarFieldLayout } from './OverrideToolbarFieldLayout.js';

/** Select host chrome without changing native widgets or workflow relation state. */
export type OverrideToolbarPresentation = 'comfy' | 'substitute' | 'plugin';

/** Keep toolbar controls stable while reconciling automatically discovered participant groups. */
export class GlobalOverrideToolbar {
  readonly element: HTMLElement;
  readonly #controls = new Map<
    string,
    { root: HTMLElement; update(group: OverrideGroup): void; dispose(): void }
  >();
  readonly #menu: ContextMenu;
  readonly #menuButton: HTMLButtonElement;
  readonly #labels: MutationObserver;
  readonly #fields: HTMLElement;
  readonly #fieldLayout: OverrideToolbarFieldLayout;
  readonly #disclosure: OverrideToolbarDisclosure;
  readonly #drag: OverrideToolbarDrag;
  readonly #placement: OverrideToolbarPlacement;
  readonly #title: HTMLHeadingElement;
  #signature = '';
  #suspended = false;

  /** Bind toolbar edits directly to the authoritative application owner. */
  constructor(
    private readonly document: Document,
    private readonly relations: WorkflowRelations,
    private readonly reportError: (error: unknown) => void,
    private readonly widgets: RelationWidgetRenderer,
    host: OverrideToolbarHost,
    stateStore: OverrideToolbarPlacementStore & OverrideToolbarDisclosureStore,
    presentation: OverrideToolbarPresentation = 'comfy',
  ) {
    this.element = document.createElement('div');
    this.element.className = 'sc-overrides-toolbar';
    this.element.setAttribute('role', 'toolbar');
    this.element.setAttribute('aria-label', 'Global overrides');
    this.#menu = new ContextMenu(document);
    this.#menuButton = document.createElement('button');
    this.#menuButton.type = 'button';
    this.#menuButton.className = 'sc-overrides-menu';
    this.#menuButton.setAttribute('aria-label', 'Set Global Override');
    this.#menuButton.title = 'Set Global Override';
    this.#menuButton.setAttribute('aria-haspopup', 'menu');
    this.#menuButton.setAttribute('aria-expanded', 'false');
    const icon = document.createElement('span');
    icon.className = 'icon-[lucide--pin]';
    icon.setAttribute('aria-hidden', 'true');
    const chevron = document.createElement('span');
    chevron.className = 'icon-[lucide--chevron-down]';
    chevron.setAttribute('aria-hidden', 'true');
    this.#menuButton.append(icon, chevron);
    this.#menuButton.addEventListener('click', () => this.openMenu(this.#menuButton));
    const grip = document.createElement('button');
    grip.type = 'button';
    grip.className = 'sc-overrides-grip';
    grip.setAttribute('aria-label', 'Move global overrides');
    grip.title =
      'Drag onto the Run dock target; Enter toggles docking; arrow keys move, Home resets';
    const gripIcon = document.createElement('span');
    gripIcon.className = 'drag-handle h-max w-3 cursor-grab';
    gripIcon.setAttribute('aria-hidden', 'true');
    grip.append(gripIcon);
    this.#fields = document.createElement('div');
    this.#fields.className = 'sc-overrides-fields';
    this.#fields.dataset.scScrollbarHorizontalOutset = '3';
    this.#fieldLayout = new OverrideToolbarFieldLayout(this.#fields);
    this.#fields.id = 'sc-overrides-fields';
    this.#title = document.createElement('h3');
    this.#title.className = 'sc-overrides-card-title';
    this.#title.textContent = 'Global Overrides';
    const disclosure = document.createElement('button');
    disclosure.type = 'button';
    disclosure.className = 'sc-overrides-disclosure';
    disclosure.setAttribute('aria-controls', this.#fields.id);
    disclosure.addEventListener('click', () => {
      this.#menu.close();
      queueMicrotask(this.#measureLabels);
    });
    this.element.append(grip, this.#menuButton, this.#title, this.#fields, disclosure);
    this.#placement = new OverrideToolbarPlacement(
      this.element,
      grip,
      host,
      (docked) => this.#disclosure.setExpansionDirection(docked ? 'left' : 'right'),
      () => this.#fieldLayout.reconcile(),
      stateStore,
    );
    this.#drag = new OverrideToolbarDrag(this.element, grip, this.#placement, () => {
      this.#menu.close();
      this.#disclosure.finish();
    });
    this.#disclosure = new OverrideToolbarDisclosure(
      this.element,
      this.#fields,
      disclosure,
      (width, height) => {
        this.#fieldLayout.reconcile();
        this.#placement.resize(width, height);
      },
      reportError,
      stateStore,
    );
    this.#labels = new MutationObserver(this.#measureLabels);
    this.#labels.observe(this.element, { childList: true, subtree: true, characterData: true });
    this.setPresentation(presentation);
  }

  /** Anchor the same override menu to either native toolbar or Substitute frame controls. */
  openMenu(trigger: HTMLElement): void {
    if (this.#suspended) return;
    if (this.#menu.isOpen()) {
      this.#menu.close();
      return;
    }
    trigger.setAttribute('aria-expanded', 'true');
    this.#menu.open({
      label: 'Set Global Override',
      trigger,
      anchor: { kind: 'below', bounds: trigger.getBoundingClientRect() },
      closed: () => trigger.setAttribute('aria-expanded', 'false'),
      entries: this.relations.snapshot().groups.map((group) => ({
        label: group.label,
        checked: group.enabled,
        selection: 'multiple',
        select: () => this.#run(() => this.relations.setOverrideEnabled(group.key, !group.enabled)),
      })),
    });
  }

  /** Let an embedding host select its chrome while retaining mounted controls and menu identity. */
  setPresentation(presentation: OverrideToolbarPresentation): void {
    if (this.element.dataset.presentation === presentation) return;
    this.element.dataset.presentation = presentation;
    const plugin = presentation === 'plugin';
    const inline = presentation !== 'comfy';
    this.element.classList.toggle('sugarcubes-cube-face__node-card', plugin);
    this.element.setAttribute('role', plugin ? 'group' : 'toolbar');
    this.#title.hidden = !plugin;
    this.#disclosure.setInline(inline);
    this.#placement.setInline(inline);
    this.#fieldLayout.reconcile();
  }

  /** Hide workflow-wide controls during nested editing without changing saved values or placement. */
  setSuspended(suspended: boolean): void {
    if (this.#suspended === suspended) return;
    this.#suspended = suspended;
    if (suspended) this.#menu.close();
    this.reconcile();
  }

  /** Reattach after native shell replacement and update only changed controls. */
  reconcile(): void {
    if (!this.#placement.attached()) return;
    this.#disclosure.refresh();
    const groups = this.relations.snapshot().groups;
    this.element.hidden =
      this.#suspended || (groups.length === 0 && this.element.dataset.presentation !== 'plugin');
    const signature = JSON.stringify(groups);
    if (signature === this.#signature) return;
    this.#signature = signature;
    for (const [key, control] of this.#controls)
      if (!groups.some((group) => group.key === key && group.enabled)) {
        control.dispose();
        this.#controls.delete(key);
      }
    for (const group of groups.filter((item) => item.enabled)) {
      let control = this.#controls.get(group.key);
      if (!control) {
        control = this.#createControl(group);
        this.#controls.set(group.key, control);
      }
      control.update(group);
    }
    let preceding: Element | null = null;
    for (const group of groups.filter((item) => item.enabled)) {
      const root = this.#controls.get(group.key)?.root;
      if (!root) continue;
      if (root.parentElement !== this.#fields || root.previousElementSibling !== preceding)
        this.#fields.insertBefore(
          root,
          preceding ? preceding.nextSibling : this.#fields.firstChild,
        );
      preceding = root;
    }
  }

  /** Release menus and owned toolbar DOM without modifying workflow state. */
  dispose(): void {
    this.#labels.disconnect();
    this.#drag.dispose();
    this.#placement.dispose();
    this.#disclosure.dispose();
    for (const control of this.#controls.values()) control.dispose();
    this.#controls.clear();
    this.#menu.close();
    this.element.remove();
  }

  /** Measure intrinsic layout widths so graph zoom never changes field wrapping. */
  readonly #measureLabels = (): void => {
    for (const control of this.#controls.values()) {
      const label = control.root.querySelector<HTMLElement>(
        '[data-testid="widget-layout-field-label"]',
      );
      const width = label?.offsetWidth ?? 0;
      if (width > 0) control.root.style.setProperty('--sc-override-label-width', `${width}px`);
    }
    this.#disclosure.refresh();
  };

  /** Mount the same native widget component used on node cards against the shared value owner. */
  #createControl(group: OverrideGroup): {
    root: HTMLElement;
    update(group: OverrideGroup): void;
    dispose(): void;
  } {
    const root = this.document.createElement('div');
    root.className = 'sc-override-control';
    root.dataset.semantic = group.members[0]?.semantic ?? '';
    root.dataset.overrideKey = group.key;
    const mount = this.widgets.mount(
      root,
      group,
      (value) => this.#run(() => this.relations.setOverrideValue(group.key, value)),
      (mode) => this.#run(() => this.relations.setSeedMode(group.key, mode)),
    );
    return {
      root,
      update: (next) => {
        root.title = `${next.members.length} fields share this value`;
        mount.update(next);
      },
      dispose: () => {
        mount.dispose();
        root.remove();
      },
    };
  }

  /** Report invalid edits through existing SugarCubes feedback and restore validated presentation. */
  #run(operation: () => void): void {
    try {
      operation();
    } catch (error: unknown) {
      this.reportError(error);
      this.#signature = '';
      this.reconcile();
    }
  }
}
