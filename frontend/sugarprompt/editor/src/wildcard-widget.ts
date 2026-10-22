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
/** Compact wildcard chrome leaves its identifier/CSV column as native editable source text. */
import type { WildcardGroup } from '../../core/src/index.js';
import type { WildcardResolution } from '../../services/src/index.js';
import { SourceWidget, type SourceToken } from './projection-widgets.js';

/** Prefix and suffix share one source token; display-only group numbers never manufacture source. */
export class WildcardWidget extends SourceWidget {
  readonly part: 'prefix' | 'suffix';
  readonly group: WildcardGroup;
  /** Prepared catalog status controls shell paint without resolving during DOM construction. */
  readonly resolution: WildcardResolution['status'];
  readonly adjustment: 'none' | 'wheel' | 'pointer';
  /** Capture prepared group, resolution and adjustment metadata without parsing or catalog work in DOM construction. */
  constructor(
    token: SourceToken,
    part: 'prefix' | 'suffix',
    group: WildcardGroup,
    resolution: WildcardResolution['status'],
    adjustment: 'none' | 'wheel' | 'pointer' = 'none',
  ) {
    super(token);
    this.part = part;
    this.group = group;
    this.resolution = resolution;
    this.adjustment = adjustment;
  }
  /** Equal literal chrome and prepared paint status retain real native DOM across semantic preparations. */
  override eq(other: WildcardWidget): boolean {
    return (
      this.part === other.part &&
      this.resolution === other.resolution &&
      this.group.displayTag === other.group.displayTag &&
      this.group.explicit === other.group.explicit &&
      this.group.numeric === other.group.numeric &&
      this.adjustment === other.adjustment
    );
  }
  /** Render the donor's compact braces and italic tag through literal DOM APIs. */
  override toDOM(): HTMLElement {
    const wrapper = document.createElement('span');
    wrapper.className = `sp-wildcard-${this.part}`;
    wrapper.dataset.resolution = this.resolution;
    if (this.adjustment !== 'none')
      wrapper.classList.add(
        this.adjustment === 'wheel' ? 'sp-wheel-ready' : 'sp-adjustment-content',
      );
    const brace = document.createElement('span');
    brace.className = 'sp-wildcard-brace';
    brace.textContent = this.part === 'prefix' ? '{' : '}';
    wrapper.append(brace);
    if (this.part === 'suffix' && this.group.displayTag !== null) {
      const tag = document.createElement('span');
      tag.className = 'sp-wildcard-tag';
      tag.textContent = this.group.displayTag;
      tag.dataset.explicit = String(this.group.explicit);
      tag.dataset.numeric = String(this.group.numeric);
      tag.setAttribute('aria-label', `Wildcard group ${this.group.displayTag}`);
      wrapper.append(tag);
    }
    return wrapper;
  }
}
