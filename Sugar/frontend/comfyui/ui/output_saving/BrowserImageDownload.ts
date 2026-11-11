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
/** Dispatch full-resolution image downloads through the browser's existing download UI. */

export interface OutputImageDownload {
  /** Start a same-origin image download using its server-supplied filename. */
  start(url: string, filename?: string): void;
}

/** Own the short-lived DOM anchor without opening another canvas or browser tab. */
export class BrowserImageDownload implements OutputImageDownload {
  /** Bind the active document as the sole browser download boundary. */
  constructor(private readonly document: Document) {}

  /** Leave file selection and destination management with the browser's download controls. */
  start(url: string, filename = ''): void {
    const anchor = this.document.createElement('a');
    anchor.href = url;
    anchor.download = filename;
    anchor.hidden = true;
    this.document.body.append(anchor);
    try {
      anchor.click();
    } finally {
      anchor.remove();
    }
  }
}
