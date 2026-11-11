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
/** Regional separators render inert literal labels while native source stays selectable and copyable. */
import { SourceToken, SourceWidget } from './projection-widgets.js';
import type { RegionTarget } from './region-index.js';

/** One full-line source token owns centered framing and an optional literal name. */
export class RegionWidget extends SourceWidget {
  /** Regional rows remain structural at an interior F2 caret, matching their atomic navigation contract. */
  override readonly revealOnInteriorSelection = false;
  readonly ordinal: number;
  readonly name: string | null;
  readonly target: RegionTarget;
  /** Capture prepared semantic display fields rather than discovering source during rendering. */
  constructor(token: SourceToken, target: RegionTarget) {
    super(token);
    this.target = target;
    this.ordinal = target.ordinal;
    this.name = target.separator.name;
  }
  /** Retain actual DOM across source snapshots when label identity and ordinal remain equal. */
  override eq(other: RegionWidget): boolean {
    return (
      other instanceof RegionWidget &&
      other.target.scopeId === this.target.scopeId &&
      other.ordinal === this.ordinal &&
      other.name === this.name
    );
  }
  /** Safe literal DOM displays names without interpreting markup or prompt syntax. */
  override toDOM(): HTMLElement {
    const row = document.createElement('span'),
      framing = document.createElement('span'),
      label = document.createElement('span');
    row.className = 'sp-region-separator';
    row.dataset.regionIndex = String(this.ordinal);
    row.dataset.regionScope = this.target.scopeId;
    row.dataset.regionNamed = String(this.name !== null);
    row.setAttribute(
      'aria-label',
      this.name === null ? 'Regional separator' : `Regional separator: ${this.name}`,
    );
    framing.className = 'sp-region-framing';
    label.className = 'sp-region-label';
    label.textContent = this.name ?? '';
    framing.append(label);
    row.append(framing);
    return row;
  }
}
