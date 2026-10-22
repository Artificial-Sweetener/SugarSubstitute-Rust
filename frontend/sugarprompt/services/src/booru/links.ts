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
/** Supported booru links delegate external lookup and ordinary tag formatting without editor state or networking. */
import { requireValue } from '../../../core/src/required.js';
import { parseWeight } from '../../../core/src/prompt/weights.js';
import { tagReplacementText } from '../../../core/src/completion/text.js';
import { strip, whitespace } from '../../../core/src/prompt/text.js';
/** Lookup keys contain only the validated endpoint identity, never a caller-selected network destination. */
export type BooruLookupKey =
  | {
      readonly kind: 'post';
      readonly id: string;
    }
  | {
      readonly kind: 'cdn';
      readonly md5: string;
    };
/** The supported original literal URL remains available to the editor's fallback owner. */
export type BooruLink = BooruLookupKey & {
  readonly url: string;
};
/** Hosts return an unknown payload for validation and own transport cancellation through the supplied signal. */
export type BooruPostLookup = (key: BooruLookupKey, signal: AbortSignal) => Promise<unknown>;
/** Validated prompt-relevant API fields exclude media and source URLs entirely. */
interface BooruPost {
  readonly id: number;
  readonly md5: string | null;
  readonly tag_string_general: string;
  readonly tag_string_artist: string;
  readonly tag_string_copyright: string;
  readonly tag_string_character: string;
  readonly tag_string_meta: string;
}
/** Only ready text is an import suggestion; every other outcome preserves the caller's literal fallback. */
export type BooruImportOutcome =
  | {
      readonly status: 'ready';
      readonly link: BooruLink;
      readonly postId: number;
      readonly text: string;
      readonly includedTags: readonly string[];
      readonly excludedTags: readonly string[];
    }
  | {
      readonly status:
        | 'unsupported'
        | 'not-found'
        | 'unavailable'
        | 'invalid-response'
        | 'cancelled';
    };
/** Classify exact supported paths without URL normalization turning unsupported authored paths into accepted ones. */
export function classifyBooruLink(text: string): BooruLink | null {
  const url = strip(text);
  // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
  if (/[\u0000-\u0020\u007f\\]/u.test(url)) return null;
  const match = /^(https?):\/\/([^/?#]+)(\/[^?#]*)?(?:[?#].*)?$/iu.exec(url);
  if (!match) return null;
  const host = requireValue(match[2], 'links').toLowerCase(),
    path = match[3] ?? '';
  if (host === 'danbooru.donmai.us' || host === 'www.danbooru.donmai.us') {
    const post = /^\/posts\/([^/]+)$/u.exec(path);
    const digits = post?.[1];
    const id = digits && !/[+.-]/u.test(digits) ? parseWeight(digits, false) : null;
    return id !== null
      ? Object.freeze({
          kind: 'post',
          id,
          url,
        })
      : null;
  }
  if (host !== 'cdn.donmai.us') return null;
  const media = /([0-9a-f]{32})\.[a-z0-9]+$/iu.exec(path.slice(path.lastIndexOf('/') + 1));
  return media
    ? Object.freeze({ kind: 'cdn', md5: requireValue(media[1], 'links').toLowerCase(), url })
    : null;
}
/** Capture each foreign scalar once before validating the immutable local record; unexpected accessor/proxy causes remain observable. */
function postSnapshot(value: unknown): BooruPost | null {
  if (!value || typeof value !== 'object') return null;
  const {
    id,
    md5,
    tag_string_general,
    tag_string_artist,
    tag_string_copyright,
    tag_string_character,
    tag_string_meta,
  } = value as Record<string, unknown>;
  if (
    typeof id !== 'number' ||
    !Number.isSafeInteger(id) ||
    id <= 0 ||
    !(md5 === null || (typeof md5 === 'string' && /^[0-9a-f]{32}$/iu.test(md5))) ||
    typeof tag_string_general !== 'string' ||
    typeof tag_string_artist !== 'string' ||
    typeof tag_string_copyright !== 'string' ||
    typeof tag_string_character !== 'string' ||
    typeof tag_string_meta !== 'string'
  )
    return null;
  return Object.freeze({
    id,
    md5,
    tag_string_general,
    tag_string_artist,
    tag_string_copyright,
    tag_string_character,
    tag_string_meta,
  });
}
/** Stable exact-tag deduplication delegates Unicode whitespace to the existing canonical text owner. */
function tags(groups: readonly string[]): readonly string[] {
  return Object.freeze([
    ...new Set(
      groups.flatMap((group) =>
        strip(group)
          .split(new RegExp(`${whitespace.source}+`, 'u'))
          .filter(Boolean),
      ),
    ),
  ]);
}
/** One call owns only its abort listener; cancellation settles without allowing an ignored late lookup to publish. */
export async function importBooruLink(
  text: string,
  lookup: BooruPostLookup,
  signal: AbortSignal,
): Promise<BooruImportOutcome> {
  if (signal.aborted) return { status: 'cancelled' };
  const link = classifyBooruLink(text);
  if (!link) return { status: 'unsupported' };
  let cancel!: () => void;
  const cancelled = new Promise<null>((resolve) => {
    cancel = () => resolve(null);
  });
  signal.addEventListener('abort', cancel, { once: true });
  try {
    const key: BooruLookupKey = Object.freeze(
      link.kind === 'post' ? { kind: 'post', id: link.id } : { kind: 'cdn', md5: link.md5 },
    );
    const response: unknown = await Promise.race([
      Promise.resolve().then(() => (signal.aborted ? null : lookup(key, signal))),
      cancelled,
    ]);
    if (signal.aborted) return { status: 'cancelled' };
    if (!response || typeof response !== 'object') return { status: 'invalid-response' };
    const external = response as Record<string, unknown>;
    const status = external.status;
    const post = status === 'found' ? postSnapshot(external.post) : null;
    if (signal.aborted) return { status: 'cancelled' };
    if (status === 'not-found' || status === 'unavailable') return { status };
    if (!post) return { status: 'invalid-response' };
    if (link.kind === 'post' ? String(post.id) !== link.id : post.md5?.toLowerCase() !== link.md5)
      return { status: 'invalid-response' };
    const includedTags = tags([
      post.tag_string_general,
      post.tag_string_artist,
      post.tag_string_copyright,
      post.tag_string_character,
    ]);
    if (signal.aborted) return { status: 'cancelled' };
    return Object.freeze({
      status: 'ready',
      link,
      postId: post.id,
      text: includedTags.map(tagReplacementText).join(', '),
      includedTags,
      excludedTags: tags([post.tag_string_meta]),
    });
  } finally {
    signal.removeEventListener('abort', cancel);
  }
}
