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
/** Adapt native editor breadcrumbs without inventing a second navigation stack. */
import type { ComfyApplication } from '../types/graph.js';
import { workflowEditorNavigationCss } from './WorkflowEditorNavigationStyles.js';
import { HostSurfaceObserver } from '../core/HostSurfaceObserver.js';
import type { AppearanceSource } from './appearance/AppearanceState.js';
import {
  WorkflowEditorNavigationView,
  type EditorBreadcrumb,
} from './WorkflowEditorNavigationView.js';

interface EditorNavigationOptions {
  readonly document: Document;
  readonly app: ComfyApplication | null;
  readonly appearance: AppearanceSource;
  suspendOverrides(suspended: boolean): void;
  report(error: unknown): void;
}

/** Project native editor navigation into the override row while preserving user override preferences. */
export class WorkflowEditorNavigationHost {
  readonly #view: WorkflowEditorNavigationView;
  readonly #style: HTMLStyleElement;
  readonly #marked = new Set<HTMLElement>();
  #presence: HostSurfaceObserver | null = null;
  #stopAppearance: (() => void) | null = null;
  #disposed = false;
  #pin: { element: HTMLElement; disabled: string | null; title: string | null } | null = null;

  /** Keep native commands authoritative and isolate failures behind product feedback. */
  constructor(private readonly options: EditorNavigationOptions) {
    this.#view = new WorkflowEditorNavigationView(options.document, () =>
      this.#run(() => {
        const back = options.document.querySelector<HTMLElement>(
          '[data-testid="subgraph-breadcrumb-back"]',
        );
        if (!back) throw new Error('Comfy parent graph navigation is unavailable.');
        back.click();
      }),
    );
    this.#style = options.document.createElement('style');
    this.#style.textContent = workflowEditorNavigationCss;
  }

  /** Observe only native breadcrumbs and the two toolbar mounts, independently of graph inventory. */
  setup(): void {
    if (this.#presence || this.#disposed) return;
    const { document } = this.options;
    document.head.append(this.#style);
    this.#presence = new HostSurfaceObserver(
      document.body,
      '.subgraph-breadcrumb,.actionbar-container,.sc-orb-actions',
      () => this.#reconcile(),
      {
        content: true,
        characterData: true,
        attributes: ['data-active'],
        exclude: '.sc-editor-navigation,.sc-overrides-toolbar',
      },
    );
    this.#stopAppearance = this.options.appearance.subscribeAppearance(() => this.#reconcile());
    document.defaultView?.addEventListener('pagehide', this.dispose);
    this.#reconcile();
  }

  /** Restore native presentation and release all owned observation and Fluent controls. */
  readonly dispose = (): void => {
    this.#disposed = true;
    this.#presence?.disconnect();
    this.#stopAppearance?.();
    this.#restoreNative();
    this.#updatePin(null);
    this.#view.dispose();
    this.#style.remove();
    this.options.suspendOverrides(false);
    this.options.document.defaultView?.removeEventListener('pagehide', this.dispose);
  };

  /** Read ancestry from Comfy's mounted navigation surface and delegate all transitions back to it. */
  #reconcile(): void {
    if (this.#disposed) return;
    const { document, app } = this.options;
    const nested = !!app?.canvas?.graph && app.canvas.graph !== app.graph;
    this.options.suspendOverrides(nested);
    this.#updatePin(
      nested
        ? document.querySelector<HTMLElement>('.sc-orb-actions [aria-label="Set Global Override"]')
        : null,
    );
    const bar = document.querySelector<HTMLElement>('.actionbar-container');
    const native = document.querySelector<HTMLElement>('.subgraph-breadcrumb');
    const back = native?.querySelector<HTMLElement>('[data-testid="subgraph-breadcrumb-back"]');
    if (!nested || !bar || !native || !back) {
      this.#restoreNative();
      this.#view.clear();
      return;
    }
    const entries = [
      ...native.querySelectorAll<HTMLElement>('[data-testid^="subgraph-breadcrumb-item-"]'),
    ];
    const items: EditorBreadcrumb[] = entries.map((entry, index) => ({
      label:
        entry.querySelector('.p-breadcrumb-item-label')?.textContent ?? entry.textContent ?? '',
      current: index === entries.length - 1,
      select: () =>
        this.#run(() => {
          if (!entry.isConnected) throw new Error('Comfy editor path changed; retry navigation.');
          const MouseEventType = document.defaultView?.MouseEvent;
          if (!MouseEventType) throw new Error('Comfy breadcrumb input is unavailable.');
          // Comfy distinguishes single clicks (detail 1) from double-click rename and synthetic detail 0.
          entry.dispatchEvent(
            new MouseEventType('click', { bubbles: true, cancelable: true, detail: 1 }),
          );
        }),
    }));
    this.#view.update(items, this.options.appearance.appearance.dark);
    if (this.#view.element.parentElement !== bar) bar.prepend(this.#view.element);
    const marked = new Set<HTMLElement>([back]);
    const nativeTrail = native.querySelector<HTMLElement>('nav');
    if (nativeTrail) marked.add(nativeTrail);
    this.#restoreNative(marked);
    for (const element of marked) this.#hide(element);
  }

  /** Lease Fluent's disabled state while keeping the pin in its original orb slot. */
  #updatePin(element: HTMLElement | null): void {
    if (this.#pin?.element === element) return;
    if (this.#pin) {
      const previous = this.#pin;
      for (const [attribute, value] of [
        ['disabled', previous.disabled],
        ['title', previous.title],
      ] as const) {
        if (value === null) previous.element.removeAttribute(attribute);
        else previous.element.setAttribute(attribute, value);
      }
      this.#pin = null;
    }
    if (!element) return;
    this.#pin = {
      element,
      disabled: element.getAttribute('disabled'),
      title: element.getAttribute('title'),
    };
    element.setAttribute('disabled', '');
    element.title = 'Global overrides are available in the top-level workflow';
  }

  /** Mark borrowed controls reversibly rather than modifying their native hidden or disabled state. */
  #hide(element: HTMLElement): void {
    if (!element.hasAttribute('data-sc-editor-native'))
      element.setAttribute('data-sc-editor-native', '');
    this.#marked.add(element);
  }

  /** Release presentation marks when the root workflow or native frame replaces its surface. */
  #restoreNative(retain: ReadonlySet<HTMLElement> = new Set()): void {
    for (const element of this.#marked) {
      if (retain.has(element)) continue;
      element.removeAttribute('data-sc-editor-native');
      this.#marked.delete(element);
    }
  }

  /** Report failed host navigation without changing workflow or override preferences. */
  #run(action: () => void): void {
    try {
      action();
    } catch (error: unknown) {
      this.options.report(error);
    }
  }
}
