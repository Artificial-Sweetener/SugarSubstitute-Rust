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
/** Controlled declarations transport only native CSSOM property, serialization and replacement operations consumed by paint owners. */
export interface ReorderPaintStyleFixture
  extends Record<string, string | ((name: string, value: string, priority?: string) => void)> {
  cssText: string;
  setProperty(name: string, value: string, priority?: string): void;
  getPropertyValue(name: string): string;
  getPropertyPriority(name: string): string;
}
/** One finite declaration map owns direct properties and serialized attributes; native proof owns CSS parsing/layout/paint. */
export function reorderPaintStyleFixture(properties = new Map<string, string>()) {
  const priorities = new Map<string, string>();
  /** Native named properties resolve to their declaration spelling without another mutable mirror. */
  const declaration = (name: string): string =>
    name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
  /** Serialization is the native declaration form, independent of assignment spelling. */
  const serialize = (): string =>
    [...properties]
      .map(([name, value]) => `${name}: ${value}${priorities.get(name) ? ' !important' : ''};`)
      .join(' ');
  const style = {
    setProperty(name: string, value: string, priority = ''): void {
      if (priority && priority !== 'important') return;
      const key = name.trim(),
        resolved = value.trim();
      if (resolved) {
        properties.set(key, resolved);
        priorities.set(key, priority);
      } else {
        properties.delete(key);
        priorities.delete(key);
      }
    },
    getPropertyValue(name: string): string {
      return properties.get(name) ?? '';
    },
    /** Return the priority from the same finite declaration entry used by native serialization. */
    getPropertyPriority(name: string): string {
      return priorities.get(name) ?? '';
    },
  } as ReorderPaintStyleFixture;
  Object.defineProperty(style, 'cssText', {
    configurable: true,
    get: serialize,
    set(value: string): void {
      properties.clear();
      priorities.clear();
      let start = 0,
        quote = '',
        escaped = false,
        depth = 0;
      for (let at = 0; at < value.length; ++at) {
        const character = value[at]!;
        if (escaped) {
          escaped = false;
          continue;
        }
        if (character === '\\') {
          escaped = true;
          continue;
        }
        if (quote) {
          if (character === quote) quote = '';
          continue;
        }
        if (character === '"' || character === "'") {
          quote = character;
          continue;
        }
        if (character === '(') ++depth;
        else if (character === ')') --depth;
        else if (character === ';' && depth === 0) {
          const entry = value.slice(start, at).trim(),
            colon = entry.indexOf(':');
          if (entry && colon <= 0) throw new Error('controlled-cssom-declaration-invalid');
          if (entry) {
            const resolved = entry.slice(colon + 1).trim(),
              important = /\s*!important$/u.test(resolved);
            style.setProperty(
              entry.slice(0, colon).trim(),
              resolved.replace(/\s*!important$/u, ''),
              important ? 'important' : '',
            );
          }
          start = at + 1;
        }
      }
      if (value.slice(start).trim() || quote || depth || escaped)
        throw new Error('controlled-cssom-serialization-incomplete');
    },
  });
  // These two finite native setters expose the actual observer frontier used by DocView transport controls.
  for (const [property, name] of [
    ['height', 'height'],
    ['flexBasis', 'flex-basis'],
  ] as const) {
    Object.defineProperty(style, property, {
      configurable: true,
      get: () => properties.get(name) ?? '',
      set: (value: string) => style.setProperty(name, value),
    });
  }
  return new Proxy(style, {
    get(target, property, receiver) {
      if (typeof property !== 'string' || Reflect.has(target, property))
        return Reflect.get(target, property, receiver);
      return properties.get(declaration(property)) ?? '';
    },
    set(target, property, value: unknown, receiver) {
      if (property === 'cssText' || typeof property !== 'string' || Reflect.has(target, property))
        return Reflect.set(target, property, value, receiver);
      if (typeof value !== 'string') throw new Error('controlled-cssom-property-invalid');
      target.setProperty(declaration(property), value);
      return true;
    },
    ownKeys(target) {
      return [...new Set([...Reflect.ownKeys(target), ...properties.keys()])];
    },
    getOwnPropertyDescriptor(target, property) {
      const owned = Reflect.getOwnPropertyDescriptor(target, property);
      if (owned) return owned;
      if (typeof property === 'string' && properties.has(property))
        return {
          configurable: true,
          enumerable: true,
          writable: true,
          value: properties.get(property),
        };
      return Reflect.getOwnPropertyDescriptor(target, property);
    },
  });
}
