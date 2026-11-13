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
/** Lease host title surfaces while sharing the canonical model-chip renderer. */
import type { CubeModelTitlePresentation } from '../cube/CubeModelTitlePresentation.js';
import { createCubeModelTitleElement } from '../surface/CubeModelPillDomRenderer.js';

interface TitleLease {
  readonly children: readonly Node[];
  readonly title: HTMLElement;
  readonly signature: string;
}

/** Retain host children and reconcile rich titles without disturbing native menu identity. */
export class CubeEditorTitlePresenter {
  readonly #leases = new Map<HTMLElement, TitleLease>();

  /** Replace only changed semantic titles and recover if the host overwrites owned content. */
  present(host: HTMLElement, presentation: CubeModelTitlePresentation): void {
    const previous = this.#leases.get(host);
    const signature = JSON.stringify(presentation);
    if (previous?.signature === signature && host.firstChild === previous.title) return;
    const title = createCubeModelTitleElement(host.ownerDocument, presentation);
    this.#leases.set(host, {
      children: previous?.children ?? [...host.childNodes],
      title,
      signature,
    });
    host.replaceChildren(title);
  }

  /** Forget unmounted host surfaces without retaining their borrowed child nodes. */
  prune(): void {
    for (const host of this.#leases.keys()) if (!host.isConnected) this.#leases.delete(host);
  }

  /** Restore the exact borrowed host children when the Cube context ends. */
  clear(): void {
    for (const [host, lease] of this.#leases) host.replaceChildren(...lease.children);
    this.#leases.clear();
  }
}
