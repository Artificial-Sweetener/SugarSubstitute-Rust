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
/** Read a validated host description into the declarations a page consults. */
import {
  HOST_PROTOCOLS,
  type HostCapabilityName,
  type HostDescriptionContent,
  type HostDescriptionMessage,
  type HostProtocol,
} from '../contracts/HostDescription.js';

/** What one host declares for a panel session; anything it does not declare is unsupported. */
export class DeclaredHost {
  readonly #protocols: ReadonlySet<string>;
  readonly #capabilities: ReadonlySet<string>;

  private constructor(content: HostDescriptionContent) {
    this.#protocols = new Set(content.protocols.map(protocolKey));
    this.#capabilities = new Set(content.capabilities.map((capability) => capability.name));
  }

  /**
   * Adopt a schema-validated description, refusing one that repeats a protocol or capability.
   *
   * A repeated name is ambiguous about what the host meant, so the whole description fails
   * closed rather than guessing which entry to honour.
   */
  static read(content: HostDescriptionContent): DeclaredHost | null {
    const protocols = new Set(content.protocols.map((protocol) => protocol.name));
    const capabilities = new Set(content.capabilities.map((capability) => capability.name));
    if (
      protocols.size !== content.protocols.length ||
      capabilities.size !== content.capabilities.length
    )
      return null;
    return new DeclaredHost(content);
  }

  /** Serve the page outside any host: no host protocol and no host capability. */
  static readonly standalone = new DeclaredHost({ protocols: [], capabilities: [] });

  /** Describe a host that configured its panel without first describing itself. */
  static readonly undeclared = new DeclaredHost({
    protocols: [HOST_PROTOCOLS.panel],
    capabilities: [],
  });

  /** Whether the host serves this protocol at exactly this envelope version. */
  serves(protocol: HostProtocol): boolean {
    return this.#protocols.has(protocolKey(protocol));
  }

  /** Whether the host declares this capability. */
  declares(capability: HostCapabilityName): boolean {
    return this.#capabilities.has(capability);
  }

  /** Compare declarations regardless of the order a host listed them in. */
  same(other: DeclaredHost): boolean {
    return (
      sameSet(this.#protocols, other.#protocols) && sameSet(this.#capabilities, other.#capabilities)
    );
  }
}

/** Build the description a host sends for one panel session. */
export function hostDescriptionMessage(
  session: string,
  content: HostDescriptionContent,
): HostDescriptionMessage {
  return {
    protocol: 'sugarcubes.host',
    version: 1,
    session,
    type: 'description',
    protocols: content.protocols.map(({ name, version }) => ({ name, version })),
    capabilities: content.capabilities.map(({ name }) => ({ name })),
  };
}

/** Identify one protocol at one envelope version. */
function protocolKey(protocol: HostProtocol): string {
  return `${protocol.name}@${String(protocol.version)}`;
}

/** Compare two name sets by membership. */
function sameSet(left: ReadonlySet<string>, right: ReadonlySet<string>): boolean {
  return left.size === right.size && [...left].every((name) => right.has(name));
}
