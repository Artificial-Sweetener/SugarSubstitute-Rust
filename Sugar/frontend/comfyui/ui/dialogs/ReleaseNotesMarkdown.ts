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
/**
 * Render a changelog's Markdown as safe DOM: headings, nested lists, paragraphs, links, bold and
 * code, the forms semantic-release writes.
 *
 * Every character of the notes reaches the page through `textContent`; nothing is parsed as
 * HTML, so markup in the notes shows literally. Only `https` links become links, and they open
 * outside the page; anything else stays plain text.
 */

const HEADING = /^(#{1,6})\s+(.*)$/;
const ITEM = /^(\s*)[*-]\s+(.*)$/;
const INLINE = /\[([^\]\n]+)\]\(([^)\s]+)\)|\*\*([^*\n]+)\*\*|`([^`\n]+)`/g;

/** Append the inline forms of `text` to `parent`. */
function appendInline(document: Document, parent: HTMLElement, text: string): void {
  let last = 0;
  for (const match of text.matchAll(INLINE)) {
    const index = match.index;
    if (index > last) parent.append(document.createTextNode(text.slice(last, index)));
    const [whole, label, href, bold, code] = match;
    if (label !== undefined && href !== undefined) parent.append(link(document, label, href));
    else if (bold !== undefined) {
      const strong = document.createElement('strong');
      strong.textContent = bold;
      parent.append(strong);
    } else if (code !== undefined) {
      const element = document.createElement('code');
      element.textContent = code;
      parent.append(element);
    } else parent.append(document.createTextNode(whole));
    last = index + whole.length;
  }
  if (last < text.length) parent.append(document.createTextNode(text.slice(last)));
}

/** An external `https` link, or the label as text for any other address. */
function link(document: Document, label: string, href: string): Node {
  let url: URL | null = null;
  try {
    url = new URL(href);
  } catch {
    url = null;
  }
  if (!url || url.protocol !== 'https:' || url.username || url.password)
    return document.createTextNode(label);
  const anchor = document.createElement('a');
  anchor.href = url.href;
  anchor.target = '_blank';
  anchor.rel = 'noopener noreferrer';
  anchor.textContent = label;
  return anchor;
}

/** Render one version's notes. */
export function renderReleaseNotes(document: Document, markdown: string): DocumentFragment {
  const fragment = document.createDocumentFragment();
  let paragraph: string[] = [];
  /** Open lists by depth; the innermost list is last. */
  let lists: { indent: number; element: HTMLUListElement }[] = [];
  const flushParagraph = (): void => {
    if (paragraph.length === 0) return;
    const element = document.createElement('p');
    appendInline(document, element, paragraph.join(' '));
    fragment.append(element);
    paragraph = [];
  };
  for (const line of markdown.replaceAll('\r\n', '\n').split('\n')) {
    const heading = HEADING.exec(line);
    const item = ITEM.exec(line);
    if (line.trim() === '' || heading) {
      flushParagraph();
      lists = [];
    }
    if (heading) {
      const level = Math.min(6, (heading[1]?.length ?? 1) + 3);
      const element = document.createElement(`h${String(level)}`);
      appendInline(document, element, (heading[2] ?? '').trim());
      fragment.append(element);
    } else if (item) {
      flushParagraph();
      const indent = item[1]?.length ?? 0;
      while (lists.length > 0 && (lists[lists.length - 1]?.indent ?? 0) > indent) lists.pop();
      let list = lists[lists.length - 1];
      if (!list || list.indent < indent) {
        const element = document.createElement('ul');
        const parent = list?.element.lastElementChild;
        if (parent) parent.append(element);
        else fragment.append(element);
        list = { indent, element };
        lists.push(list);
      }
      const entry = document.createElement('li');
      appendInline(document, entry, (item[2] ?? '').trim());
      list.element.append(entry);
    } else if (line.trim() !== '') {
      lists = [];
      paragraph.push(line.trim());
    }
  }
  flushParagraph();
  return fragment;
}
