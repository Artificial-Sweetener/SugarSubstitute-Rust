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
/** Track only mounted masonry section geometry for the rail's scrolling selection. */
export class CubeStackScrollSelection {
  readonly #element: HTMLElement;
  readonly #update: (id: string | null) => void;
  #frame = 0;
  #ids: readonly string[] = [];
  #navigationId: string | null = null;
  #selectedId: string | null = null;

  /** Subscribe to scoped scroll and settled layout without discovering graph candidates. */
  constructor(element: HTMLElement, update: (id: string | null) => void) {
    this.#element = element;
    this.#update = update;
    element.addEventListener('scroll', this.#schedule, { passive: true });
    element.addEventListener('sc-masonry-layout', this.#schedule);
    element.addEventListener('scrollend', this.#settleNavigation);
    element.addEventListener('wheel', this.#interruptNavigation, { passive: true });
    element.addEventListener('touchstart', this.#interruptNavigation, { passive: true });
    element.addEventListener('pointerdown', this.#interruptNavigation);
    element.addEventListener('keydown', this.#onKeyDown);
  }

  /** Reconcile the exact already projected row identities after topology or preview order changes. */
  refresh(ids: readonly string[]): void {
    this.#ids = ids;
    if (this.#navigationId && !ids.includes(this.#navigationId)) this.#navigationId = null;
    this.#measure();
  }

  /** Hold an explicitly revealed cube through smooth scrolling until settlement or user input. */
  navigate(id: string): void {
    if (!this.#ids.includes(id)) return;
    this.#navigationId = id;
    this.#select(id);
  }

  /** Resume viewport tracking when the user takes control of scrolling. */
  readonly #interruptNavigation = (): void => {
    this.#navigationId = null;
    this.#schedule();
  };

  /** Leave typing and unrelated shortcuts out of scroll selection ownership. */
  readonly #onKeyDown = (event: KeyboardEvent): void => {
    if (['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End', ' '].includes(event.key))
      this.#interruptNavigation();
  };

  /** Publish the requested cube at settlement before resuming subsequent viewport tracking. */
  readonly #settleNavigation = (): void => {
    if (!this.#navigationId) return;
    this.#element.ownerDocument.defaultView?.cancelAnimationFrame(this.#frame);
    this.#frame = 0;
    this.#select(this.#navigationId);
    this.#navigationId = null;
  };

  /** Notify presentation only when active cube identity changes. */
  #select(id: string): void {
    if (this.#selectedId === id) return;
    this.#selectedId = id;
    this.#update(id);
  }

  /** Coalesce browser scroll delivery into one read of the mounted section destinations. */
  readonly #schedule = (): void => {
    if (this.#frame) return;
    this.#frame =
      this.#element.ownerDocument.defaultView?.requestAnimationFrame(() => {
        this.#frame = 0;
        this.#measure();
      }) ?? 0;
  };

  /** Match Substitute's first visible whole-cube bounds using settled masonry destinations. */
  #measure(): void {
    if (this.#navigationId) {
      this.#select(this.#navigationId);
      return;
    }
    const headers = [
      ...this.#element.querySelectorAll<HTMLElement>('header[data-masonry-section]'),
    ];
    const sections = this.#ids
      .flatMap((id) => {
        const header = headers.find(
          (item) =>
            item.dataset.masonrySection === `cube:${id}` &&
            !item.hasAttribute('data-sc-search-card-hidden'),
        );
        if (!header) return [];
        const y = Number(header.dataset.masonryY) + (header.parentElement?.offsetTop ?? 0);
        const height = Number(header.dataset.masonryHeight);
        return Number.isFinite(y) && Number.isFinite(height) && height >= 0
          ? [{ id, y, height }]
          : [];
      })
      .sort((a, b) => a.y - b.y);
    const y = this.#element.scrollTop;
    const firstVisible = sections.find(
      (section) => section.y + section.height >= y && section.y <= y + this.#element.clientHeight,
    );
    if (firstVisible) this.#select(firstVisible.id);
  }

  /** Release every scoped listener and pending geometry read before renderer teardown. */
  dispose(): void {
    this.#element.removeEventListener('scroll', this.#schedule);
    this.#element.removeEventListener('sc-masonry-layout', this.#schedule);
    this.#element.removeEventListener('scrollend', this.#settleNavigation);
    this.#element.removeEventListener('wheel', this.#interruptNavigation);
    this.#element.removeEventListener('touchstart', this.#interruptNavigation);
    this.#element.removeEventListener('pointerdown', this.#interruptNavigation);
    this.#element.removeEventListener('keydown', this.#onKeyDown);
    this.#element.ownerDocument.defaultView?.cancelAnimationFrame(this.#frame);
  }
}
