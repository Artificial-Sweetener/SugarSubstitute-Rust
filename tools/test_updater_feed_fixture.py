# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Check exact feed routing and resumable range boundaries without network access."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from tools.updater_feed_fixture import Feed, asset, interval


class FeedBoundaries(unittest.TestCase):
    def test_restarted_fixture_preserves_literal_loopback_and_requested_port(self):
        with patch('tools.updater_feed_fixture.HTTPServer.__init__') as initialize:
            Feed(Path('routes.json'), Path('control'), 10, 43210)
            self.assertEqual(initialize.call_args.args[0], ('127.0.0.1', 43210))
            for value in (-1, 65536, True, '43210'):
                with self.subTest(value=value), self.assertRaises(ValueError):
                    Feed(Path('routes.json'), Path('control'), 10, value)

    def test_full_and_partial_intervals_preserve_exact_byte_offsets(self):
        self.assertEqual(interval(None, 10), (0, 9, 200))
        self.assertEqual(interval('bytes=0-', 10), (0, 9, 206))
        self.assertEqual(interval('bytes=7-', 10), (7, 9, 206))
        self.assertEqual(interval('bytes=3-6', 10), (3, 6, 206))
        self.assertEqual(interval('bytes=3-50', 10), (3, 9, 206))

    def test_invalid_or_ambiguous_ranges_are_rejected(self):
        for value in ('bytes=10-', 'bytes=3-2', 'bytes=-4', 'bytes=00-', 'bytes=1-2,3-4',
                      'items=0-', 'bytes=+1-', 'bytes=0-01'):
            with self.subTest(value=value), self.assertRaises(ValueError):
                interval(value, 10)

    def test_requests_cannot_escape_the_explicit_asset_mapping(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'actual.json'
            path.write_bytes(b'fixture')
            routes = {'timestamp.json': str(path)}
            self.assertEqual(asset(routes, '/timestamp.json'), path)
            for request in ('/actual.json', '/..', '/../actual.json', '/%74imestamp.json',
                            '/timestamp.json?override=1', '/timestamp.json/extra', '//timestamp.json'):
                with self.subTest(request=request):
                    self.assertIsNone(asset(routes, request))
            with self.assertRaises(ValueError):
                asset({'timestamp.json': 'relative.json'}, '/timestamp.json')
            path.unlink()
            self.assertIsNone(asset(routes, '/timestamp.json'))


if __name__ == '__main__':
    unittest.main()
