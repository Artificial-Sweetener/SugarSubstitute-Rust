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
/** The editor's file list: files under "Group (count)" headings, one selected. */
import type { TextAsset } from '../domain/TextAsset.js';

/** What the list tells its editor. */
export interface TextAssetListEvents {
  select(asset: TextAsset): void;
  /** The person asked for a file's menu at a viewport point. */
  menu(asset: TextAsset, x: number, y: number): void;
}

/** Draw `assets` into `list`, grouped in the order their groups first appear. */
export function renderTextAssetList(
  list: HTMLElement,
  assets: readonly TextAsset[],
  selected: string | null,
  emptyText: string,
  events: TextAssetListEvents,
): void {
  const document = list.ownerDocument;
  if (!assets.length) {
    const empty = document.createElement('p');
    empty.className = 'sc-text-assets__empty';
    empty.textContent = emptyText;
    list.replaceChildren(empty);
    return;
  }
  const groups = new Map<string, TextAsset[]>();
  for (const asset of assets) groups.set(asset.group, [...(groups.get(asset.group) ?? []), asset]);
  const rows: HTMLElement[] = [];
  // Rows alternate across the whole list, as they did in SugarSubstitute.
  let stripe = 0;
  for (const [group, members] of groups) {
    const heading = document.createElement('div');
    heading.className = 'sc-text-assets__group';
    heading.textContent = `${group} (${String(members.length)})`;
    rows.push(heading);
    for (const asset of members) {
      const row = document.createElement('button');
      row.type = 'button';
      row.className = 'sc-text-assets__row';
      row.setAttribute('role', 'option');
      row.setAttribute('aria-selected', String(asset.id === selected));
      row.dataset.asset = asset.id;
      if (stripe++ % 2 === 0) row.classList.add('is-alternate');
      if (asset.enabled === false) row.classList.add('is-disabled');
      row.title = asset.subtitle;
      const label = document.createElement('span');
      label.className = 'sc-text-assets__label';
      label.textContent = asset.label;
      const subtitle = document.createElement('span');
      subtitle.className = 'sc-text-assets__subtitle';
      subtitle.textContent = asset.subtitle;
      row.append(label, subtitle);
      row.addEventListener('click', () => events.select(asset));
      row.addEventListener('contextmenu', (event) => {
        event.preventDefault();
        events.select(asset);
        events.menu(asset, event.clientX, event.clientY);
      });
      rows.push(row);
    }
  }
  list.replaceChildren(...rows);
}
