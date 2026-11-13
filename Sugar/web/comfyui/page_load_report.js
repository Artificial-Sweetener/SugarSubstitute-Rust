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
 * Register the small extension that reports whether this page loaded SugarCubes.
 *
 * It stays separate from the main SugarCubes module so it can still report when that module
 * fails to load.
 */
import { app } from '/scripts/app.js';
import { api } from '/scripts/api.js';
import { SUGARCUBES_EXTENSION_NAME, reportPageLoad } from './ui/core/updates/PageLoadReport.js';
/** The registered extension names, or null when this ComfyUI does not list them. */
function registeredExtensionNames() {
    const extensions = Reflect.get(app, 'extensions');
    if (!Array.isArray(extensions))
        return null;
    return extensions.flatMap((extension) => {
        const name = typeof extension === 'object' && extension !== null
            ? Reflect.get(extension, 'name')
            : undefined;
        return typeof name === 'string' ? [name] : [];
    });
}
app.registerExtension({
    name: 'SugarCubes.PageLoadReport',
    setup() {
        reportPageLoad({
            // Without a listing, wait for the main extension's own announcement instead.
            registeredExtensions: registeredExtensionNames() ?? [SUGARCUBES_EXTENSION_NAME],
            document,
            report: (loaded) => {
                api
                    .fetchApi('/sugarcubes/self-update/page-load', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ loaded }),
                })
                    .catch((error) => {
                    console.warn('SugarCubes could not report how the page loaded', { error });
                });
            },
        });
    },
});
