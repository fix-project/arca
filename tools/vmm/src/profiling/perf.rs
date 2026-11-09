use elf::{
    endian::AnyEndian,
    file::{FileHeader, parse_ident},
    parse::ParseAt,
    segment::ProgramHeader,
};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    os::unix::fs::FileExt,
    path::{Path, PathBuf},
};

type Segment = (u64, u64, u64, u64, u32);
struct Image {
    path: PathBuf,
    file: File,
    segments: Vec<Segment>,
}
impl Image {
    fn load(path: PathBuf) -> io::Result<Option<Self>> {
        let mut source = File::open(&path)?;
        let mut header = [0; 6];
        match source.read_exact(&mut header) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(error) => return Err(error),
        }
        if header != *b"\x7fELF\x02\x01" {
            return Ok(None);
        }
        source.rewind()?;
        let mut bytes = [0; 64];
        source.read_exact(&mut bytes)?;
        let ident = parse_ident::<AnyEndian>(&bytes[..16]).map_err(io::Error::other)?;
        let header = FileHeader::parse_tail(ident, &bytes[16..]).map_err(io::Error::other)?;
        if header.e_entry == 0 {
            return Ok(None);
        }
        let entry_size = ProgramHeader::validate_entsize(header.class, header.e_phentsize as usize)
            .map_err(io::Error::other)?;
        let mut table_entry = vec![0; entry_size];
        let mut segments = Vec::new();
        for index in 0..header.e_phnum {
            source.read_exact_at(
                &mut table_entry,
                header.e_phoff + u64::from(index) * entry_size as u64,
            )?;
            let segment =
                ProgramHeader::parse_at(header.endianness, header.class, &mut 0, &table_entry)
                    .map_err(io::Error::other)?;
            if segment.p_type == elf::abi::PT_LOAD {
                segments.push((
                    segment.p_vaddr,
                    segment.p_memsz,
                    segment.p_offset,
                    segment.p_filesz,
                    segment.p_flags,
                ));
            }
        }
        Ok(Some(Self {
            path,
            file: source,
            segments,
        }))
    }
    fn executable(&self, address: u64) -> bool {
        self.segments.iter().any(|&(start, _, _, size, flags)| {
            flags & 1 != 0 && address >= start && address - start < size
        })
    }

    fn code(&self, address: u64) -> Option<[u8; 32]> {
        for &(start, _, offset, size, flags) in &self.segments {
            if flags & 1 != 0 && address >= start && address.checked_add(32)? <= start + size {
                let mut code = [0; 32];
                self.file
                    .read_exact_at(&mut code, offset + address - start)
                    .ok()?;
                return Some(code);
            }
        }
        None
    }
}
struct Sample {
    time: u64,
    core: u32,
    image: Option<usize>,
    frames: Vec<u64>,
    weight: u64,
}
fn number(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
fn record(file: &mut impl Write, kind: u32, bytes: &[u8]) -> io::Result<()> {
    let size = (8 + bytes.len() + 7) & !7;
    file.write_all(&kind.to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?;
    file.write_all(&(size as u16).to_le_bytes())?;
    file.write_all(bytes)?;
    file.write_all(&[0; 7][..size - 8 - bytes.len()])
}
fn words(values: &[u64]) -> Vec<u8> {
    values.iter().flat_map(|x| x.to_le_bytes()).collect()
}
fn paths(directory: &Path, output: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            paths(&path, output)?;
        } else {
            output.push(path);
        }
    }
    Ok(())
}

