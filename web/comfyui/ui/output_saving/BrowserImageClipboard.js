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
/** Adapt exact preferred image bytes to Chromium's supported clipboard representations. */
/** Keep browser format constraints out of shared saving and transfer policy. */
export class BrowserImageClipboard {
    document;
    /** Bind one browser document for clipboard access and transient pixel conversion. */
    constructor(document) {
        this.document = document;
    }
    /** Preserve JPEG/WebP bytes as custom formats while offering PNG pixels to ordinary applications. */
    async write(blob, mimeType) {
        const clipboard = this.document.defaultView?.navigator.clipboard;
        if (!clipboard || typeof ClipboardItem !== 'function')
            throw new Error('Image copying is unavailable in this browser.');
        const representations = {};
        if (ClipboardItem.supports(mimeType))
            representations[mimeType] = blob;
        else {
            const customType = 'web ' + mimeType;
            if (!ClipboardItem.supports(customType))
                throw new Error('The browser cannot copy the selected image format.');
            representations[customType] = blob;
        }
        if (mimeType !== 'image/png')
            representations['image/png'] = this.#png(blob);
        await clipboard.write([new ClipboardItem(representations)]);
    }
    /** Release temporary decoded pixels immediately after interoperable PNG encoding. */
    async #png(blob) {
        const bitmap = await createImageBitmap(blob);
        const canvas = this.document.createElement('canvas');
        try {
            canvas.width = bitmap.width;
            canvas.height = bitmap.height;
            const context = canvas.getContext('2d');
            if (!context)
                throw new Error('The browser could not prepare clipboard image pixels.');
            context.drawImage(bitmap, 0, 0);
            return await new Promise((resolve, reject) => canvas.toBlob((result) => result
                ? resolve(result)
                : reject(new Error('The browser could not encode the clipboard image.')), 'image/png'));
        }
        finally {
            bitmap.close();
            canvas.width = 0;
            canvas.height = 0;
        }
    }
}
