//! Minimal ELF reader: which shared libraries does a binary actually link?
//!
//! Added in v1.4.2 for issue #16. The soname rule from v1.3.1 reads the
//! *declared* dependency graph, and a package can link a library it never
//! declares at the right version. chaotic-aur's `ffmpeg-obs` declared plain
//! `libbluray` while its `libavformat.so.63` linked `libbluray.so.3`, so when
//! `libbluray` moved to `.so.4` nothing declared stood in the way, pacman was
//! satisfied, and nine packages broke silently.
//!
//! The truth lives in each binary's dynamic section: one `DT_NEEDED` entry per
//! library the loader must find. This module reads exactly that and nothing
//! else — no symbols, no relocations, no section headers — which keeps it to
//! a few small reads per file and avoids taking on a parsing dependency.
//!
//! Little-endian only. Arch's supported targets (x86_64, and i686 for the
//! `lib32-` packages) are both little-endian; anything else is reported as
//! not-an-ELF and skipped, which is the safe direction: an unread file can
//! only cause a missed hold, never a spurious one.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// ELF word size. It is part of the soname identity: `libEGL.so=1-32` and
/// `libEGL.so=1-64` are different libraries, and a 32-bit binary needing
/// `libEGL.so.1` is never satisfied by the 64-bit one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Elf32,
    Elf64,
}

/// Upper bound on a string table nog will read. Real ones are a few KB; a
/// value far beyond this is a corrupt header, and reading it could allocate
/// gigabytes on a single bad file.
const MAX_STRTAB: u64 = 4 * 1024 * 1024;
/// Same idea for the dynamic segment and program header table.
const MAX_TABLE: u64 = 1024 * 1024;

const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const DT_NULL: u64 = 0;
const DT_NEEDED: u64 = 1;
const DT_STRTAB: u64 = 5;
const DT_STRSZ: u64 = 10;

/// The libraries a file links, or `None` if it is not a dynamically linked
/// little-endian ELF executable or shared object.
pub fn needed(path: &Path) -> Option<(Class, Vec<String>)> {
    let mut f = File::open(path).ok()?;
    read_needed(&mut f)
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}
fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn u64_at(b: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(o..o + 8)?.try_into().ok()?))
}

fn read_at<R: Read + Seek>(r: &mut R, offset: u64, len: u64) -> Option<Vec<u8>> {
    r.seek(SeekFrom::Start(offset)).ok()?;
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf).ok()?;
    Some(buf)
}

struct Load {
    vaddr: u64,
    offset: u64,
    filesz: u64,
}

