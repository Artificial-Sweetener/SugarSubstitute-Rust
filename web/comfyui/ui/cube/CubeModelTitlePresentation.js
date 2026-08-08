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
/** Segment authoritative Cube model routes for renderer-neutral title presentation. */
/** Split only a declared target-model route prefix from one visible title. */
export function resolveCubeModelTitle({ targetModel, title, suffix = '', allowModelPill = true, }) {
    const modelText = readText(targetModel);
    const titleText = readText(title);
    const suffixText = readText(suffix);
    const accessibleText = [titleText, suffixText].filter(Boolean).join(' ');
    const routePrefix = `${modelText}/`;
    const hasMatchingPrefix = allowModelPill &&
        Boolean(modelText) &&
        titleText.length > routePrefix.length &&
        titleText.slice(0, routePrefix.length).toLocaleLowerCase() === routePrefix.toLocaleLowerCase();
    if (!hasMatchingPrefix) {
        return {
            accessibleText,
            modelText: '',
            nameText: titleText,
            suffixText,
            usesModelPill: false,
        };
    }
    return {
        accessibleText,
        modelText,
        nameText: titleText.slice(routePrefix.length).trim(),
        suffixText,
        usesModelPill: true,
    };
}
/** Use a model pill in the instance lane only while it shows the default alias. */
export function resolveDefaultInstanceModelTitle({ targetModel, instanceTitle, defaultAlias, }) {
    const resolvedInstanceTitle = readText(instanceTitle);
    const resolvedDefaultAlias = readText(defaultAlias);
    return resolveCubeModelTitle({
        targetModel,
        title: resolvedInstanceTitle,
        allowModelPill: Boolean(resolvedInstanceTitle) && resolvedInstanceTitle === resolvedDefaultAlias,
    });
}
/** Accept only explicit non-empty title text from typed or dynamic callers. */
function readText(value) {
    return typeof value === 'string' ? value.trim() : '';
}
