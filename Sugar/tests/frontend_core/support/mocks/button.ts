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
export class ComfyButton {
  readonly label: string;
  readonly element: HTMLButtonElement;

  constructor(options: string | ComfyButtonOptions = '') {
    const config =
      options && typeof options === 'object'
        ? options
        : {
            content: String(options ?? ''),
          };
    this.label = config.content || '';
    this.element = document.createElement('button');
    this.element.textContent = config.content || '';
    if (config.classList) {
      this.element.className = String(config.classList);
    }
    if (config.tooltip) {
      this.element.title = String(config.tooltip);
    }
    const action = config.action;
    if (typeof action === 'function') {
      this.element.addEventListener('click', () => action());
    }
  }

  setEnabled(enabled: boolean): void {
    this.element.disabled = !enabled;
  }
}

interface ComfyButtonOptions {
  content?: string;
  classList?: string;
  tooltip?: string;
  action?: () => void;
}
