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
/** Present declared model identity independently of editable Cube titles. */

export interface CubeModelTitlePresentation {
  accessibleText: string;
  modelText: string;
  nameText: string;
  suffixText: string;
  usesModelPill: boolean;
}

export interface CubeModelTitleInput {
  targetModel: unknown;
  title: unknown;
  suffix?: unknown;
}

/** Preserve arbitrary names while removing only their declared model route prefix. */
export function resolveCubeModelTitle({
  targetModel,
  title,
  suffix = '',
}: CubeModelTitleInput): CubeModelTitlePresentation {
  const modelText = readText(targetModel);
  const titleText = readText(title);
  const suffixText = readText(suffix);
  const routePrefix = `${modelText}/`;
  const hasMatchingPrefix =
    Boolean(modelText) &&
    titleText.length > routePrefix.length &&
    titleText.slice(0, routePrefix.length).toLocaleLowerCase() === routePrefix.toLocaleLowerCase();
  const accessibleTitle =
    hasMatchingPrefix || !modelText ? titleText : [modelText, titleText].filter(Boolean).join(' ');
  return {
    accessibleText: [accessibleTitle, suffixText].filter(Boolean).join(' '),
    modelText,
    nameText: hasMatchingPrefix ? titleText.slice(routePrefix.length).trim() : titleText,
    suffixText,
    usesModelPill: Boolean(modelText),
  };
}

/** Accept only explicit non-empty title text from typed or dynamic callers. */
function readText(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