/// The parser proper, over any seekable reader so tests can feed it bytes.
pub fn read_needed<R: Read + Seek>(r: &mut R) -> Option<(Class, Vec<String>)> {
    let mut ident = [0u8; 64];
    let n = r.read(&mut ident).ok()?;
    if n < 52 || &ident[..4] != b"\x7fELF" || ident[5] != 1 {
        return None;
    }
    let class = match ident[4] {
        1 => Class::Elf32,
        2 => Class::Elf64,
        _ => return None,
    };
    let is64 = class == Class::Elf64;
    if is64 && n < 64 {
        return None;
    }

    // ET_EXEC or ET_DYN. Object files and core dumps have no dynamic section
    // the loader will act on.
    let e_type = u16_at(&ident, 16)?;
    if e_type != 2 && e_type != 3 {
        return None;
    }

    let (phoff, phentsize, phnum) = if is64 {
        (u64_at(&ident, 32)?, u16_at(&ident, 54)? as u64, u16_at(&ident, 56)? as u64)
    } else {
        (u32_at(&ident, 28)? as u64, u16_at(&ident, 42)? as u64, u16_at(&ident, 44)? as u64)
    };
    let min_ent = if is64 { 56 } else { 32 };
    if phentsize < min_ent || phnum == 0 || phentsize * phnum > MAX_TABLE {
        return None;
    }
    let ph = read_at(r, phoff, phentsize * phnum)?;

    let mut loads: Vec<Load> = Vec::new();
    let mut dynamic: Option<(u64, u64)> = None;
    for i in 0..phnum as usize {
        let b = &ph[i * phentsize as usize..(i + 1) * phentsize as usize];
        let p_type = u32_at(b, 0)?;
        // (offset, vaddr, filesz) sit at different places in the two layouts.
        let (offset, vaddr, filesz) = if is64 {
            (u64_at(b, 8)?, u64_at(b, 16)?, u64_at(b, 32)?)
        } else {
            (u32_at(b, 4)? as u64, u32_at(b, 8)? as u64, u32_at(b, 16)? as u64)
        };
        match p_type {
            PT_LOAD => loads.push(Load { vaddr, offset, filesz }),
            PT_DYNAMIC => dynamic = Some((offset, filesz)),
            _ => {}
        }
    }
    // Statically linked: nothing to need.
    let (dyn_off, dyn_size) = dynamic?;
    if dyn_size > MAX_TABLE {
        return None;
    }
    let dyn_bytes = read_at(r, dyn_off, dyn_size)?;

    let ent = if is64 { 16 } else { 8 };
    let mut needed_offsets: Vec<u64> = Vec::new();
    let mut strtab: Option<u64> = None;
    let mut strsz: Option<u64> = None;
    for chunk in dyn_bytes.chunks_exact(ent) {
        let (tag, val) = if is64 {
            (u64_at(chunk, 0)?, u64_at(chunk, 8)?)
        } else {
            (u32_at(chunk, 0)? as u64, u32_at(chunk, 4)? as u64)
        };
        match tag {
            DT_NULL => break,
            DT_NEEDED => needed_offsets.push(val),
            DT_STRTAB => strtab = Some(val),
            DT_STRSZ => strsz = Some(val),
            _ => {}
        }
    }
    if needed_offsets.is_empty() {
        return Some((class, Vec::new()));
    }

    // DT_STRTAB is a virtual address; translate through the PT_LOAD segment
    // that maps it to find where the bytes sit in the file.
    let (addr, size) = (strtab?, strsz?);
    if size == 0 || size > MAX_STRTAB {
        return None;
    }
    let seg = loads
        .iter()
        .find(|l| addr >= l.vaddr && addr - l.vaddr < l.filesz)?;
    let str_off = seg.offset + (addr - seg.vaddr);
    let table = read_at(r, str_off, size)?;

    let names = needed_offsets
        .into_iter()
        .filter_map(|o| {
            let start = o as usize;
            let rest = table.get(start..)?;
            let end = rest.iter().position(|&c| c == 0)?;
            String::from_utf8(rest[..end].to_vec()).ok()
        })
        .collect();
    Some((class, names))
}

