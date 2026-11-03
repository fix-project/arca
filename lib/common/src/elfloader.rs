use arca::Entry;
use arca::prelude::*;
use bytes::{Buf, buf::Take};
use elf::{
    endian::AnyEndian,
    file::{Class, ELF32_EHDR_TAILSIZE, ELF64_EHDR_TAILSIZE, FileHeader, parse_ident},
    parse::ParseAt,
    section::SectionHeader,
    segment::ProgramHeader,
};

extern crate alloc;
use alloc::vec::Vec;

const ZEROED_REGION_MIN_SIZE: usize = 1 << 21;

#[derive(derive_more::From, Debug)]
pub enum Error {
    Parse(elf::ParseError),
    Runtime,
    InvalidElf,
}

fn range<B: Buf + Clone>(source: &B, offset: u64, length: usize) -> Result<Take<B>, Error> {
    let offset = usize::try_from(offset).map_err(|_| Error::InvalidElf)?;
    let end = offset.checked_add(length).ok_or(Error::InvalidElf)?;
    if end > source.remaining() {
        return Err(Error::InvalidElf);
    }
    let mut data = source.clone();
    data.advance(offset);
    Ok(data.take(length))
}

/// Buffer clones must have independent cursors for ELF-relative offsets.
pub fn load_elf<R: arca::Runtime>(elf: impl Buf + Clone) -> Result<Function<R>, Error> {
    load_elf_from_pages(elf, |_| None)
}

