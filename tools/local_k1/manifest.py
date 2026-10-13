# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Encode and strictly decode the bounded signed local K1 release payload.

This codec produces matching data. Only signature verification under the embedded
runtime anchor followed by actual pinned-file validation establishes authority.
"""
from dataclasses import dataclass
import re,struct

MAGIC=b'SSRELEASE-K1-v1\0'
CAPTURE_MAGIC=b'SSRELEASE-K1-v2\0'
BOUND=65536
LABEL=re.compile(r'[a-z0-9_-]+\.(exe|dll|pdb)\Z')
BUILD=re.compile(r'[A-Za-z0-9_-]{1,64}\Z')
RESERVED={'con','prn','aux','nul',*(f'com{x}' for x in range(1,10)),*(f'lpt{x}' for x in range(1,10))}
@dataclass(frozen=True)
class Pair:
    roles:int
    pe_label:str
    pdb_label:str
    pe_length:int
    pe_sha256:bytes
    pdb_length:int
    pdb_sha256:bytes
    image_size:int
    timestamp:int
    checksum:int
    pdb_guid:bytes
    pdb_age:int
@dataclass(frozen=True)
class Manifest:
    key_id:bytes
    build:str
    pairs:tuple[Pair,...]
    schema:int=1
class ManifestError(ValueError):
    """Reject a noncanonical, unknown or out-of-bound signed payload."""
def _label(value:str,extensions:set[str])->bytes:
    if not isinstance(value,str) or not LABEL.fullmatch(value) or len(value)>128:
        raise ManifestError('Invalid release leaf')
    stem,extension=value.rsplit('.',1)
    if extension not in extensions or stem in RESERVED:
        raise ManifestError('Invalid release leaf extension/stem')
    return value.encode('ascii')
def _bytes(value:bytes,count:int)->None:
    if not isinstance(value,bytes) or len(value)!=count:
        raise ManifestError('Invalid fixed release field')
def _number(value:int,bits:int,positive:bool=False)->None:
    if type(value) is not int or not (int(positive)<=value<1<<bits):
        raise ManifestError('Invalid release number')
def validate(value:Manifest)->None:
    if type(value.schema) is not int or value.schema not in (1,2):
        raise ManifestError('Unknown release schema')
    _bytes(value.key_id,32)
    if not isinstance(value.build,str) or not BUILD.fullmatch(value.build):
        raise ManifestError('Invalid release build')
    if not isinstance(value.pairs,tuple) or not 1<=len(value.pairs)<=16:
        raise ManifestError('Invalid release count')
    names=set();host=0;helper=0;client=0;handler=0;previous=''
    for pair in value.pairs:
        _number(pair.roles,8,True)
        if pair.roles&~(31 if value.schema==2 else 7):raise ManifestError('Unknown release role')
        _label(pair.pe_label,{'exe','dll'});_label(pair.pdb_label,{'pdb'})
        if pair.pe_label<=previous:raise ManifestError('Release order/alias')
        previous=pair.pe_label
        for name in (pair.pe_label,pair.pdb_label):
            if name in names:raise ManifestError('Release alias')
            names.add(name)
        if pair.roles&3 and not pair.pe_label.endswith('.exe'):
            raise ManifestError('Role must identify an executable')
        host+=bool(pair.roles&1);helper+=bool(pair.roles&2)
        if pair.roles&8:
            client+=1
            if pair.roles&~12 or (pair.pe_label,pair.pdb_label)!=('sugarsubstitute_crashpad_client.dll','sugarsubstitute_crashpad_client.pdb'):
                raise ManifestError('Invalid fixed capture client role')
        if pair.roles&16:
            handler+=1
            if pair.roles&~20 or (pair.pe_label,pair.pdb_label)!=('crashpad_handler.exe','crashpad_handler.pdb'):
                raise ManifestError('Invalid fixed capture handler role')
        _number(pair.pe_length,64,True);_number(pair.pdb_length,64,True)
        _bytes(pair.pe_sha256,32);_bytes(pair.pdb_sha256,32);_bytes(pair.pdb_guid,16)
        if pair.pdb_guid==bytes(16):raise ManifestError('Absent PDB GUID')
        for field in (pair.image_size,pair.pdb_age):_number(field,32,True)
        for field in (pair.timestamp,pair.checksum):_number(field,32)
    if host!=1 or helper!=1:raise ManifestError('Missing/ambiguous release roles')
    if client>1 or handler>1:raise ManifestError('Ambiguous capture roles')
    host_pair=next(pair for pair in value.pairs if pair.roles&1)
    helper_pair=next(pair for pair in value.pairs if pair.roles&2)
    if host_pair!=helper_pair:raise ManifestError('Current host/helper must be same executable')

def capture_pairs(value:Manifest)->tuple[Pair,Pair]:
    """Select matching schema2 records; no paths, signatures or authority are produced."""
    validate(value)
    if value.schema!=2:raise ManifestError('Capture requires schema2')
    client=next((pair for pair in value.pairs if pair.roles&8),None)
    handler=next((pair for pair in value.pairs if pair.roles&16),None)
    if client is None or handler is None:raise ManifestError('Missing capture roles')
    return client,handler
def encode(value:Manifest)->bytes:
    validate(value)
    build=value.build.encode('ascii')
    magic=CAPTURE_MAGIC if value.schema==2 else MAGIC
    output=bytearray(magic+struct.pack('<IB',value.schema,1)+value.key_id+bytes([len(build)])+build+struct.pack('<HB',0x8664,len(value.pairs)))
    for pair in value.pairs:
        pe=pair.pe_label.encode('ascii');pdb=pair.pdb_label.encode('ascii')
        output+=bytes([pair.roles,len(pe)])+pe+bytes([len(pdb)])+pdb
        output+=struct.pack('<Q',pair.pe_length)+pair.pe_sha256+struct.pack('<Q',pair.pdb_length)+pair.pdb_sha256
        output+=struct.pack('<III',pair.image_size,pair.timestamp,pair.checksum)+pair.pdb_guid+struct.pack('<I',pair.pdb_age)
    if len(output)>BOUND:raise ManifestError('Release bound')
    return bytes(output)
class _Reader:
    def __init__(self,value:bytes):
        if not isinstance(value,bytes) or not 1<=len(value)<=BOUND:raise ManifestError('Release bound')
        self.value=value;self.offset=0
    def take(self,count:int)->bytes:
        if count<0 or self.offset+count>len(self.value):raise ManifestError('Truncated release')
        output=self.value[self.offset:self.offset+count];self.offset+=count;return output
    def number(self,size:int)->int:return int.from_bytes(self.take(size),'little')
    def text(self)->str:
        count=self.number(1)
        try:return self.take(count).decode('ascii')
        except UnicodeDecodeError as error:raise ManifestError('Non-ASCII release') from error
def decode(value:bytes)->Manifest:
    source=_Reader(value)
    magic=source.take(16);schema=source.number(4)
    if (magic,schema) not in ((MAGIC,1),(CAPTURE_MAGIC,2)) or source.number(1)!=1:
        raise ManifestError('Unknown release schema/identity')
    key=source.take(32);build=source.text()
    if source.number(2)!=0x8664:raise ManifestError('Release machine')
    count=source.number(1)
    if not 1<=count<=16:raise ManifestError('Release count')
    pairs=[]
    for _ in range(count):
        roles=source.number(1);pe=source.text();pdb=source.text()
        pairs.append(Pair(roles,pe,pdb,source.number(8),source.take(32),source.number(8),source.take(32),
            source.number(4),source.number(4),source.number(4),source.take(16),source.number(4)))
    if source.offset!=len(value):raise ManifestError('Trailing release bytes')
    result=Manifest(key,build,tuple(pairs),schema);validate(result);return result
