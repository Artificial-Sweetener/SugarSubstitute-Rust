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
/** Read the facts the protocol evolution rules compare: schema digests, capabilities and reads. */
import { createHash } from 'node:crypto';

/** One released protocol version, frozen by the digest of its schema. */
export interface ReleasedSchema {
  readonly file: string;
  readonly id: string;
  readonly sha256: string;
}

/** One released capability name and the behaviour it names. */
export interface ReleasedCapability {
  readonly name: string;
  readonly meaning: string;
}

/** The registry of released protocol versions and capability names. */
export interface ReleasedContracts {
  readonly schemas: readonly ReleasedSchema[];
  readonly capabilities: readonly ReleasedCapability[];
}

/** Order object keys so formatting and key order never change a digest. */
function canonical(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object')
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, canonical((value as Record<string, unknown>)[key])]),
    );
  return value;
}

/** Digest a schema's meaning: its parsed content with keys in a fixed order. */
export function schemaDigest(text: string): string {
  return createHash('sha256')
    .update(JSON.stringify(canonical(JSON.parse(text))))
    .digest('hex');
}

/** Read the schema's `$id`, which names its protocol and version. */
export function schemaId(text: string): string {
  const id = (JSON.parse(text) as { $id?: unknown }).$id;
  if (typeof id !== 'string') throw new Error('A contract schema must declare its $id.');
  return id;
}

/** Read each capability name and the doc comment that states its behaviour. */
export function declaredCapabilities(source: string): ReleasedCapability[] {
  const block = /HOST_CAPABILITIES = Object\.freeze\(\{([\s\S]*?)\}/.exec(source)?.[1];
  if (!block) throw new Error('HOST_CAPABILITIES was not found.');
  return [...block.matchAll(/\/\*\*([\s\S]*?)\*\/\s*\w+: '([a-z0-9.-]+)'/g)].map((match) => ({
    name: match[2] ?? '',
    meaning: (match[1] ?? '').replace(/\s*\*\s*/g, ' ').trim(),
  }));
}

/**
 * Find page reads of host declarations that bypass the declaration check.
 *
 * A capability is read only as `declares(HOST_CAPABILITIES.name)` and a protocol only as
 * `serves(HOST_PROTOCOLS.name)`, with names that exist. Any other use of either table, and any
 * capability name written as text, lets page behaviour depend on something the host never
 * declared.
 */
export function undeclaredHostReads(
  source: string,
  capabilities: readonly string[],
  protocols: readonly string[],
  capabilityNames: readonly string[],
): string[] {
  const findings: string[] = [];
  for (const match of source.matchAll(/(\.?\w*\(?)\s*HOST_(CAPABILITIES|PROTOCOLS)\.(\w+)/g)) {
    const [whole, before = '', table, key = ''] = match;
    const capability = table === 'CAPABILITIES';
    const reader = capability ? '.declares(' : '.serves(';
    if (!before.endsWith(reader)) findings.push(`${whole.trim()} is not read through ${reader}`);
    if (!(capability ? capabilities : protocols).includes(key))
      findings.push(`HOST_${table ?? ''}.${key} does not exist`);
  }
  for (const match of source.matchAll(/\.declares\(\s*(?!HOST_CAPABILITIES\.)([^)]*)\)/g))
    findings.push(`declares(${match[1] ?? ''}) does not name a declared capability`);
  for (const match of source.matchAll(/\.serves\(\s*(?!HOST_PROTOCOLS\.)([^)]*)\)/g))
    findings.push(`serves(${match[1] ?? ''}) does not name a declared protocol`);
  for (const name of capabilityNames)
    if (source.includes(`'${name}'`) || source.includes(`"${name}"`))
      findings.push(`the capability name ${name} is written as text`);
  return findings;
}