/// Translate a soname provide into the filename a binary's `DT_NEEDED` would
/// carry, and the class it applies to: `libbluray.so=3-64` →
/// (`libbluray.so.3`, Elf64).
///
/// `None` for anything that does not end in a `-32` / `-64` class suffix: an
/// unsuffixed entry cannot be tied to one word size, and guessing would let a
/// 32-bit binary hold a 64-bit library or the reverse.
pub fn provide_to_needed(entry: &str) -> Option<(String, Class)> {
    let (base, rest) = entry.split_once('=')?;
    let (ver, bits) = rest.rsplit_once('-')?;
    let class = match bits {
        "32" => Class::Elf32,
        "64" => Class::Elf64,
        _ => return None,
    };
    if !base.ends_with(".so") || ver.is_empty() {
        return None;
    }
    Some((format!("{}.{}", base, ver), class))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Build the smallest ELF the parser accepts: header, one PT_LOAD mapping
    /// the whole file at `VADDR`, one PT_DYNAMIC, a dynamic section and a
    /// string table. Enough to prove the offset arithmetic in both layouts
    /// without depending on whatever binaries the build machine happens to have.
    fn build(class: Class, e_type: u16, needed: &[&str], with_dynamic: bool) -> Vec<u8> {
        const VADDR: u64 = 0x40_0000;
        let is64 = class == Class::Elf64;
        let (ehsize, phentsize, dynent) = if is64 { (64u64, 56u64, 16u64) } else { (52, 32, 8) };
        let phnum = if with_dynamic { 2 } else { 1 };
        let phoff = ehsize;

        let mut strtab = vec![0u8];
        let mut offs = Vec::new();
        for n in needed {
            offs.push(strtab.len() as u64);
            strtab.extend_from_slice(n.as_bytes());
            strtab.push(0);
        }
        let str_off = phoff + phentsize * phnum;
        let dyn_off = str_off + strtab.len() as u64;
        let mut dyn_entries: Vec<(u64, u64)> = offs.iter().map(|&o| (DT_NEEDED, o)).collect();
        dyn_entries.push((DT_STRTAB, VADDR + str_off));
        dyn_entries.push((DT_STRSZ, strtab.len() as u64));
        dyn_entries.push((DT_NULL, 0));
        let dyn_size = dyn_entries.len() as u64 * dynent;
        let total = dyn_off + dyn_size;

        let mut b = vec![0u8; total as usize];
        b[..4].copy_from_slice(b"\x7fELF");
        b[4] = if is64 { 2 } else { 1 };
        b[5] = 1;
        b[16..18].copy_from_slice(&e_type.to_le_bytes());
        let put16 = |b: &mut Vec<u8>, o: usize, v: u16| b[o..o + 2].copy_from_slice(&v.to_le_bytes());
        let put32 = |b: &mut Vec<u8>, o: usize, v: u32| b[o..o + 4].copy_from_slice(&v.to_le_bytes());
        let put64 = |b: &mut Vec<u8>, o: usize, v: u64| b[o..o + 8].copy_from_slice(&v.to_le_bytes());
        if is64 {
            put64(&mut b, 32, phoff);
            put16(&mut b, 54, phentsize as u16);
            put16(&mut b, 56, phnum as u16);
        } else {
            put32(&mut b, 28, phoff as u32);
            put16(&mut b, 42, phentsize as u16);
            put16(&mut b, 44, phnum as u16);
        }
        let ph = |b: &mut Vec<u8>, i: u64, p_type: u32, offset: u64, vaddr: u64, filesz: u64| {
            let o = (phoff + i * phentsize) as usize;
            put32(b, o, p_type);
            if is64 {
                put64(b, o + 8, offset);
                put64(b, o + 16, vaddr);
                put64(b, o + 32, filesz);
            } else {
                put32(b, o + 4, offset as u32);
                put32(b, o + 8, vaddr as u32);
                put32(b, o + 16, filesz as u32);
            }
        };
        ph(&mut b, 0, PT_LOAD, 0, VADDR, total);
        if with_dynamic {
            ph(&mut b, 1, PT_DYNAMIC, dyn_off, VADDR + dyn_off, dyn_size);
        }
        b[str_off as usize..dyn_off as usize].copy_from_slice(&strtab);
        for (i, (tag, val)) in dyn_entries.iter().enumerate() {
            let o = (dyn_off + i as u64 * dynent) as usize;
            if is64 {
                put64(&mut b, o, *tag);
                put64(&mut b, o + 8, *val);
            } else {
                put32(&mut b, o, *tag as u32);
                put32(&mut b, o + 4, *val as u32);
            }
        }
        b
    }

    #[test]
    fn reads_needed_from_a_64_bit_shared_object() {
        // The shape of the #16 culprit: libavformat.so.63 needing libbluray.so.3.
        let b = build(Class::Elf64, 3, &["libbluray.so.3", "libc.so.6"], true);
        let (class, names) = read_needed(&mut Cursor::new(b)).unwrap();
        assert_eq!(class, Class::Elf64);
        assert_eq!(names, vec!["libbluray.so.3", "libc.so.6"]);
    }

    #[test]
    fn reads_needed_from_a_32_bit_executable() {
        let b = build(Class::Elf32, 2, &["libEGL.so.1"], true);
        let (class, names) = read_needed(&mut Cursor::new(b)).unwrap();
        assert_eq!(class, Class::Elf32);
        assert_eq!(names, vec!["libEGL.so.1"]);
    }

    #[test]
    fn a_static_binary_has_no_dynamic_section_and_is_skipped() {
        let b = build(Class::Elf64, 2, &[], false);
        assert!(read_needed(&mut Cursor::new(b)).is_none());
    }

    #[test]
    fn object_files_and_non_elf_files_are_skipped() {
        let b = build(Class::Elf64, 1, &["libfoo.so.1"], true); // ET_REL
        assert!(read_needed(&mut Cursor::new(b)).is_none());
        assert!(read_needed(&mut Cursor::new(b"#!/bin/sh\necho hi\n".to_vec())).is_none());
        assert!(read_needed(&mut Cursor::new(Vec::new())).is_none());
    }

    #[test]
    fn a_truncated_file_is_skipped_not_a_panic() {
        let b = build(Class::Elf64, 3, &["libbluray.so.3"], true);
        for cut in [10, 60, 100, b.len() - 3] {
            let _ = read_needed(&mut Cursor::new(b[..cut].to_vec()));
        }
    }

    #[test]
    fn provide_to_needed_maps_provides_to_loader_filenames() {
        assert_eq!(
            provide_to_needed("libbluray.so=3-64"),
            Some(("libbluray.so.3".to_string(), Class::Elf64))
        );
        assert_eq!(
            provide_to_needed("libEGL.so=1-32"),
            Some(("libEGL.so.1".to_string(), Class::Elf32))
        );
        // No class suffix: cannot be tied to a word size, so not guessed at.
        assert_eq!(provide_to_needed("libfoo.so=3"), None);
        assert_eq!(provide_to_needed("glibc"), None);
    }
}
