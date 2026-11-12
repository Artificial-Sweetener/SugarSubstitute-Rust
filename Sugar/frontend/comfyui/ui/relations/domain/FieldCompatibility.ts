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
/** Infer field meaning and intersect valid domains without per-member coercion. */
import type { FieldContract, FieldDomain, FieldValue } from './RelationContracts.js';

const aliases: Readonly<Record<string, string>> = {
  noise_seed: 'seed',
  sampler: 'sampler_name',
  sampling_method: 'sampler_name',
  cfg_scale: 'cfg',
  guidance_scale: 'cfg',
  encode_style: 'encode_style',
};

/** Normalize field names while retaining distinct meanings such as variation seed. */
export function semanticFieldName(name: string): string {
  const normalized = name
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_|_$/g, '');
  return aliases[normalized] ?? normalized;
}

/** Require shared semantics and at least one value valid in every member. */
export function commonFieldDomain(fields: readonly FieldContract[]): FieldDomain | null {
  const first = fields[0];
  if (
    !first ||
    fields.some(
      (field) => field.semantic !== first.semantic || field.domain.kind !== first.domain.kind,
    )
  )
    return null;
  return intersectFieldDomains(fields.map((field) => field.domain));
}

/** Intersect valid values behind one exposed facade without enrolling its hidden implementation. */
export function intersectFieldDomains(domains: readonly FieldDomain[]): FieldDomain | null {
  const domain = domains[0];
  if (!domain || domains.some((candidate) => candidate.kind !== domain.kind)) return null;
  if (domain.kind === 'choice') {
    const values = domain.values.filter((value) =>
      domains.every((candidate) => acceptsFieldValue(candidate, value)),
    );
    return values.length ? { kind: 'choice', values: [...new Set(values)] } : null;
  }
  if (domain.kind !== 'number') return domain;
  return intersectNumbers(
    domains.flatMap((candidate) => (candidate.kind === 'number' ? [candidate] : [])),
  );
}

/** Validate an edit against the exact shared contract instead of silently clamping it. */
export function acceptsFieldValue(domain: FieldDomain | null, value: FieldValue): boolean {
  if (!domain) return false;
  if (domain.kind === 'choice') return typeof value === 'string' && domain.values.includes(value);
  if (domain.kind === 'string') return typeof value === 'string';
  if (domain.kind === 'boolean') return typeof value === 'boolean';
  if (
    typeof value !== 'number' ||
    !Number.isFinite(value) ||
    value < domain.minimum ||
    value > domain.maximum ||
    (domain.integer && !Number.isInteger(value))
  )
    return false;
  const position = (value - domain.minimum) / domain.step;
  return Math.abs(position - Math.round(position)) < 1e-7;
}

/** Present the union inventory with an explanation for every unavailable choice. */
export function choiceRestrictions(
  fields: readonly FieldContract[],
): { value: string; blockers: string[] }[] {
  const values = new Set(
    fields.flatMap((field) => (field.domain.kind === 'choice' ? [...field.domain.values] : [])),
  );
  return [...values].map((value) => ({
    value,
    blockers: fields
      .filter((field) => !acceptsFieldValue(field.domain, value))
      .map((field) => field.label),
  }));
}

/** Choose a deterministic valid starting value when the source value is outside the intersection. */
export function initialCommonValue(domain: FieldDomain, preferred: FieldValue): FieldValue {
  if (acceptsFieldValue(domain, preferred)) return preferred;
  if (domain.kind === 'choice') return domain.values[0] ?? '';
  if (domain.kind === 'number') return domain.minimum;
  return domain.kind === 'boolean' ? false : '';
}

/** Intersect decimal lattices using integer arithmetic to avoid floating point membership drift. */
function intersectNumbers(
  domains: readonly Extract<FieldDomain, { kind: 'number' }>[],
): FieldDomain | null {
  const scale =
    10 **
    Math.min(
      8,
      Math.max(
        ...domains.flatMap((domain) => [decimalPlaces(domain.step), decimalPlaces(domain.minimum)]),
      ),
    );
  let offset = 0n,
    period = 1n;
  for (const domain of domains) {
    const nextPeriod = BigInt(Math.max(1, Math.round(domain.step * scale)));
    const nextOffset = BigInt(Math.round(domain.minimum * scale));
    const divisor = gcd(period, nextPeriod);
    const delta = nextOffset - offset;
    if (delta % divisor !== 0n) return null;
    const modulus = nextPeriod / divisor;
    const inverse = modularInverse(period / divisor, modulus);
    const multiplier =
      modulus === 1n ? 0n : ((((delta / divisor) * inverse) % modulus) + modulus) % modulus;
    offset += period * multiplier;
    period = (period / divisor) * nextPeriod;
    offset = ((offset % period) + period) % period;
  }
  const minimum = Math.max(...domains.map((domain) => domain.minimum));
  const maximum = Math.min(...domains.map((domain) => domain.maximum));
  const step = Number(period) / scale;
  const base = Number(offset) / scale;
  const start = base + Math.ceil((minimum - base) / step - 1e-9) * step;
  if (!Number.isFinite(start) || start > maximum) return null;
  return {
    kind: 'number',
    minimum: start,
    maximum,
    step,
    integer: domains.some((domain) => domain.integer),
  };
}

/** Count meaningful decimal precision including exponential host constraints. */
function decimalPlaces(value: number): number {
  const [mantissa = '', exponent = '0'] = String(value).toLowerCase().split('e');
  return Math.max(0, (mantissa.split('.')[1]?.length ?? 0) - Number(exponent));
}

/** Compute the lattice divisor without losing numeric precision. */
function gcd(left: bigint, right: bigint): bigint {
  while (right) [left, right] = [right, left % right];
  return left;
}

/** Compute the coprime modular inverse used by the common step grid. */
function modularInverse(value: bigint, modulus: bigint): bigint {
  if (modulus === 1n) return 0n;
  let a = value,
    b = modulus,
    x = 1n,
    y = 0n;
  while (b) {
    const quotient = a / b;
    [a, b] = [b, a % b];
    [x, y] = [y, x - quotient * y];
  }
  return ((x % modulus) + modulus) % modulus;
}