pub(super) fn export(directory: &Path, repository: &Path, output: &Path) -> io::Result<()> {
    let mut files = Vec::new();
    paths(directory, &mut files)?;
    let mut images = Vec::new();
    let kernel_path = directory.join("kernel.elf");
    if kernel_path.exists()
        && let Some(kernel) = Image::load(kernel_path)?
    {
        images.push(kernel);
    }
    let image_directory = directory.join("images");
    fs::create_dir_all(&image_directory)?;
    let mut objects = Vec::new();
    let object_directory = repository.join(".fix/objects");
    if object_directory.exists() {
        paths(&object_directory, &mut objects)?;
    }
    let mut seen = HashSet::new();
    for path in objects {
        if let Ok(canonical) = path.canonicalize()
            && seen.insert(canonical)
            && let Ok(Some(image)) = Image::load(path)
        {
            images.push(image);
        }
    }
    let mut samples = Vec::new();
    let mut identities = HashMap::new();
    let mut candidates = HashSet::new();
    let mut cores = Vec::new();
    for path in files
        .iter()
        .filter(|p| p.to_string_lossy().ends_with("-stacks.bin"))
    {
        let core: u32 = path
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.split('-').nth(1))
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::other("invalid profile core"))?;
        if !cores.contains(&core) {
            cores.push(core);
        }
        let mut source = File::open(path)?;
        let mut magic = [0; 8];
        source.read_exact(&mut magic)?;
        if &magic != b"ARCASMP2" {
            return Err(io::Error::other("unsupported stack snapshot version"));
        }
        let mut captured = HashMap::new();
        let mut bytes = [0u8; 4696];
        loop {
            let n = source.read(&mut bytes[..1])?;
            if n == 0 {
                break;
            }
            source.read_exact(&mut bytes[1..])?;
            let time = number(&bytes, 0);
            let mode = number(&bytes, 16);
            let ip = number(&bytes, 24);
            let depth = usize::try_from(number(&bytes, 48)).map_err(io::Error::other)?;
            if depth > 48 {
                return Err(io::Error::other("invalid snapshot depth"));
            }
            let code: [u8; 32] = bytes[184..216].try_into().unwrap();
            let image =
                if mode == 0 && images.first().and_then(|image| image.code(ip)) == Some(code) {
                    Some(0)
                } else {
                    *identities.entry((ip, code)).or_insert_with(|| {
                        let matches: Vec<_> = images
                            .iter()
                            .enumerate()
                            .skip(1)
                            .filter(|(_, image)| image.code(ip) == Some(code))
                            .map(|(index, _)| index)
                            .collect();
                        if matches.len() == 1 {
                            matches.first().copied()
                        } else {
                            candidates.extend(matches);
                            None
                        }
                    })
                };
            let frames = (0..depth)
                .map(|i| number(&bytes, 4312 + i * 8))
                .enumerate()
                .take_while(|&(index, frame)| {
                    index == 0
                        || image
                            .is_some_and(|image| images[image].executable(frame.saturating_sub(1)))
                })
                .map(|(_, frame)| frame)
                .collect();
            captured.insert(time, (image, frames));
        }
        let pulse_path = path.with_file_name(format!("core-{core}-pulses.bin"));
        let pulses = fs::read(pulse_path)?;
        let (pulses, remainder) = pulses.as_chunks::<9>();
        if !remainder.is_empty() {
            return Err(io::Error::other("truncated profile pulses"));
        }
        let rows: Vec<_> = pulses.iter().map(|b| (number(b, 0), b[8])).collect();
        for (index, &(time, phase)) in rows.iter().enumerate() {
            let weight = rows
                .get(index + 1)
                .map(|r| r.0 - time)
                .unwrap_or(10_000_000);
            let (image, frames) = if phase == 1 {
                captured.remove(&time).unwrap_or((None, vec![0]))
            } else {
                (None, vec![0])
            };
            samples.push(Sample {
                time,
                core,
                image,
                frames,
                weight,
            });
        }
    }
    let mut used: HashSet<_> = samples.iter().filter_map(|sample| sample.image).collect();
    used.extend(candidates);
    let mut remap = HashMap::new();
    let mut selected = Vec::new();
    for (index, mut image) in images.into_iter().enumerate() {
        if !used.contains(&index) {
            continue;
        }
        if image.path != directory.join("kernel.elf") {
            let destination = image_directory.join(format!("image-{index}.elf"));
            fs::copy(&image.path, &destination)?;
            image.path = destination;
        }
        remap.insert(index, selected.len());
        selected.push(image);
    }
    let images = selected;
    for sample in &mut samples {
        sample.image = sample.image.map(|index| remap[&index]);
    }
    samples.sort_by_key(|s| s.time);
    cores.sort_unstable();
    let mut file = BufWriter::with_capacity(
        1 << 20,
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?,
    );
    file.write_all(&[0; 248])?;
    for (index, image) in images.iter().enumerate() {
        let pid = 1000 + index as u32;
        for core in &cores {
            let tid = 10000 + index as u32 * 1024 + core;
            let mut data = [pid.to_le_bytes(), tid.to_le_bytes()].concat();
            data.extend_from_slice(format!("vCPU {core}\0").as_bytes());
            record(&mut file, 3, &data)?;
        }
        for &(start, size, offset, _, flags) in &image.segments {
            if flags & 1 == 0 {
                continue;
            }
            let mut data = [pid.to_le_bytes(), pid.to_le_bytes()].concat();
            data.extend(words(&[
                start & !4095,
                (start % 4096 + size).div_ceil(4096) * 4096,
                offset & !4095,
            ]));
            data.extend_from_slice(image.path.as_os_str().as_encoded_bytes());
            data.push(0);
            record(&mut file, 1, &data)?;
        }
    }
    let unknown = images.len() as u32;
    for core in &cores {
        let pid = 1000 + unknown;
        let tid = 10000 + unknown * 1024 + core;
        let mut data = [pid.to_le_bytes(), tid.to_le_bytes()].concat();
        data.extend_from_slice(format!("vCPU {core}\0").as_bytes());
        record(&mut file, 3, &data)?;
    }
    let mut weight = 0;
    for sample in &samples {
        let image = sample.image.unwrap_or(images.len());
        let pid = 1000 + image as u32;
        let tid = 10000 + image as u32 * 1024 + sample.core;
        let frames = if sample.frames.is_empty() {
            &[0][..]
        } else {
            &sample.frames
        };
        let mut data = words(&[frames[0]]);
        data.extend([pid.to_le_bytes(), tid.to_le_bytes()].concat());
        data.extend(words(&[sample.time]));
        data.extend([sample.core.to_le_bytes(), 0u32.to_le_bytes()].concat());
        data.extend(words(&[sample.weight, frames.len() as u64]));
        data.extend(words(frames));
        record(&mut file, 9, &data)?;
        weight += sample.weight;
    }
    let data_size = file.stream_position()? - 248;
    let mut attr = [0u8; 128];
    attr[..4].copy_from_slice(&1u32.to_le_bytes());
    attr[4..8].copy_from_slice(&128u32.to_le_bytes());
    attr[8..16].copy_from_slice(&0u64.to_le_bytes());
    attr[16..24].copy_from_slice(&10_000_000u64.to_le_bytes());
    attr[24..32].copy_from_slice(&423u64.to_le_bytes());
    let mut name = b"arca-vcpu-wall-ns\0".to_vec();
    name.resize(64, 0);
    let mut feature = [1u32.to_le_bytes(), 128u32.to_le_bytes()].concat();
    feature.extend(attr);
    feature.extend([0u32.to_le_bytes(), 64u32.to_le_bytes()].concat());
    feature.extend(name);
    file.write_all(&words(&[248 + data_size + 16, feature.len() as u64]))?;
    file.write_all(&feature)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(b"PERFILE2")?;
    file.write_all(&words(&[
        104,
        144,
        104,
        144,
        248,
        data_size,
        0,
        0,
        1 << 12,
        0,
        0,
        0,
    ]))?;
    file.write_all(&attr)?;
    file.write_all(&[0; 16])?;
    file.flush()?;
    fs::write(
        directory.join("summary.json"),
        format!(
            "{{\"event\":\"arca-vcpu-wall-ns\",\"samples\":{},\"weight_ns\":{weight}}}\n",
            samples.len()
        ),
    )?;
    Ok(())
}
