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
 * Read the colours and weight a console line asks for with ANSI escape sequences.
 *
 * ComfyUI and its custom nodes colour their output with SGR sequences. The console shows them in
 * the palette the SugarSubstitute Python application's console used, and drops every other
 * control sequence so none reaches the page as text.
 */
/** The Python application's console palette, by SGR foreground code. */
const FOREGROUND = {
    30: '#17191c',
    31: '#f66151',
    32: '#33d17a',
    33: '#e5a50a',
    34: '#3584e4',
    35: '#c061cb',
    36: '#33c7de',
    37: '#deddda',
    90: '#77767b',
    91: '#ff7b72',
    92: '#57e389',
    93: '#f8e45c',
    94: '#62a0ea',
    95: '#dc8add',
    96: '#5bc8af',
    97: '#ffffff',
};
/** Any CSI control sequence; only those ending in `m` change the style. */
// eslint-disable-next-line no-control-regex -- ESC is the sequence introducer being parsed.
const CONTROL = /\u001b\[([0-9;?]*)([@-~])/g;
/** Split one line into styled runs, applying the SGR sequences it carries. */
export function parseAnsiLine(line) {
    const spans = [];
    let color = null;
    let bold = false;
    let last = 0;
    const push = (text) => {
        if (!text)
            return;
        const previous = spans.at(-1);
        if (previous && previous.color === color && previous.bold === bold)
            spans[spans.length - 1] = { text: previous.text + text, color, bold };
        else
            spans.push({ text, color, bold });
    };
    for (const match of line.matchAll(CONTROL)) {
        push(line.slice(last, match.index));
        last = match.index + match[0].length;
        if (match[2] !== 'm')
            continue;
        const codes = (match[1] ?? '').split(';').map((code) => (code === '' ? 0 : Number(code)));
        for (const code of codes) {
            if (code === 0) {
                color = null;
                bold = false;
            }
            else if (code === 1)
                bold = true;
            else if (code === 22)
                bold = false;
            else if (code === 39)
                color = null;
            else if (FOREGROUND[code] !== undefined)
                color = FOREGROUND[code] ?? null;
        }
    }
    push(line.slice(last));
    return spans;
}
/** The line's text without any control sequences, as Copy hands it on. */
export function plainAnsiLine(line) {
    return line.replace(CONTROL, '');
}
