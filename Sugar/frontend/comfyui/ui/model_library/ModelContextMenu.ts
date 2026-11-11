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
/** Own Substitute's conditional metadata-menu actions independently of Vue and transport. */
import type { ModelCatalogItem } from './ModelCatalog.js';

export type ModelContextCommand =
  | 'refresh'
  | 'output-thumbnail'
  | 'detector-thumbnail'
  | 'versions'
  | 'dismiss'
  | 'disable-updates';
export interface ModelContextEntry {
  readonly label: string;
  readonly enabled: boolean;
  readonly command: ModelContextCommand | null;
  readonly url: string | null;
  readonly children?: readonly ModelContextEntry[];
  readonly imageId?: string;
  readonly separator?: true;
  readonly checked?: boolean;
  readonly providerIcon?: string;
}

/** Restrict provider navigation to known HTTPS model hosts rather than accepting executable URLs. */
export function modelProviderPage(value: string): string | null {
  try {
    const url = new URL(value);
    return url.protocol === 'https:' &&
      ['civitai.com', 'huggingface.co', 'openmodeldb.info'].includes(url.hostname) &&
      !url.username &&
      !url.password &&
      (!url.port || url.port === '443')
      ? url.href
      : null;
  } catch {
    return null;
  }
}

/** Establish version browsing from provider identity or an exact retained update offer. */
export function canBrowseModelVersions(item: ModelCatalogItem, hasUpdate = false): boolean {
  if (item.versionCount != null && item.versionCount < 2) return false;
  return (
    ['checkpoints', 'diffusion_models', 'loras'].includes(item.kind) &&
    /^[a-f0-9]{64}$/.test(item.id) &&
    (hasUpdate ||
      (item.provider === 'civitai' &&
        /^[1-9][0-9]{0,15}$/.test(item.providerModelId) &&
        /^[1-9][0-9]{0,15}$/.test(item.providerVersionId) &&
        !!item.baseModel.trim()))
  );
}

/** Place provider history before canvas thumbnail actions independently of update notifications. */
export function modelContextEntries(
  item: ModelCatalogItem,
  canManage: boolean,
  hasUpdate: boolean,
  hasOutput: boolean,
): readonly ModelContextEntry[] {
  const entries: ModelContextEntry[] = [];
  const detector = item.kind === 'ultralytics';
  if (detector && canManage)
    entries.push({
      label: 'Choose detector thumbnail',
      enabled: true,
      command: 'detector-thumbnail',
      url: null,
    });
  const links = item.providerLinks.length
    ? item.providerLinks
    : [
        {
          providerName:
            item.provider === 'civitai'
              ? 'CivitAI'
              : item.provider === 'openmodeldb'
                ? 'OpenModelDB'
                : 'Hugging Face',
          modelPageUrl: item.providerUrl,
        },
      ];
  for (const link of links) {
    const page = modelProviderPage(link.modelPageUrl);
    if (page)
      entries.push({
        label: `Go to ${link.providerName} page`,
        enabled: true,
        command: null,
        url: page,
      });
  }
  const canEditMetadata = canManage && !detector && !!item.value && !!item.kind;
  if (canEditMetadata)
    entries.push({
      label: 'Refresh CivitAI metadata',
      enabled: true,
      command: 'refresh',
      url: null,
    });
  if (canManage && canBrowseModelVersions(item, hasUpdate))
    entries.push({ label: 'See other versions', enabled: true, command: 'versions', url: null });
  if (canEditMetadata)
    entries.push({
      label: 'Set thumbnail from canvas',
      enabled: hasOutput,
      command: 'output-thumbnail',
      url: null,
    });
  return entries;
}
