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
/** Resolve Substitute's stable model-family preset associations from prepared metadata evidence. */
import type { PresetAssociation } from './UserPreset.js';
export interface FamilyEvidence {
  readonly architecture?: string;
  readonly baseModel: string;
  readonly title: string;
  readonly tags: readonly string[];
}
const aliases: readonly [string, string, RegExp][] = [
  ['anima', 'Anima', /\banima\b/i],
  ['sdxl', 'SDXL', /\b(?:sdxl|sdxl\s*1\.?0|stable\s+diffusion\s+xl)\b/i],
  ['illustrious', 'Illustrious', /\billustrious\b/i],
  ['noobai', 'NoobAI', /\bnoob\s*ai\b|\bnoobai\b/i],
  ['pony', 'Pony', /\bpony\b/i],
  ['flux', 'FLUX', /\bflux\b/i],
];
/** Retain detected architecture alongside Substitute-compatible family associations. */
export function modelFamilyScopes(evidence: FamilyEvidence | null): PresetAssociation[] {
  if (!evidence) return [];
  const base = aliases.find(([, , pattern]) => pattern.test(evidence.baseModel));
  const families = [...(base ? [base] : [])];
  if (!base || base[0] === 'sdxl')
    for (const text of [evidence.title, evidence.tags.join(' ')]) {
      const family = aliases.find(([, , pattern]) => pattern.test(text));
      if (family && !families.some(([key]) => key === family[0])) families.push(family);
    }
  const scopes: PresetAssociation[] = families.map(([key, label]) => ({
    scope: 'model_family',
    provider: 'civitai',
    key,
    label,
  }));
  if (evidence.architecture) {
    const alias = aliases.find(([, label]) => label === evidence.architecture);
    const architecture: PresetAssociation = {
      scope: 'model_family',
      provider: alias ? 'civitai' : 'comfy',
      key: alias?.[0] ?? evidence.architecture.toLowerCase(),
      label: evidence.architecture,
    };
    if (
      !scopes.some(
        (scope) => scope.provider === architecture.provider && scope.key === architecture.key,
      )
    )
      scopes.unshift(architecture);
  }
  return scopes;
}
