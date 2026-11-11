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
/** Apply server-owned declarations and unambiguous catalog evidence to native fields. */
import type { ModelCatalogSnapshot } from './ModelCatalog.js';
import type { NativeWidgetOrigin } from '../cube/graph/NativeSubgraphBoundaryResolver.js';

/** Keep ambiguous or non-model dropdowns under their original widget owner. */
export function recognizeModelField(
  snapshot: ModelCatalogSnapshot,
  classType: string,
  name: string,
  options: readonly string[],
  origins: readonly NativeWidgetOrigin[] | null = null,
): string | null {
  if (origins !== null) {
    if (origins.length === 0) return null;
    const kinds = origins.map((origin) =>
      recognizeModelField(snapshot, origin.classType, origin.inputName, options),
    );
    return kinds.every((kind) => kind !== null && kind === kinds[0]) ? (kinds[0] ?? null) : null;
  }
  const declared =
    snapshot.fieldContracts.find(
      (rule) => rule.classType === classType && rule.inputName === name,
    ) ?? snapshot.fieldContracts.find((rule) => rule.classType === '' && rule.inputName === name);
  if (declared) return declared.kind;
  const kinds = new Set(
    snapshot.items.filter((item) => options.includes(item.value)).map((item) => item.kind),
  );
  return kinds.size === 1 ? ([...kinds][0] ?? null) : null;
}
