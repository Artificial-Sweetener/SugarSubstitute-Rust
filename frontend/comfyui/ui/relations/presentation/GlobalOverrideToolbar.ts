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

/** Select host chrome without changing native widgets or workflow relation state. */
export type OverrideToolbarPresentation = 'comfy' | 'substitute';

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
  #signature = '';

  /** Bind toolbar edits directly to the authoritative application owner. */
  constructor(
    private readonly document: Document,
    private readonly relations: WorkflowRelations,
    private readonly reportError: (error: unknown) => void,
    private readonly widgets: RelationWidgetRenderer,
    private readonly attach: (element: HTMLElement) => boolean,
    presentation: OverrideToolbarPresentation = 'comfy',
  ) {
    this.element = document.createElement('div');
    this.element.className = 'sc-overrides-toolbar';
    this.element.setAttribute('role', 'toolbar');
    this.element.setAttribute('aria-label', 'Global overrides');
    this.setPresentation(presentation);
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
    this.#menuButton.addEventListener('click', () => {
      if (this.#menu.isOpen()) {
        this.#menu.close();
        return;
      }
      this.#menuButton.setAttribute('aria-expanded', 'true');
      this.#menu.open({
        label: 'Set Global Override',
        trigger: this.#menuButton,
        anchor: { kind: 'below', bounds: this.#menuButton.getBoundingClientRect() },
        closed: () => this.#menuButton.setAttribute('aria-expanded', 'false'),
        entries: relations.snapshot().groups.map((group) => ({
          label: group.label,
          checked: group.enabled,
          selection: 'multiple',
          select: () => this.#run(() => relations.setOverrideEnabled(group.key, !group.enabled)),
        })),
      });
    });
    this.element.append(this.#menuButton);
    this.#labels = new MutationObserver(this.#measureLabels);
    this.#labels.observe(this.element, { childList: true, subtree: true, characterData: true });
  }

  /** Let an embedding host select its chrome while retaining mounted controls and menu identity. */
  setPresentation(presentation: OverrideToolbarPresentation): void {
    this.element.dataset.presentation = presentation;
  }

  /** Reattach after native shell replacement and update only changed controls. */
  reconcile(): void {
    if (!this.attach(this.element)) return;
    const groups = this.relations.snapshot().groups;
    this.element.hidden = groups.length === 0;
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
    let preceding: Element = this.#menuButton;
    for (const group of groups.filter((item) => item.enabled)) {
      const root = this.#controls.get(group.key)?.root;
      if (!root) continue;
      if (root.previousElementSibling !== preceding)
        this.element.insertBefore(root, preceding.nextSibling);
      preceding = root;
    }
  }

  /** Release menus and owned toolbar DOM without modifying workflow state. */
  dispose(): void {
    this.#labels.disconnect();
    for (const control of this.#controls.values()) control.dispose();
    this.#controls.clear();
    this.#menu.close();
    this.element.remove();
  }

  /** Keep label width fixed while each asynchronously mounted native control surrenders its surplus. */
  readonly #measureLabels = (): void => {
    for (const control of this.#controls.values()) {
      const label = control.root.querySelector<HTMLElement>(
        '[data-testid="widget-layout-field-label"]',
      );
      if (label)
        control.root.style.setProperty(
          '--sc-override-label-width',
          `${label.getBoundingClientRect().width}px`,
        );
    }
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
