#![no_std]

use fixruntime::arca::{EffectAction, FixOnArca, trust::Trust};
use fixruntime::storage::{Storage, memory::MemoryStorage};
use fixruntime::{Blob as FixBlob, Handle, Tree as FixTree};
use kernel::prelude::*;

pub fn number(value: u64) -> Handle {
    FixBlob::literal(&value.to_le_bytes()).unwrap().into()
}

pub fn fix_program(memory: &MemoryStorage, author: Handle, elf: &[u8]) -> FixTree {
    memory
        .add_tree(&[
            author,
            memory.add_blob(elf).into(),
            FixBlob::literal(b"Runnable").unwrap().into(),
        ])
        .tag_descriptor()
}

pub struct FixState {
    pub memory: Arc<MemoryStorage>,
    pub runtime: FixOnArca<Arc<MemoryStorage>>,
    pub combination: FixTree,
    pub program: FixTree,
    pub request: Handle,
}

impl FixState {
    pub fn new(elf: &[u8], first: u64, second: u64) -> Self {
        let memory = Arc::new(MemoryStorage::default());
        let compiler = memory
            .add_blob(fixruntime::stdlib::PRIMITIVES[0].1)
            .object()
            .into_ref();
        let canonical = memory.canonicalize(compiler.into()).unwrap();
        let authority: Handle = canonical.into();
        let trust = Trust::parse(&format!("{authority}\n")).unwrap();
        let program = fix_program(&memory, compiler.into(), elf);
        let request: Handle = memory
            .add_tree(&[FixBlob::literal(b"bench").unwrap().into()])
            .into();
        let combination = memory.add_tree(&[
            number(0),
            program.into(),
            number(first),
            number(second),
            request,
        ]);
        let runtime = FixOnArca::new(memory.clone()).with_trust(trust);
        Self {
            memory,
            runtime,
            combination,
            program,
            request,
        }
    }

    pub fn run(&self, combination: FixTree) -> Handle {
        self.runtime
            .execute_with_effects(combination, false, &mut |_| EffectAction::Unhandled)
            .unwrap()
    }

    pub fn pair(&self, value: Handle) -> (Handle, Handle) {
        let fields = self
            .memory
            .get_tree(FixTree::try_from(value).unwrap())
            .unwrap();
        assert_eq!(fields.len(), 2);
        (fields[0], fields[1])
    }

    pub fn function_effect(&self, handler: Handle, pending: Handle) -> (Handle, Handle) {
        let (request, continuation) = self.pair(pending);
        let combination = self.memory.add_tree(&[number(0), handler, request]);
        let (reply, handler) = self.pair(self.run(combination));
        let combination = self.memory.add_tree(&[number(0), continuation, reply]);
        (self.run(combination), handler)
    }

    pub fn capture(&self) -> Handle {
        self.continuation(self.run(self.combination))
    }

    pub fn continuation(&self, pair: Handle) -> Handle {
        let fields = self
            .memory
            .get_tree(FixTree::try_from(pair).unwrap())
            .unwrap();
        assert_eq!(fields.len(), 2);
        assert!(self.memory.equals(fields[0], self.request).unwrap());
        fields[1]
    }

    pub fn resume(&self, continuation: Handle) -> Handle {
        let combination = self.memory.add_tree(&[number(0), continuation, number(0)]);
        self.continuation(self.run(combination))
    }

    pub fn check_sum(&self, result: Handle, expected: u64) {
        let result = self
            .memory
            .get_blob(FixBlob::try_from(result).unwrap())
            .unwrap();
        assert_eq!(result.as_ref(), expected.to_le_bytes());
    }
}
