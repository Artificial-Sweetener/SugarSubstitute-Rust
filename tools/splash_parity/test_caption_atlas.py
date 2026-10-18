# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Reject malformed resource dimensions and preserve sparse original-path coverage."""
import struct
import unittest

from tools.splash_parity.caption_atlas import encode_atlas, encode_path


class CaptionAtlasTests(unittest.TestCase):
    """Exercise exact sparse coverage and finite index boundaries without native Qt."""

    def test_sparse_path_preserves_positions_and_fractional_alpha(self) -> None:
        """Keep each nonzero pixel and omit only fully transparent coverage."""
        self.assertEqual(encode_path(3, 2, bytes([0, 26, 0, 255, 0, 51])),
                         struct.pack("<IHHHBHHHBHHHB", 3, 0, 1, 1, 26, 1, 0, 1, 255, 1, 2, 1, 51))

    def test_invalid_shapes_and_dpi_ranges_are_rejected(self) -> None:
        """Reject truncated pixels and values outside the compiled resource contract."""
        for width, height, data in [(0, 1, b""), (1, 0, b""), (1, 2, b"\0"), (65536, 1, b"")]:
            with self.assertRaises(ValueError):
                encode_path(width, height, data)
        for first, records in [(0, [b"a"]), (48, []), (768, [b"a", b"b"])]:
            with self.assertRaises(ValueError):
                encode_atlas(first, records)

    def test_index_covers_exact_records_and_terminal_boundary(self) -> None:
        """Keep both per-DPI paths inside exact, nonoverlapping indexed records."""
        encoded = encode_atlas(96, [b"first", b"second"])
        self.assertEqual(struct.unpack_from("<8sHHI", encoded), (b"SSCAPA1\0", 96, 97, 2))
        a, b, end = struct.unpack_from("<III", encoded, 16)
        self.assertEqual(encoded[a:b], b"first")
        self.assertEqual(encoded[b:end], b"second")
        self.assertEqual(end, len(encoded))


if __name__ == "__main__":
    unittest.main()
