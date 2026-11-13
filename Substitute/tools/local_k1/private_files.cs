// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using Microsoft.Win32.SafeHandles;
public static class LocalK1PrivateFiles {
    [StructLayout(LayoutKind.Sequential)] struct SecurityAttributes {
        public uint length; public IntPtr descriptor; public int inherit;
    }
    [StructLayout(LayoutKind.Sequential)] struct FileInfo {
        public uint attributes; public uint creationLow,creationHigh,accessLow,accessHigh,writeLow,writeHigh;
        public uint volume, sizeHigh, sizeLow, links, indexHigh, indexLow;
    }
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern IntPtr CreateFileW(string name,uint access,uint share,IntPtr security,
        uint disposition,uint flags,IntPtr template);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern IntPtr CreateFileW(string name,uint access,uint share,ref SecurityAttributes security,
        uint disposition,uint flags,IntPtr template);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    [return:MarshalAs(UnmanagedType.Bool)]
    static extern bool CreateDirectoryW(string name,ref SecurityAttributes security);
    [DllImport("kernel32.dll", SetLastError=true)]
    [return:MarshalAs(UnmanagedType.Bool)]
    static extern bool GetFileInformationByHandle(SafeFileHandle handle,out FileInfo info);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern uint GetFinalPathNameByHandleW(SafeFileHandle handle,StringBuilder output,uint count,uint flags);
    [DllImport("kernel32.dll", SetLastError=true)]
    [return:MarshalAs(UnmanagedType.Bool)] static extern bool CloseHandle(IntPtr handle);
    // No SafeHandle owns closure: the fixed durable cell adopts output before status.
    public sealed class Lease {
        internal IntPtr raw;
        internal SafeFileHandle borrowed;
        internal Exception cause;
        internal bool uncertain, released;
        public FileStream Stream { get; internal set; }
    }
    static readonly Lease[] files=new Lease[128];
    static readonly object fileGate=new object();
    static Lease Reserve() {
        lock(fileGate) {
            for(int slot=0;slot<files.Length;slot++) if(files[slot]==null) {
                var lease=new Lease();files[slot]=lease;return lease;
            }
        }
        throw new IOException("Signer native file custody capacity retained");
    }
    static void Release(Lease lease) {
        if(lease.raw!=IntPtr.Zero||lease.Stream!=null) return;
        lock(fileGate) {
            for(int slot=0;slot<files.Length;slot++) if(ReferenceEquals(files[slot],lease)) {
                files[slot]=null;lease.released=true;return;
            }
        }
        throw new IOException("Signer native file owner identity lost");
    }
    static Lease OpenOwned(string path,uint access,uint share,uint disposition,bool directory,FileAccess mode,byte[] descriptor=null) {
        if(descriptor!=null&&(descriptor.Length<20||descriptor.Length>8192)) throw new IOException("Protection descriptor bound");
        var lease=Reserve();
        try {
            int error;
            if(descriptor==null) {
                lease.raw=CreateFileW(path,access,share,IntPtr.Zero,disposition,0x02200000,IntPtr.Zero);
                error=Marshal.GetLastWin32Error();
            } else {
                var pin=GCHandle.Alloc(descriptor,GCHandleType.Pinned);
                try {
                    var security=new SecurityAttributes{length=(uint)Marshal.SizeOf<SecurityAttributes>(),descriptor=pin.AddrOfPinnedObject()};
                    lease.raw=CreateFileW(path,access,share,ref security,disposition,0x02200000,IntPtr.Zero);
                    error=Marshal.GetLastWin32Error();
                } finally { pin.Free(); }
            }
            if(lease.raw==new IntPtr(-1)||lease.raw==IntPtr.Zero) {
                lease.raw=IntPtr.Zero;
                throw new Win32Exception(error);
            }
            lease.borrowed=new SafeFileHandle(lease.raw,false);
            Validate(lease,path,directory);
            if(!directory) lease.Stream=new FileStream(lease.borrowed,mode,4096,false);
            return lease;
        } catch(Exception error) {
            lease.cause=error;
            try { Close(lease); } catch(Exception close) { throw new AggregateException(error,close); }
            throw;
        }
    }
    public static Lease Pin(string path) { return OpenOwned(path,0x20080,3,3,true,FileAccess.Read); }
    public static Lease OpenRead(string path) { return OpenOwned(path,0x80020080,1,3,false,FileAccess.Read); }
    public static Lease CreateNew(string path) { return OpenOwned(path,0xc0020080,0,1,false,FileAccess.ReadWrite); }
    public static Lease CreateProtectedNew(string path,byte[] descriptor) {
        if(descriptor==null) throw new ArgumentNullException(nameof(descriptor));
        return OpenOwned(path,0xc0020080,0,1,false,FileAccess.ReadWrite,descriptor);
    }
    // CreateDirectory is creation-only. An existing object is never modified.
    public static void CreateProtectedDirectoryNew(string path,byte[] descriptor) {
        if(descriptor==null||descriptor.Length<20||descriptor.Length>8192) throw new IOException("Protection descriptor bound");
        var pin=GCHandle.Alloc(descriptor,GCHandleType.Pinned);
        try {
            var security=new SecurityAttributes{length=(uint)Marshal.SizeOf<SecurityAttributes>(),descriptor=pin.AddrOfPinnedObject()};
            if(!CreateDirectoryW(path,ref security)) throw new Win32Exception(Marshal.GetLastWin32Error());
        } finally { pin.Free(); }
    }
    public static void Validate(Lease lease,string path,bool directory) {
        var handle=lease.borrowed;
        FileInfo info;
        if(!GetFileInformationByHandle(handle,out info)) throw new Win32Exception(Marshal.GetLastWin32Error());
        if((info.attributes&0x400)!=0 || ((info.attributes&0x10)!=0)!=directory || (!directory&&info.links!=1))
            throw new IOException("Signer object is not an exact regular non-reparse object");
        var name=new StringBuilder(4096);
        var count=GetFinalPathNameByHandleW(handle,name,4096,0);
        if(count==0) throw new Win32Exception(Marshal.GetLastWin32Error());
        var expected=Path.GetFullPath(path); if(expected.Length>3) expected=expected.TrimEnd('\\');
        if(count>=4096 || !String.Equals(name.ToString(),@"\\?\"+expected,StringComparison.OrdinalIgnoreCase))
            throw new IOException("Signer canonical object changed");
    }
    public static void Close(Lease lease) {
        CloseUsing(lease,raw=> {
            if(!CloseHandle(raw)) throw new Win32Exception(Marshal.GetLastWin32Error());
        });
    }
    internal static void CloseUsing(Lease lease,Action<IntPtr> close) {
        if(lease==null) return;
        if(lease.released) return;
        if(lease.uncertain) throw lease.cause;
        if(lease.raw!=IntPtr.Zero) {
            try { close(lease.raw); }
            catch(Exception error) {
                lease.cause=lease.cause==null?error:new AggregateException(lease.cause,error);
                lease.uncertain=true;
                throw;
            }
            lease.raw=IntPtr.Zero;
            if(lease.borrowed!=null) lease.borrowed.SetHandleAsInvalid();
        }
        if(lease.Stream!=null) {
            try { lease.Stream.Dispose();lease.Stream=null; }
            catch(Exception error) { lease.cause=lease.cause==null?error:new AggregateException(lease.cause,error);lease.uncertain=true;throw; }
        }
        if(lease.borrowed!=null) { lease.borrowed.Dispose();lease.borrowed=null; }
        Release(lease);
    }
    public static void WriteNew(string path,byte[] bytes) {
        var lease=CreateNew(path);
        Exception cause=null;
        try { lease.Stream.Write(bytes,0,bytes.Length);lease.Stream.Flush(true); }
        catch(Exception error) { cause=error; }
        try { Close(lease); } catch(Exception close) { cause=cause==null?close:new AggregateException(cause,close); }
        if(cause!=null) throw cause;
    }
    public static void WriteProtectedNew(string path,byte[] bytes,byte[] descriptor) {
        var lease=CreateProtectedNew(path,descriptor);
        Exception cause=null;
        try { lease.Stream.Write(bytes,0,bytes.Length);lease.Stream.Flush(true); }
        catch(Exception error) { cause=error; }
        try { Close(lease); } catch(Exception close) { cause=cause==null?close:new AggregateException(cause,close); }
        if(cause!=null) throw cause;
    }
    public static RSA ImportPrivate(byte[] bytes) {
        var rsa=RSA.Create();
        try { int read; rsa.ImportPkcs8PrivateKey(bytes,out read);
            if(read!=bytes.Length||rsa.KeySize!=3072) throw new CryptographicException("Unexpected private signer encoding");
            return rsa;
        } catch { rsa.Dispose(); throw; }
    }
    public static void Clear(byte[] bytes) { if(bytes!=null) CryptographicOperations.ZeroMemory(bytes); }
}

public static class LocalK1PublicVerification {
    [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct Pss {
        [MarshalAs(UnmanagedType.LPWStr)] public string hash;
        public uint salt;
    }
    [DllImport("bcrypt.dll",CharSet=CharSet.Unicode)] static extern int BCryptOpenAlgorithmProvider(out IntPtr output,string algorithm,string provider,uint flags);
    [DllImport("bcrypt.dll",CharSet=CharSet.Unicode)] static extern int BCryptImportKeyPair(IntPtr provider,IntPtr importKey,string kind,out IntPtr output,byte[] blob,uint count,uint flags);
    [DllImport("bcrypt.dll")] static extern int BCryptVerifySignature(IntPtr key,ref Pss padding,byte[] hash,uint hashCount,byte[] signature,uint signatureCount,uint flags);
    [DllImport("bcrypt.dll")] static extern int BCryptDestroyKey(IntPtr key);
    [DllImport("bcrypt.dll")] static extern int BCryptCloseAlgorithmProvider(IntPtr provider,uint flags);
    // Reserve an actual owner before FFI; a failed retirement keeps all raw state.
    sealed class Owner {
        public IntPtr provider,key;
        public byte[] blob,data,signature,hash;
        public Exception cause;
    }
    static readonly Owner[] owners=new Owner[4];
    static readonly object ownerGate=new object();
    static Owner Reserve(byte[] blob,byte[] data,byte[] signature) {
        lock(ownerGate) {
            for(int slot=0;slot<owners.Length;slot++) {
                if(owners[slot]!=null) continue;
                var owner=new Owner{blob=(byte[])blob.Clone(),data=(byte[])data.Clone(),signature=(byte[])signature.Clone()};
                owners[slot]=owner;return owner;
            }
        }
        throw new CryptographicException("Public verifier ownership capacity retained");
    }
    static void Release(Owner owner) {
        if(owner.provider!=IntPtr.Zero||owner.key!=IntPtr.Zero) return;
        lock(ownerGate) {
            for(int slot=0;slot<owners.Length;slot++) {
                if(ReferenceEquals(owners[slot],owner)) { owners[slot]=null;return; }
            }
        }
        throw new CryptographicException("Public verifier owner identity lost");
    }
    public static bool Verify(byte[] blob,byte[] data,byte[] signature) {
        if(blob==null||data==null||signature==null||blob.Length!=411||signature.Length!=384||data.Length>65536)
            throw new CryptographicException("Public proof bounds");
        var owner=Reserve(blob,data,signature);
        Exception cause=null;
        bool valid=false;
        try {
            int status=BCryptOpenAlgorithmProvider(out owner.provider,"RSA","Microsoft Primitive Provider",0);
            if(status!=0) throw new CryptographicException("CNG provider status0x"+status.ToString("x8"));
            status=BCryptImportKeyPair(owner.provider,IntPtr.Zero,"RSAPUBLICBLOB",out owner.key,owner.blob,(uint)owner.blob.Length,0);
            if(status!=0) throw new CryptographicException("CNG public import status0x"+status.ToString("x8"));
            owner.hash=SHA256.HashData(owner.data);
            var padding=new Pss{hash="SHA256",salt=32};
            status=BCryptVerifySignature(owner.key,ref padding,owner.hash,32,owner.signature,384,8);
            if(status==0) valid=true;
            else if(unchecked((uint)status)!=0xc000a000) throw new CryptographicException("CNG verify status0x"+status.ToString("x8"));
        } catch(Exception error) { cause=error; }
        owner.cause=cause;
        try {
        if(owner.key!=IntPtr.Zero) {
            int status=BCryptDestroyKey(owner.key);
            if(status!=0) {
                var close=new CryptographicException("CNG destroy status0x"+status.ToString("x8"));
                cause=cause==null?close:new AggregateException(cause,close);
            } else owner.key=IntPtr.Zero;
        }
        if(owner.provider!=IntPtr.Zero&&owner.key==IntPtr.Zero) {
            int status=BCryptCloseAlgorithmProvider(owner.provider,0);
            if(status!=0) {
                var close=new CryptographicException("CNG provider close status0x"+status.ToString("x8"));
                cause=cause==null?close:new AggregateException(cause,close);
            } else owner.provider=IntPtr.Zero;
        }
        } catch(Exception close) { cause=cause==null?close:new AggregateException(cause,close); }
        owner.cause=cause;
        Release(owner);
        if(cause!=null) throw cause;
        return valid;
    }
}
