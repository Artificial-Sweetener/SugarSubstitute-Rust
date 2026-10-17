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
/** Match SugarSubstitute's stable label overrides and title casing for native presentation. */
const labelOverrides: Readonly<Record<string, string>> = {
  sampler_name: 'Sampler',
  scheduler: 'Scheduler',
  cfg: 'CFG',
  ksampler: 'KSampler',
  vectorscopecc: 'VectorscopeCC',
};
const acronymOverrides: Readonly<Record<string, string>> = {
  cfg: 'CFG',
  vae: 'VAE',
  lora: 'LoRA',
};
const minorWords = new Set(
  (
    'a above about across after against along amid among an and around as at ' +
    'before behind below beneath beside between beyond but by because despite down during ' +
    'except for from in inside into near nor of off on onto or out over past per since so ' +
    'than the through till to under until up upon versus via with within without yet'
  ).split(' '),
);

/** Format visible labels while preserving authored slash prefixes and uppercase words. */
export function formatDisplayLabel(label: string): string {
  const overrideKey = label.toLowerCase().replaceAll(' ', '_');
  const override = Object.hasOwn(labelOverrides, overrideKey)
    ? labelOverrides[overrideKey]
    : undefined;
  if (override) return override;
  const slash = label.indexOf('/');
  const hasPrefix = slash > 0 && slash < label.length - 1;
  const prefix = hasPrefix ? label.slice(0, slash + 1) : '';
  const body = hasPrefix ? label.slice(slash + 1) : label;
  const words = body.replaceAll('_', ' ').trim().split(/\s+/).filter(Boolean);
  return (
    prefix +
    words
      .map((word, index) =>
        index > 0 && index < words.length - 1 && minorWords.has(word.toLowerCase())
          ? word.toLowerCase()
          : formatSignificantWord(word),
      )
      .join(' ')
  );
}

/** Preserve known acronym spelling before normalizing a significant title word. */
function formatSignificantWord(word: string): string {
  const lower = word.toLowerCase();
  const acronym = Object.hasOwn(acronymOverrides, lower) ? acronymOverrides[lower] : undefined;
  if (acronym) return acronym;
  if (word.length > 1 && word === word.toUpperCase() && word !== word.toLowerCase()) return word;
  if (word.startsWith('/') && word.length > 1) return '/' + capitalize(word.slice(1));
  return capitalize(word);
}

/** Match Substitute's first-letter uppercase and remaining-letter lowercase convention. */
function capitalize(word: string): string {
  return word.slice(0, 1).toUpperCase() + word.slice(1).toLowerCase();
}