/// Like [`load_elf`], but maps whole-page segment contents from `backing` instead of copying
/// them. `backing(offset)` returns the 4096-byte page holding the ELF's bytes at `offset`, if
/// one exists; writable segments map it copy-on-write.
pub fn load_elf_from_pages<R: arca::Runtime>(
    elf: impl Buf + Clone,
    backing: impl Fn(usize) -> Option<Page<R>>,
) -> Result<Function<R>, Error> {
    log::debug!("loading: {} byte ELF file", elf.remaining());
    let mut cursor = elf.clone();
    let mut ident = [0; elf::abi::EI_NIDENT];
    cursor
        .try_copy_to_slice(&mut ident)
        .map_err(|_| Error::InvalidElf)?;
    let ident = parse_ident::<AnyEndian>(&ident)?;
    let tail_size = match ident.1 {
        Class::ELF32 => ELF32_EHDR_TAILSIZE,
        Class::ELF64 => ELF64_EHDR_TAILSIZE,
    };
    let mut tail = [0; ELF64_EHDR_TAILSIZE];
    cursor
        .try_copy_to_slice(&mut tail[..tail_size])
        .map_err(|_| Error::InvalidElf)?;
    let header = FileHeader::parse_tail(ident, &tail[..tail_size])?;
    if header.e_type != elf::abi::ET_EXEC || header.e_phoff == 0 {
        return Err(Error::InvalidElf);
    }
    let start_address = header.e_entry;
    let mut count = header.e_phnum as usize;
    if count == elf::abi::PN_XNUM as usize {
        let size = SectionHeader::size_for(header.class);
        let mut bytes = [0; 64];
        range(&elf, header.e_shoff, size)?.copy_to_slice(&mut bytes[..size]);
        let section =
            SectionHeader::parse_at(header.endianness, header.class, &mut 0, &bytes[..size])?;
        count = section.sh_info as usize;
    }
    let entry_size = ProgramHeader::validate_entsize(header.class, header.e_phentsize as usize)?;
    let size = count.checked_mul(entry_size).ok_or(Error::InvalidElf)?;
    let mut headers = range(&elf, header.e_phoff, size)?;
    let mut segments = Vec::with_capacity(count);
    let mut bytes = [0; 56];
    for _ in 0..count {
        headers.copy_to_slice(&mut bytes[..entry_size]);
        segments.push(ProgramHeader::parse_at(
            header.endianness,
            header.class,
            &mut 0,
            &bytes[..entry_size],
        )?);
    }

    let mut table = R::create_table(0);

    for (i, segment) in segments.iter().enumerate() {
        match segment.p_type {
            elf::abi::PT_NOTE => {
                // ignore for now
            }
            elf::abi::PT_LOAD => {
                let start = segment.p_vaddr as usize;
                let page_start_memory = (start / 4096) * 4096;
                let offset = start - page_start_memory;
                let filesz = segment.p_filesz as usize;
                let memsz = segment.p_memsz as usize;

                let mut pages = (offset + memsz) / 4096;
                if !(offset + memsz).is_multiple_of(4096) {
                    pages += 1;
                }

                let mut memi = offset;
                let mut filei = 0;
                let mut data = range(&elf, segment.p_offset, filesz)?;
                let overlaps = segments[..i].iter().any(|previous| {
                    previous.p_type == elf::abi::PT_LOAD
                        && previous.p_memsz != 0
                        && (previous.p_vaddr as usize / 4096 * 4096)
                            < page_start_memory + pages * 4096
                        && ((previous.p_vaddr + previous.p_memsz) as usize).div_ceil(4096) * 4096
                            > page_start_memory
                });
                let zero_start = (offset + filesz).div_ceil(4096);
                let share_zeroes = !overlaps
                    && segment.p_flags & elf::abi::PF_W != 0
                    && pages.saturating_sub(zero_start) * 4096 >= ZEROED_REGION_MIN_SIZE;
                let mut page = 0;
                while page < pages {
                    let page_start = page * 4096;
                    if share_zeroes && page >= zero_start {
                        let address = page_start_memory + page_start;
                        let remaining = (pages - page) * 4096;
                        let span = [1usize << 30, 1 << 21, 4096]
                            .into_iter()
                            .find(|span| address.is_multiple_of(*span) && *span <= remaining)
                            .unwrap();
                        let entry = R::create_zeroed_entry(span);
                        table.map(address, entry).map_err(|_| Error::Runtime)?;
                        page += span / 4096;
                        memi = page * 4096;
                        continue;
                    }
                    let writable = segment.p_flags & elf::abi::PF_W != 0;
                    if !overlaps
                        && memi == page_start
                        && filesz - filei >= 4096
                        && let Some(shared) = backing(segment.p_offset as usize + filei)
                    {
                        let entry = if writable {
                            Entry::RWPage(shared)
                        } else {
                            Entry::ROPage(shared)
                        };
                        table
                            .map(page_start_memory + page_start, entry)
                            .map_err(|_| Error::Runtime)?;
                        data.advance(4096);
                        filei += 4096;
                        memi = page_start + 4096;
                        page += 1;
                        continue;
                    }
                    let unique_page =
                        table
                            .unmap(page_start_memory + page_start)
                            .and_then(|entry| match entry {
                                Entry::ROPage(page) | Entry::RWPage(page) => Some(page),
                                _ => None,
                            });
                    let mut unique_page = if let Some(up) = unique_page {
                        up
                    } else {
                        R::create_page(1 << 12)
                    };
                    assert!(memi >= page_start);
                    let page_end = page_start + 4096;
                    if memi >= page_start && memi < page_end {
                        if filei < filesz {
                            let mem_left = page_end - memi;
                            let file_left = filesz - filei;

                            let copy_size = core::cmp::min(mem_left, file_left);
                            let mut copied = 0;
                            while copied < copy_size {
                                let chunk = data.chunk();
                                let size = chunk.len().min(copy_size - copied);
                                assert_eq!(
                                    unique_page.write(memi - page_start + copied, &chunk[..size]),
                                    size
                                );
                                data.advance(size);
                                copied += size;
                            }
                            memi = page_end;
                            filei += copy_size;
                        } else {
                            memi = page_end;
                        }
                    }

                    if segment.p_flags & elf::abi::PF_W != 0 {
                        table
                            .map(page_start_memory + page_start, Entry::RWPage(unique_page))
                            .map_err(|_| Error::Runtime)?;
                    } else {
                        table
                            .map(page_start_memory + page_start, Entry::ROPage(unique_page))
                            .map_err(|_| Error::Runtime)?;
                    }
                    page += 1;
                }
            }
            elf::abi::PT_TLS => {}
            elf::abi::PT_PHDR => {
                // program header
            }
            0x60000000..0x70000000 => {
                // os-specific
            }
            0x70000000..0x80000000 => {
                // arch-specific
            }
            x => unimplemented!("{i} - segment type {x:#x}"),
        }
    }

    let mut registers = R::create_tuple(20);
    registers.set(16, Word::from(start_address));

    let descriptors = R::create_tuple(0);

    let mut data = R::create_tuple(3);
    data.set(0, Value::Tuple(registers));
    data.set(1, Value::Table(table));
    data.set(2, Value::Tuple(descriptors));

    let args = R::create_tuple(0);
    R::create_function(Tuple::from(("Arcane", data, args)).into()).map_err(|_| Error::Runtime)
}
