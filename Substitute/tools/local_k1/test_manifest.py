# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Exercise the actual bounded release wire codec and canonical role constraints."""
import unittest
from dataclasses import replace
from manifest import Pair,Manifest,ManifestError,encode,decode,capture_pairs,BOUND

def pair(**changes):
    return replace(Pair(7,'sugarsubstitute.exe','sugarsubstitute.pdb',1,bytes([1])*32,2,bytes([2])*32,4096,0,0,bytes([3])*16,1),**changes)
def release(**changes):return replace(Manifest(bytes([4])*32,'k1_actual', (pair(),)),**changes)
class ManifestTests(unittest.TestCase):
    def capture(self):
        entries=(pair(roles=20,pe_label='crashpad_handler.exe',pdb_label='crashpad_handler.pdb'),pair(),
            pair(roles=12,pe_label='sugarsubstitute_crashpad_client.dll',pdb_label='sugarsubstitute_crashpad_client.pdb'))
        return release(pairs=entries,schema=2)
    def test_capture_schema2_roundtrip_and_same_manifest_role_projection(self):
        value=self.capture();self.assertEqual(decode(encode(value)),value)
        client,handler=capture_pairs(value)
        self.assertIs(client,value.pairs[2]);self.assertIs(handler,value.pairs[0])
    def test_capture_missing_roles_and_schema1_selection_reject(self):
        value=self.capture()
        for missing in (release(),replace(value,pairs=value.pairs[:2]),replace(value,pairs=value.pairs[1:])):
            with self.assertRaises(ManifestError):capture_pairs(missing)
        with self.assertRaises(ManifestError):encode(replace(value,schema=1))
    def test_capture_wrong_labels_combined_roles_and_unknown_bits_reject(self):
        value=self.capture()
        for index,changes in [(0,{'roles':24}),(0,{'roles':19}),(0,{'pe_label':'other.exe'}),
            (0,{'pdb_label':'other.pdb'}),(2,{'roles':15}),(2,{'roles':32}),
            (2,{'pe_label':'sugarsubstitute_crashpad_client.exe'}),(2,{'pdb_label':'other.pdb'})]:
            entries=list(value.pairs);entries[index]=replace(entries[index],**changes)
            with self.subTest(index=index,changes=changes),self.assertRaises(ManifestError):encode(replace(value,pairs=tuple(entries)))
    def test_capture_every_truncated_prefix_trailing_and_mismatched_schema_reject(self):
        wire=encode(self.capture())
        for length in range(len(wire)):
            with self.subTest(length=length),self.assertRaises(ManifestError):decode(wire[:length])
        for changed in (wire+b'\0',wire[:16]+bytes([1])+wire[17:]):
            with self.assertRaises(ManifestError):decode(changed)
    def test_actual_roundtrip_same_executable_roles_and_pdb_facts(self):
        value=release();self.assertEqual(decode(encode(value)),value)
    def test_every_truncated_prefix_and_trailing_byte_rejects(self):
        value=encode(release())
        for length in range(len(value)):
            with self.subTest(length=length),self.assertRaises(ManifestError):decode(value[:length])
        with self.assertRaises(ManifestError):decode(value+b'\0')
    def test_unknown_schema_kind_architecture_and_count_reject(self):
        value=bytearray(encode(release()));build_length=value[53];machine=54+build_length;count=machine+2
        for offset,new in [(0,0),(16,2),(20,2),(machine,0),(count,0),(count,17)]:
            changed=value.copy();changed[offset]=new
            with self.subTest(offset=offset),self.assertRaises(ManifestError):decode(bytes(changed))
    def test_invalid_labels_roles_hash_guid_age_and_numeric_bounds(self):
        invalid=[{'pe_label':'CON.exe'},{'pe_label':'con.exe'},{'pe_label':'../a.exe'},
            {'pe_label':'A.exe'},{'pe_label':'a..exe'},{'pe_label':'a.exe '},{'pe_label':'a:p.exe'},
            {'pe_label':'a.dll','roles':7},{'pdb_label':'a.exe'},{'roles':0},{'roles':8},
            {'pe_length':0},{'pdb_length':1<<64},{'image_size':0},{'pdb_age':0},
            {'pdb_guid':bytes(16)},{'pe_sha256':bytes(31)},{'timestamp':-1},{'checksum':True}]
        for changes in invalid:
            with self.subTest(changes=changes),self.assertRaises(ManifestError):encode(release(pairs=(pair(**changes),)))
    def test_missing_ambiguous_split_roles_and_aliases_reject(self):
        invalid=[(),(pair(roles=4),),(pair(roles=1),pair(roles=2,pe_label='z.exe',pdb_label='z.pdb')),
            (pair(),pair(pe_label='z.exe',pdb_label='z.pdb')),
            (pair(),pair(roles=4,pe_label='z.exe'))]
        for pairs in invalid:
            with self.subTest(pairs=pairs),self.assertRaises(ManifestError):encode(release(pairs=pairs))
    def test_sixteen_maximum_records_fit_strict_ingress_and_seventeenth_rejects(self):
        entries=tuple(pair(roles=7 if x==0 else 4,pe_label=f'a{x:02d}_'+('x'*119)+'.exe',pdb_label=f'a{x:02d}_'+('x'*119)+'.pdb') for x in range(16))
        value=release(pairs=entries);raw=encode(value);self.assertLess(len(raw),8192);self.assertEqual(decode(raw),value)
        with self.assertRaises(ManifestError):encode(release(pairs=entries+(pair(),)))
        with self.assertRaises(ManifestError):decode(bytes(BOUND+1))
    def test_build_and_fixed_key_sizes_reject_before_serialization(self):
        for value in [release(build=''),release(build='x'*65),release(build='k1/other'),release(key_id=bytes(31))]:
            with self.subTest(value=value),self.assertRaises(ManifestError):encode(value)
if __name__=='__main__':unittest.main()
