use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io,
};

use gix_hash::{Kind, ObjectId, oid};
use gix_pack::data::{Entry, entry::Header};

use crate::{Object, invalid, not_found};

pub struct Repository {
    pack: gix_pack::data::File<Vec<u8>>,
    offsets: BTreeMap<ObjectId, u64>,
    pub(crate) refs: BTreeMap<Vec<u8>, ObjectId>,
}

impl Repository {
    pub(crate) fn from_pack(
        data: Vec<u8>,
        refs: BTreeMap<Vec<u8>, ObjectId>,
        limit: Option<usize>,
    ) -> io::Result<Self> {
        let pack = gix_pack::data::File::from_data(data, "memory-pack".into(), Kind::Sha1)
            .map_err(invalid)?
            .with_alloc_limit_bytes(limit);
        let data = pack
            .entry_slice(0..pack.pack_end() as u64)
            .ok_or_else(|| invalid("invalid pack length"))?;
        let mut hasher = gix_hash::hasher(Kind::Sha1);
        hasher.update(data);
        if hasher.try_finalize().map_err(invalid)? != pack.checksum() {
            return Err(invalid("Git pack checksum mismatch"));
        }
        let mut entries = Vec::new();
        let mut inflate = gix_zlib::Inflate::default();
        let mut offset = 12;
        for _ in 0..pack.num_objects() {
            let entry = pack.entry(offset).map_err(invalid)?;
            let size = usize::try_from(entry.decompressed_size).map_err(invalid)?;
            if limit.is_some_and(|limit| size > limit) {
                return Err(allocation_limit());
            }
            let mut scratch = Vec::new();
            scratch.try_reserve_exact(size).map_err(invalid)?;
            scratch.resize(size, 0);
            let consumed = pack
                .decompress_entry(&entry, &mut inflate, &mut scratch)
                .map_err(invalid)?;
            offset = entry
                .data_offset
                .checked_add(consumed as u64)
                .ok_or_else(|| invalid("pack entry offset overflow"))?;
            if offset > pack.pack_end() as u64 {
                return Err(invalid("pack entry extends beyond trailer"));
            }
            entries.push(entry);
        }
        if offset != pack.pack_end() as u64 {
            return Err(invalid("pack object count does not match its data"));
        }
        let known_offsets: BTreeSet<_> = entries.iter().map(Entry::pack_offset).collect();
        let mut waiting_offsets: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
        let mut waiting_ids: BTreeMap<ObjectId, Vec<usize>> = BTreeMap::new();
        let mut ready = VecDeque::new();
        for (index, entry) in entries.iter().enumerate() {
            match entry.header {
                Header::OfsDelta { base_distance } => {
                    let base = entry
                        .checked_base_pack_offset(base_distance)
                        .filter(|base| known_offsets.contains(base))
                        .ok_or_else(|| invalid("invalid offset-delta base"))?;
                    waiting_offsets.entry(base).or_default().push(index);
                }
                Header::RefDelta { base_id } => waiting_ids.entry(base_id).or_default().push(index),
                _ => ready.push_back(index),
            }
        }
        let mut repository = Self {
            pack,
            offsets: BTreeMap::new(),
            refs,
        };
        let mut resolved = 0;
        while let Some(index) = ready.pop_front() {
            let entry = &entries[index];
            let object = repository.decode(entry.clone(), &mut inflate)?;
            gix_object::ObjectRef::from_bytes(&object.data, object.kind, Kind::Sha1)
                .map_err(invalid)?;
            let oid =
                gix_object::compute_hash(Kind::Sha1, object.kind, &object.data).map_err(invalid)?;
            repository.offsets.insert(oid, entry.pack_offset());
            ready.extend(
                waiting_offsets
                    .remove(&entry.pack_offset())
                    .into_iter()
                    .flatten(),
            );
            ready.extend(waiting_ids.remove(&oid).into_iter().flatten());
            resolved += 1;
        }
        if resolved != entries.len() {
            return Err(invalid("unresolved reference-delta base or cyclic pack"));
        }
        Ok(repository)
    }

    pub fn object(&self, oid: &oid) -> io::Result<Object> {
        let offset = self
            .offsets
            .get(oid)
            .ok_or_else(|| not_found("Git object is not in the fetched pack"))?;
        let entry = self.pack.entry(*offset).map_err(invalid)?;
        self.decode(entry, &mut gix_zlib::Inflate::default())
    }

    fn decode(&self, entry: Entry, inflate: &mut gix_zlib::Inflate) -> io::Result<Object> {
        let mut data = Vec::new();
        let outcome = self
            .pack
            .decode_entry(
                entry,
                &mut data,
                inflate,
                &|oid, _| {
                    self.offsets
                        .get(oid)
                        .map(|offset| {
                            self.pack
                                .entry(*offset)
                                .map(gix_pack::data::decode::entry::ResolvedBase::InPack)
                        })
                        .transpose()
                },
                &mut gix_pack::cache::Never,
            )
            .map_err(invalid)?;
        Ok(Object {
            kind: outcome.kind,
            data,
        })
    }
}

pub(crate) fn allocation_limit() -> io::Error {
    io::Error::new(
        io::ErrorKind::OutOfMemory,
        "Git fetch allocation limit exceeded",
    )
}
