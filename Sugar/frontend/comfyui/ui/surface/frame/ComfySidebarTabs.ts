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
/**
 * Read ComfyUI's registered sidebar tabs, its own and every extension's, through its public
 * workspace store, so the frame can offer them where its hidden sidebar strip would.
 *
 * ComfyUI keeps rendering its strip while the frame hides it, one `<id>-tab-button` per tab with
 * the tab's translated label, so a tab's shown name is read from there; the registered title is
 * the fallback.
 */
import { isRecord } from '../../types/common.js';

/** One registered sidebar tab as the frame offers it. */
export interface SidebarTab {
  readonly id: string;
  readonly title: string;
  /** ComfyUI's icon class for the tab, when it registered one as a class. */
  readonly icon: string | null;
}

/** The part of ComfyUI's sidebar store the frame uses. */
interface SidebarStore {
  readonly sidebarTabs: readonly unknown[];
  readonly activeSidebarTabId: unknown;
  toggleSidebarTab(id: string): void;
}

/** Adapt ComfyUI's sidebar store to typed tabs, open state and toggling. */
export class ComfySidebarTabs {
  constructor(
    private readonly document: Document,
    private readonly app: unknown,
  ) {}

  /** Every registered tab, in ComfyUI's order. */
  tabs(): readonly SidebarTab[] {
    const store = this.#store();
    if (!store) return [];
    const tabs: SidebarTab[] = [];
    for (const tab of store.sidebarTabs) {
      if (!isRecord(tab) || typeof tab.id !== 'string' || !tab.id) continue;
      tabs.push({
        id: tab.id,
        title: this.#label(tab.id) ?? (typeof tab.title === 'string' ? tab.title : tab.id),
        icon: typeof tab.icon === 'string' && tab.icon.trim() ? tab.icon.trim() : null,
      });
    }
    return tabs;
  }

  /** The open tab, if any. */
  active(): string | null {
    const id = this.#store()?.activeSidebarTabId;
    return typeof id === 'string' && id ? id : null;
  }

  /** Open a tab, or close it when it is the open one, as its strip button would. */
  toggle(id: string): void {
    const store = this.#store();
    if (!store) throw new Error('ComfyUI sidebar tabs are unavailable.');
    store.toggleSidebarTab(id);
  }

  #store(): SidebarStore | null {
    const manager = isRecord(this.app) ? this.app.extensionManager : null;
    const store = isRecord(manager) ? manager.sidebarTab : null;
    if (
      !isRecord(store) ||
      !Array.isArray(store.sidebarTabs) ||
      typeof store.toggleSidebarTab !== 'function'
    )
      return null;
    const toggle = store.toggleSidebarTab as (id: string) => void;
    return {
      sidebarTabs: store.sidebarTabs as readonly unknown[],
      activeSidebarTabId: store.activeSidebarTabId,
      toggleSidebarTab: (id) => toggle.call(store, id),
    };
  }

  /** The tab's translated label from ComfyUI's own strip button, without soft hyphens. */
  #label(id: string): string | null {
    const className = `${id}-tab-button`;
    const button = [...this.document.querySelectorAll('[class*="-tab-button"]')].find((element) =>
      element.classList.contains(className),
    );
    const label = button?.querySelector('.side-bar-button-label')?.textContent;
    const text = (label ?? button?.getAttribute('aria-label') ?? '')
      .replace(/­/g, '')
      .replace(/\s*\([^)]*\)\s*$/, '')
      .trim();
    return text || null;
  }
}
