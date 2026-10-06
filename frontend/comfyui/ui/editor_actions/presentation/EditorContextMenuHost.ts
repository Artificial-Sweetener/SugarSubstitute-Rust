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
/** Own editor menu entry-point lifetimes while delegating values, policies and persistence. */
import { ContextMenu } from '../../menus/ContextMenu.js';
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import type { ContextMenuEntry } from '../../menus/ContextMenu.js';
import { inferDimensionPairs } from '../domain/DimensionPolicy.js';
import type { PresetAssociation } from '../domain/UserPreset.js';
import type { UserPresetLibrary } from '../application/UserPresetLibrary.js';
import {
  readNativeEditorCard,
  projectedEditorCardMount,
} from '../infrastructure/NativeEditorCardAdapter.js';
import type { NativeEditorCard } from '../infrastructure/NativeEditorCardAdapter.js';
import { SaveNodePresetDialog } from './SaveNodePresetDialog.js';
import { nodeActionEntries } from './NodeActionMenu.js';
import { dimensionMenuEntries } from './DimensionMenu.js';
import { dimensionFieldTarget } from './DimensionFieldTarget.js';
import type { ModelContextContribution } from '../../model_library/ModelContextProjection.js';
export interface EditorContextMenuOptions {
  readonly document: Document;
  readonly library: UserPresetLibrary;
  readonly families: (card: NativeEditorCard) => readonly PresetAssociation[];
  readonly reportError: (error: unknown) => void;
  readonly skipped: (names: readonly string[]) => void;
  readonly readCard?: (root: HTMLElement) => NativeEditorCard | null;
  readonly enabled?: () => boolean;
  readonly modelContributions?: (root: HTMLElement) => readonly ModelContextContribution[];
}
interface CardMenu {
  readonly root: HTMLElement;
  card: NativeEditorCard;
}

/** Use one native-card owner for both SugarCube faces and Workflow Masonry. */
export class EditorContextMenuHost {
  readonly #options: EditorContextMenuOptions;
  readonly #menu: ContextMenu;
  readonly #dialog: SaveNodePresetDialog;
  readonly #cards = new Map<HTMLElement, CardMenu>();
  #presence: HostSurfaceObserver | null = null;
  readonly #dirty = new Set<HTMLElement>();
  #queued = false;
  #active = false;
  #menuRoot: HTMLElement | null = null;
  /** Prepare delegated owners without touching cards until setup completes. */
  constructor(options: EditorContextMenuOptions) {
    this.#options = options;
    this.#menu = new ContextMenu(options.document);
    this.#dialog = new SaveNodePresetDialog(options.document);
  }
  /** Observe native card mounts and intercept dimension menus before embedded event isolation. */
  setup(): void {
    if (this.#active) return;
    this.#active = true;
    this.#presence = new HostSurfaceObserver(
      this.#options.document.body,
      '.lg-node',
      (changes) => {
        for (const { element } of changes) this.#dirty.add(element);
        this.#schedule();
      },
      { content: true, exclude: '[data-sc-floating-surface],.sugarcubes-modal-overlay' },
    );
    this.#options.document.addEventListener('contextmenu', this.#context, true);
    this.refresh();
  }
  /** Follow renderer switches without retaining detached or unavailable native controls. */
  refresh(): void {
    this.#queue(this.#options.document.body);
    this.#schedule();
  }
  /** Expose prepared live card boundaries for context projection without rediscovering Vue trees. */
  preparedCards(): readonly NativeEditorCard[] {
    return [...this.#cards.values()].map((record) => record.card);
  }
  /** Remove every owned listener, menu and modal on shutdown. */
  dispose(): void {
    this.#active = false;
    this.#presence?.disconnect();
    this.#options.document.removeEventListener('contextmenu', this.#context, true);
    this.#menu.close();
    this.#dialog.close();
    this.#cards.clear();
    this.#dirty.clear();
  }
  /** Collect only the nearest card and newly introduced native roots. */
  #queue(element: HTMLElement): void {
    const own = element.closest<HTMLElement>('.lg-node');
    if (own) this.#dirty.add(own);
    for (const root of element.querySelectorAll<HTMLElement>('.lg-node')) this.#dirty.add(root);
  }
  /** Coalesce structural observation; menu contents are rebuilt only on an explicit open gesture. */
  #schedule(): void {
    if (this.#queued || !this.#active) return;
    this.#queued = true;
    queueMicrotask(() => {
      this.#queued = false;
      if (!this.#active) return;
      try {
        const enabled = this.#options.enabled?.() ?? true;
        if (
          !enabled ||
          (this.#menuRoot &&
            (!this.#menuRoot.isConnected || !projectedEditorCardMount(this.#menuRoot)))
        ) {
          this.#menu.close();
          this.#dialog.close();
          this.#menuRoot = null;
        }
        for (const root of this.#cards.keys())
          if (!enabled || !root.isConnected) this.#cards.delete(root);
        if (enabled) for (const root of this.#dirty) if (root.isConnected) this.#bind(root);
      } catch (error: unknown) {
        this.#options.reportError(error);
      }
      this.#dirty.clear();
    });
  }
  /** Retain recognized native cards without inserting presentation controls. */
  #bind(root: HTMLElement): void {
    const card = this.#read(root);
    if (!card?.header) {
      this.#cards.delete(root);
      return;
    }
    const existing = this.#cards.get(root);
    if (existing) {
      existing.card = card;
      return;
    }
    this.#cards.set(root, { root, card });
  }
  /** Resolve the card again at command entry so replaced controls never retain stale value bindings. */
  #read(root: HTMLElement): NativeEditorCard | null {
    if (!projectedEditorCardMount(root)) return null;
    return (this.#options.readCard ?? readNativeEditorCard)(root);
  }
  /** Compose available commands through their authoritative policy owners. */
  #entries(card: NativeEditorCard): ContextMenuEntry[] {
    return nodeActionEntries(
      card,
      this.#options.library,
      this.#options.families(card),
      this.#dialog,
      this.#run,
      this.#options.skipped,
      this.#options.modelContributions?.(card.root) ?? [],
    );
  }
  /** Route exact numeric pair fields while retaining model and prompt-specific menu ownership. */
  readonly #context = (event: MouseEvent): void => {
    if (!(event.target instanceof Element) || this.#options.enabled?.() === false) return;
    const root = event.target.closest<HTMLElement>('.lg-node');
    if (!root) return;
    const card = this.#read(root);
    if (!card) return;
    if (card.header?.contains(event.target)) {
      const entries = this.#entries(card);
      if (!entries.length) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      this.#menuRoot = root;
      this.#menu.open({
        label: `${card.title} node actions`,
        entries,
        anchor: { kind: 'pointer', x: event.clientX, y: event.clientY },
        trigger: card.header,
      });
      return;
    }
    const field = dimensionFieldTarget(
      root,
      card.fields,
      event.target,
      event.clientX,
      event.clientY,
    );
    const pair = field
      ? inferDimensionPairs(card.fields.map((field) => field.name)).find(
          (pair) => pair.width === field.name || pair.height === field.name,
        )
      : null;
    if (!pair || field?.kind !== 'number') return;
    const width = card.fields.find((field) => field.name === pair.width),
      height = card.fields.find((field) => field.name === pair.height);
    if (!width || !height || width.kind !== 'number' || height.kind !== 'number') return;
    const entries = dimensionMenuEntries(
      { width, height },
      field.name === pair.width ? 'width' : 'height',
      this.#options.library,
      this.#options.families(card),
      this.#run,
    );
    if (!entries.length) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    this.#menuRoot = root;
    this.#menu.open({
      label: 'Dimensions',
      entries,
      anchor: { kind: 'pointer', x: event.clientX, y: event.clientY },
      trigger: field.element,
    });
  };
  /** Route persistence failures through existing non-blocking SugarCubes feedback. */
  readonly #run = (operation: Promise<void>): void => {
    void operation.catch(this.#options.reportError);
  };
}
