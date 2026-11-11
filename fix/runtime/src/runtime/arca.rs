extern crate alloc;

use crate::handle::*;
use crate::runtime::{InvalidApplication, Runtime};
use crate::storage::memory::MemoryStorage;
use crate::storage::repository::RepositoryStorage;
use crate::storage::{ImmutableBytes, Storage};
use alloc::sync::Arc;
use fix::Memoization;
mod checkpoint;
mod continuation;
mod handles;
mod images;
mod locals;
pub mod trust;
use continuation::Store;
use core::sync::atomic::{AtomicUsize, Ordering};
use kernel::kthread::KMutex;
use kernel::prelude::{Blob as ArcaBlob, Function, Tuple, Value, Vec};

pub enum EffectAction {
    Resume(Handle),
    Stop(Handle),
    Unhandled,
}

/// What a combination applies.
enum Callable {
    Continuation(Function, usize, Tree),
    Runnable(Blob, usize),
}

pub struct Advice {
    pub target: Handle,
    pub key: Vec<u8>,
    pub payload: Vec<u8>,
}

#[expect(
    clippy::large_enum_variant,
    reason = "Keep advice delivery allocation-free."
)]
pub enum Execution {
    Complete(Handle),
    Advice { event: Advice, suspended: Suspended },
}

pub struct Suspended {
    invocation: Invocation,
    application: Option<Tree>,
}

struct Invocation {
    runtime_id: usize,
    function: Function,
    combination: Tree,
    privileged: bool,
    pending: Option<(ArcaBlob, usize, Handle)>,
    profiles: Vec<([u8; 32], usize)>,
}

impl Execution {
    fn completed(self) -> Handle {
        match self {
            Self::Complete(value) => value,
            Self::Advice { .. } => unreachable!(),
        }
    }
}

pub enum CoordinationAction {
    Resume(u64, Handle),
    Stop(Handle),
}

#[derive(Debug)]
pub struct FixOnArca<S = MemoryStorage> {
    storage: Store<S>,
    memoization: Option<Arc<RepositoryStorage>>,
    pub(crate) memoize: Memoization,
}

const WASM_TRAP_UNREACHABLE: u32 = 5;

type Coordinator<'a> = dyn FnMut(&[u8], &[Handle]) -> CoordinationAction + 'a;

type ProfileKey = ([u8; 32], usize);
type ProfileTiming = (u64, u64);

type ClaimKey = [[u8; HANDLE_SIZE]; 2];
type ClaimEntry = Arc<KMutex<Option<Vec<Tree>>>>;

/// A scheduler run's view of repository claims. In-memory claims remain live.
#[derive(Default)]
pub struct SavedClaims {
    entries: KMutex<hashbrown::HashMap<ClaimKey, ClaimEntry>>,
    hits: AtomicUsize,
    misses: AtomicUsize,
    profile: bool,
}

impl SavedClaims {
    pub fn with_profile(profile: bool) -> Self {
        Self {
            profile,
            ..Self::default()
        }
    }

    fn get(
        &self,
        repository: &crate::repository::Repository,
        prefix: &[Handle; 2],
    ) -> Result<Vec<Tree>, &'static str> {
        let key = prefix.map(|value| *value.as_bytes());
        let entry = self.entries.lock().entry(key).or_default().clone();
        let mut entry = entry.lock();
        if let Some(claims) = &*entry {
            if self.profile {
                self.hits.fetch_add(1, Ordering::Relaxed);
            }
            return Ok(claims.clone());
        }
        if self.profile {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }
        let claims = repository
            .claims(prefix)
            .map_err(|_| "could not read saved claims")?;
        *entry = Some(claims.clone());
        Ok(claims)
    }

    pub fn counts(&self) -> (usize, usize) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
        )
    }
}

impl Default for FixOnArca {
    fn default() -> Self {
        Self::new(MemoryStorage::default())
    }
}

impl<S: Storage + AsRef<MemoryStorage>> Runtime for FixOnArca<S> {
    fn storage(&self) -> &dyn Storage {
        &self.storage
    }

    fn execute(&self, combination: Tree) -> Result<Tree, InvalidApplication> {
        assert!(
            combination.as_handle().is_value(),
            "application requires an evaluated combination"
        );
        if self.memoization.is_some() {
            let apply = self.storage.add_blob(&2u64.to_le_bytes());
            for coupon in self
                .cached_claims(&[apply.into(), combination.into()])
                .unwrap()
            {
                let fields = self.storage.get_tree(coupon).unwrap();
                if fields.len() == 2 && trust::is_kernel_author(fields[0]) {
                    return Ok(coupon);
                }
            }
        }
        let result = self.storage.execute(combination, false)?;
        let coupon = self.storage.certificate(2, combination.into(), result);
        if self.memoization.is_some() {
            self.index_claim(coupon.into()).unwrap();
        }
        Ok(coupon)
    }
}

impl<S: Storage + AsRef<MemoryStorage>> FixOnArca<S> {
    pub(crate) fn enable_profile(&self) {
        self.storage.profile.store(true, Ordering::Relaxed);
    }

    pub(crate) fn application_profiles(&self) -> Vec<(ProfileKey, ProfileTiming)> {
        self.storage
            .applications
            .lock()
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect()
    }

    pub(crate) fn capture_count(&self) -> usize {
        self.storage.capture_count()
    }

    pub(crate) fn elf_loads(&self) -> (usize, u64) {
        self.storage.elf_loads()
    }

    #[doc(hidden)]
    pub fn kernel_calls(&self) -> (usize, usize, usize, usize) {
        self.storage.kernel_calls()
    }

    pub(crate) fn call_times(&self) -> [u64; 5] {
        self.storage.call_times()
    }

    pub(crate) fn other_calls(&self) -> [usize; 5] {
        self.storage.other_calls()
    }

    pub(crate) fn index_claim(&self, value: Handle) -> Result<(), &'static str> {
        self.storage
            .memory()
            .index_claim(&self.storage, value)
            .map_err(|_| "could not index claim")
    }

    pub(crate) fn persists_coupon(&self, recommendation: u64) -> bool {
        match self.memoize {
            Memoization::Always => true,
            Memoization::Never => false,
            Memoization::Auto => recommendation != 0,
        }
    }

    pub(crate) fn cached_claims(&self, prefix: &[Handle]) -> Result<Vec<Tree>, &'static str> {
        self.claims(prefix, None)
    }

    pub fn cached_claims_in(
        &self,
        prefix: &[Handle],
        saved: &SavedClaims,
    ) -> Result<Vec<Tree>, &'static str> {
        self.claims(prefix, Some(saved))
    }

    fn claims(
        &self,
        prefix: &[Handle],
        saved: Option<&SavedClaims>,
    ) -> Result<Vec<Tree>, &'static str> {
        let [statement, input] = prefix else {
            return Err("expected statement and input");
        };
        let mut coupons = self.storage.memory().claims([*statement, *input]);
        if let Some(cache) = &self.memoization {
            if !statement.is_canonical() || !input.is_canonical() {
                return Ok(coupons);
            }
            let prefix = [*statement, *input];
            let stored = match saved {
                Some(saved) => saved.get(cache.repository(), &prefix)?,
                None => cache
                    .repository()
                    .claims(&prefix)
                    .map_err(|_| "could not read saved claims")?,
            };
            for coupon in stored {
                if !coupons
                    .iter()
                    .any(|saved| saved.as_handle().same_encoding(coupon.as_handle()))
                {
                    coupons.push(coupon);
                }
            }
        }
        Ok(coupons)
    }

    /// `storage.as_ref()` must expose the memory store backing `storage`.
    pub fn new(storage: S) -> Self {
        Self {
            storage: Store::new(storage),
            memoization: None,
            memoize: Memoization::Auto,
        }
    }

    pub fn with_trust(mut self, trust: trust::Trust) -> Self {
        self.storage.trust = trust;
        self
    }

    /// Runs an application or privileged callable, yielding each advice event to the caller.
    /// Applications complete with an Apply coupon; privileged calls return their raw result.
    /// Resuming never supplies scheduler data to the application.
    pub fn execute_observed(
        &self,
        combination: Tree,
        privileged: bool,
    ) -> Result<Execution, InvalidApplication> {
        if !privileged {
            assert!(
                combination.as_handle().is_value(),
                "application requires an evaluated combination"
            );
            if self.memoization.is_some() {
                let apply = self.storage.add_blob(&2u64.to_le_bytes());
                for coupon in self
                    .cached_claims(&[apply.into(), combination.into()])
                    .unwrap()
                {
                    let fields = self.storage.get_tree(coupon).unwrap();
                    if fields.len() == 2 && trust::is_kernel_author(fields[0]) {
                        return Ok(Execution::Complete(coupon.into()));
                    }
                }
            }
        }
        let execution = self.storage.invoke(
            combination,
            privileged,
            &mut |_| EffectAction::Unhandled,
            &mut None,
            true,
        )?;
        self.complete_observed(execution, (!privileged).then_some(combination))
    }

    /// Resumes an invocation belonging to this runtime with its fixed acknowledgement.
    pub fn resume_observed(&self, suspended: Suspended) -> Result<Execution, InvalidApplication> {
        let execution = self.storage.resume(
            suspended.invocation,
            &mut |_| EffectAction::Unhandled,
            &mut None,
            true,
        )?;
        self.complete_observed(execution, suspended.application)
    }

    fn complete_observed(
        &self,
        execution: Execution,
        application: Option<Tree>,
    ) -> Result<Execution, InvalidApplication> {
        Ok(match execution {
            Execution::Complete(value) => {
                let value = if let Some(combination) = application {
                    let coupon = self.storage.certificate(2, combination.into(), value);
                    if self.memoization.is_some() {
                        self.index_claim(coupon.into()).unwrap();
                    }
                    coupon.into()
                } else {
                    value
                };
                Execution::Complete(value)
            }
            Execution::Advice {
                event,
                mut suspended,
            } => {
                suspended.application = application;
                Execution::Advice { event, suspended }
            }
        })
    }

    /// Runs outside deterministic Fix semantics and must not receive an application coupon.
    pub fn execute_privileged(&self, combination: Tree) -> Result<Handle, InvalidApplication> {
        self.storage.execute(combination, true)
    }
    /// Kernel-handled execution must not receive an application coupon.
    pub fn execute_with_effects(
        &self,
        combination: Tree,
        privileged: bool,
        handler: &mut dyn FnMut(Handle) -> EffectAction,
    ) -> Result<Handle, InvalidApplication> {
        self.storage
            .execute_with_effects(combination, privileged, handler)
    }

    pub fn execute_with_coordination(
        &self,
        combination: Tree,
        handler: &mut dyn FnMut(Handle) -> EffectAction,
        coordinator: &mut dyn FnMut(&[u8], &[Handle]) -> CoordinationAction,
    ) -> Result<Handle, InvalidApplication> {
        self.storage
            .execute_with_coordination(combination, handler, coordinator)
    }
}

impl FixOnArca<Arc<RepositoryStorage>> {
    pub fn memoized(storage: Arc<RepositoryStorage>, mode: Memoization) -> Self {
        let mut runtime = Self::new(storage.clone());
        if mode != Memoization::Never {
            runtime.memoization = Some(storage);
        }
        runtime.memoize = mode;
        runtime
    }
}

impl<S: Storage + AsRef<MemoryStorage>> Store<S> {
    fn trap(&self, code: u32) -> Execution {
        let marker = self.add_blob(b"Wasm.trap");
        let code = self.add_blob(&code.to_le_bytes());
        Execution::Complete(self.add_tree(&[marker.into(), code.into()]).into())
    }

    fn execute(&self, combination: Tree, privileged: bool) -> Result<Handle, InvalidApplication> {
        self.execute_with_effects(combination, privileged, &mut |_| EffectAction::Unhandled)
    }

    fn execute_with_effects(
        &self,
        combination: Tree,
        privileged: bool,
        handler: &mut dyn FnMut(Handle) -> EffectAction,
    ) -> Result<Handle, InvalidApplication> {
        self.invoke(combination, privileged, handler, &mut None, false)
            .map(Execution::completed)
    }

    fn execute_with_coordination(
        &self,
        combination: Tree,
        handler: &mut dyn FnMut(Handle) -> EffectAction,
        coordinator: &mut dyn FnMut(&[u8], &[Handle]) -> CoordinationAction,
    ) -> Result<Handle, InvalidApplication> {
        self.invoke(combination, true, handler, &mut Some(coordinator), false)
            .map(Execution::completed)
    }

    fn invoke(
        &self,
        combination: Tree,
        privileged: bool,
        handler: &mut dyn FnMut(Handle) -> EffectAction,
        coordinator: &mut Option<&mut Coordinator<'_>>,
        observing: bool,
    ) -> Result<Execution, InvalidApplication> {
        let contents = self.get_tree(combination).unwrap();
        let (function, combination, profiles) = match self.callable(&contents)? {
            Callable::Continuation(mut function, address, author) => {
                let reply = handles::insert(&mut function, address, contents[2]);
                let profiles = if !privileged && self.profile.load(Ordering::Relaxed) {
                    self.application_profile(author).into_iter().collect()
                } else {
                    Vec::new()
                };
                (function.apply(pack_reference(reply)), author, profiles)
            }
            Callable::Runnable(executable, arguments) => {
                let mut function = self.image(executable)?;
                let root = handles::insert(&mut function, handles::HANDLE_BASE, combination.into());
                let profiles = if !privileged && self.profile.load(Ordering::Relaxed) {
                    alloc::vec![(*executable.as_handle().as_bytes(), arguments)]
                } else {
                    Vec::new()
                };
                (function.apply(pack_reference(root)), combination, profiles)
            }
        };
        self.resume(
            Invocation {
                runtime_id: self.image_cache_id,
                function,
                combination,
                privileged,
                pending: None,
                profiles,
            },
            handler,
            coordinator,
            observing,
        )
    }

    fn resume(
        &self,
        invocation: Invocation,
        handler: &mut dyn FnMut(Handle) -> EffectAction,
        coordinator: &mut Option<&mut Coordinator<'_>>,
        observing: bool,
    ) -> Result<Execution, InvalidApplication> {
        if invocation.runtime_id != self.image_cache_id {
            return Err(InvalidApplication);
        }
        let profiles = invocation.profiles.clone();
        let started = (!profiles.is_empty()).then(kernel::tsc::read);
        let result = self.run(invocation, handler, coordinator, observing);
        if let Some(started) = started {
            let elapsed = kernel::tsc::read().saturating_sub(started).as_nanos() as u64;
            let completed = !matches!(&result, Ok(Execution::Advice { .. }));
            let mut applications = self.applications.lock();
            for key in profiles {
                let entry = applications.entry(key).or_default();
                entry.0 += u64::from(completed);
                entry.1 += elapsed;
            }
        }
        result
    }

    fn application_profile(&self, combination: Tree) -> Option<([u8; 32], usize)> {
        let fields = self.get_tree(combination).ok()?;
        let mut procedure = *fields.get(1)?;
        let mut arguments = fields.len().saturating_sub(2);
        loop {
            let tree = Tree::try_from(procedure).ok()?;
            let fields = self.get_tree(tree).ok()?;
            procedure = *fields.get(1)?;
            if Blob::try_from(procedure).is_ok() {
                let executable = self.trust.runnable(self, tree).ok()?;
                return Some((*executable.as_handle().as_bytes(), arguments));
            }
            arguments += fields.len().saturating_sub(2);
        }
    }

    /// What `contents`, a combination, applies: a continuation taking one reply, or a
    /// trusted Runnable found by following the first element of each Tree.
    fn callable(&self, contents: &[Handle]) -> Result<Callable, InvalidApplication> {
        let mut procedure = *contents.get(1).ok_or(InvalidApplication)?;
        let mut arguments = contents.len().saturating_sub(2);
        loop {
            let tree = Tree::try_from(procedure).map_err(|_| InvalidApplication)?;
            if let Some((function, address, author)) = self.continuation(tree) {
                if contents.len() != 3 {
                    return Err(InvalidApplication);
                }
                return Ok(Callable::Continuation(function, address, author));
            }
            let fields = self.get_tree(tree).unwrap();
            procedure = *fields.get(1).ok_or(InvalidApplication)?;
            if Blob::try_from(procedure).is_ok() {
                return match self.trust.runnable(self, tree) {
                    Ok(executable) => Ok(Callable::Runnable(executable, arguments)),
                    Err(trust::TrustError::Storage(error)) => panic!("{error:?}"),
                    Err(_) => Err(InvalidApplication),
                };
            }
            arguments += fields.len().saturating_sub(2);
        }
    }

    fn run(
        &self,
        invocation: Invocation,
        handler: &mut dyn FnMut(Handle) -> EffectAction,
        coordinator: &mut Option<&mut Coordinator<'_>>,
        observing: bool,
    ) -> Result<Execution, InvalidApplication> {
        let Invocation {
            runtime_id,
            function: mut f,
            combination,
            privileged,
            mut pending,
            profiles,
        } = invocation;
        let profile = self.profile.load(Ordering::Relaxed);
        loop {
            let started = profile.then(kernel::tsc::read);
            let value = f.force();
            self.record_force(started);
            let Value::Function(g) = value else {
                return Err(InvalidApplication);
            };
            let (effect, mut args) = g.into_inner().into_symbolic().ok_or(InvalidApplication)?;
            let effect: ArcaBlob = effect.try_into().map_err(|_| InvalidApplication)?;
            if profile {
                self.record_kernel_call(&effect);
            }
            let Some(Value::Function(mut k)) = args.pop_back() else {
                return Err(InvalidApplication);
            };

            let result_address = if matches!(
                &*effect,
                b"create_tag"
                    | b"create_tag_unchecked"
                    | b"get_tree_element"
                    | b"unsound_handle_bytes"
            ) {
                let Some(Value::Word(address)) = args.pop_back() else {
                    panic!("missing result cell")
                };
                address.read() as usize
            } else {
                0
            };
            f = match &*effect {
                b"coordinate" => {
                    let callback = coordinator
                        .as_mut()
                        .expect("scheduler coordination requires scheduler mode");
                    let Some(Value::Word(reply)) = args.pop_back() else {
                        panic!("missing coordination reply cell")
                    };
                    let Some(Value::Blob(bytes)) = args.pop_back() else {
                        panic!("missing coordination arguments")
                    };
                    let Some(Value::Blob(name)) = args.pop_back() else {
                        panic!("missing coordination name")
                    };
                    let tree = Tree::try_from(handles::resolve(&mut k, unpack_reference(&bytes)))
                        .expect("coordination arguments must be a Tree Object");
                    let arguments = self.get_tree(tree).unwrap();
                    let (status, result) = match callback(&name, &arguments) {
                        CoordinationAction::Resume(status, value) => (status, value),
                        CoordinationAction::Stop(value) => return Ok(Execution::Complete(value)),
                    };
                    handles::insert(&mut k, reply.read() as usize, result);
                    k.apply(kernel::prelude::Word::new(status))
                }
                b"advise" => {
                    let Some(Value::Blob(payload)) = args.pop_back() else {
                        panic!("missing advice payload")
                    };
                    let Some(Value::Blob(key)) = args.pop_back() else {
                        panic!("missing advice key")
                    };
                    let Some(Value::Blob(value)) = args.pop_back() else {
                        panic!("missing advised value")
                    };
                    let target = handles::resolve(&mut k, unpack_reference(&value));
                    let function = k.apply(kernel::prelude::Word::new(0));
                    if observing {
                        return Ok(Execution::Advice {
                            event: Advice {
                                target,
                                key: key.to_vec(),
                                payload: payload.to_vec(),
                            },
                            suspended: Suspended {
                                invocation: Invocation {
                                    runtime_id,
                                    function,
                                    combination,
                                    privileged,
                                    pending,
                                    profiles,
                                },
                                application: None,
                            },
                        });
                    }
                    function
                }
                b"trap" => {
                    let Some(Value::Word(code)) = args.pop_back() else {
                        return Err(InvalidApplication);
                    };
                    let code = u32::try_from(code.read()).map_err(|_| InvalidApplication)?;
                    return Ok(self.trap(code));
                }
                b"return" => {
                    let Some(Value::Blob(value)) = args.pop_back() else {
                        panic!("missing result")
                    };
                    return Ok(Execution::Complete(handles::resolve(
                        &mut k,
                        unpack_reference(&value),
                    )));
                }
                b"effect" | b"eval" | b"return_cc" | b"call_cc" => {
                    assert!(pending.is_none(), "effect during capture preparation");
                    let Some(Value::Word(reply)) = args.pop_back() else {
                        panic!("missing reply cell")
                    };
                    let Some(Value::Blob(value)) = args.pop_back() else {
                        panic!("missing control argument")
                    };
                    let value = handles::resolve(&mut k, unpack_reference(&value));
                    if &*effect == b"effect" {
                        match handler(value) {
                            EffectAction::Resume(value) => {
                                let reply = handles::insert(&mut k, reply.read() as usize, value);
                                f = k.apply(pack_reference(reply));
                                continue;
                            }
                            EffectAction::Stop(value) => return Ok(Execution::Complete(value)),
                            EffectAction::Unhandled => {}
                        }
                    }
                    pending = Some((effect, reply.read() as usize, value));
                    k.apply(kernel::prelude::Null::new())
                }
                b"capture" => {
                    let (effect, reply, value) = pending.take().expect("unexpected capture");
                    let Some(Value::Tuple(attachments)) = args.pop_back() else {
                        panic!("missing attachments")
                    };
                    let Some(Value::Word(used)) = args.pop_back() else {
                        panic!("missing handle count")
                    };
                    let continuation =
                        self.capture(k, used.read() as usize, reply, attachments, combination);
                    if &*effect == b"eval" {
                        let metadata = Blob::literal(b"").unwrap().into();
                        return Ok(Execution::Complete(
                            self.add_tree(&[metadata, continuation.into(), value])
                                .into_ref()
                                .application()
                                .into(),
                        ));
                    }
                    return if &*effect != b"call_cc" {
                        Ok(Execution::Complete(
                            self.add_tree(&[value, continuation.into()]).into(),
                        ))
                    } else {
                        let metadata = Blob::literal(b"").unwrap().into();
                        let combination = self.add_tree(&[metadata, value, continuation.into()]);
                        let mut execution =
                            self.invoke(combination, privileged, handler, coordinator, observing)?;
                        if let Execution::Advice { suspended, .. } = &mut execution {
                            suspended.invocation.profiles.extend(profiles);
                        }
                        Ok(execution)
                    };
                }
                b"lift" => {
                    let length: kernel::prelude::Word =
                        args.pop_back().unwrap().try_into().unwrap();
                    let address: kernel::prelude::Word =
                        args.pop_back().unwrap().try_into().unwrap();
                    let objects: Tuple = args.pop_back().unwrap().try_into().unwrap();
                    self.lift(
                        &mut k,
                        objects,
                        address.read() as usize,
                        length.read() as usize,
                    );
                    k.apply(kernel::prelude::Word::new(0))
                }
                b"is_privileged" => {
                    let started = profile.then(kernel::tsc::read);
                    let result = k.apply(kernel::prelude::Word::new(privileged as u64));
                    self.record_call_time(b"is_privileged", started);
                    result
                }
                b"is_trusted_tagger" => {
                    let allowed = privileged && self.trust.is_tagger(self, combination).unwrap();
                    k.apply(kernel::prelude::Word::new(allowed as u64))
                }
                b"create_tag_unchecked" => {
                    if !privileged || !self.trust.is_tagger(self, combination).unwrap() {
                        return Ok(self.trap(WASM_TRAP_UNREACHABLE));
                    }
                    let Some(Value::Blob(reference)) = args.pop_back() else {
                        panic!("missing tag fields")
                    };
                    let tree =
                        Tree::try_from(handles::resolve(&mut k, unpack_reference(&reference)))
                            .expect("tag fields must be a Tree Object");
                    assert!(!tree.is_empty(), "missing tag author");
                    let data = self.get_tree_backing(tree).unwrap();
                    let value = unsafe { self.add_tree_backing(data) }.tag_descriptor();
                    let value = handles::insert(&mut k, result_address, value.into());
                    k.apply(pack_reference(value))
                }
                b"unsound_handle_bytes" => {
                    let started = profile.then(kernel::tsc::read);
                    if !privileged {
                        return Ok(self.trap(WASM_TRAP_UNREACHABLE));
                    }
                    let Some(Value::Blob(value)) = args.pop_back() else {
                        panic!("missing handle")
                    };
                    let value = handles::resolve(&mut k, unpack_reference(&value));
                    let bytes = self.add_blob(value.as_bytes());
                    let result = handles::insert(&mut k, result_address, bytes.into());
                    let result = k.apply(pack_reference(result));
                    self.record_call_time(b"unsound_handle_bytes", started);
                    result
                }
                b"equals" => {
                    let Some(Value::Blob(rhs)) = args.pop_back() else {
                        panic!()
                    };
                    let Some(Value::Blob(lhs)) = args.pop_back() else {
                        panic!()
                    };
                    let equal = self
                        .equals(
                            handles::resolve(&mut k, unpack_reference(&lhs)),
                            handles::resolve(&mut k, unpack_reference(&rhs)),
                        )
                        .expect("equals: invalid or non-comparable operand");
                    k.apply(kernel::prelude::Word::new(equal as u64))
                }
                b"create_tag" => {
                    let Some(Value::Blob(reference)) = args.pop_back() else {
                        panic!("missing tag fields")
                    };
                    let tree =
                        Tree::try_from(handles::resolve(&mut k, unpack_reference(&reference)))
                            .expect("tag fields must be a Tree Object");
                    assert!(!tree.is_empty(), "missing tag author");
                    let data = self.get_tree_backing(tree).unwrap();
                    let author = handles::read_backing(&data, 0);
                    if !trust::is_author(self, combination, author).unwrap() {
                        return Ok(self.trap(WASM_TRAP_UNREACHABLE));
                    }
                    let value = unsafe { self.add_tree_backing(data) };
                    let value = value.tag_descriptor();
                    let value = handles::insert(&mut k, result_address, value.into());
                    k.apply(pack_reference(value))
                }
                b"read_blob" => {
                    use kernel::prelude::{Entry, Word};

                    let Some(Value::Word(length)) = args.pop_back() else {
                        panic!("missing Blob read length")
                    };
                    let Some(Value::Word(destination)) = args.pop_back() else {
                        panic!("missing Blob read destination")
                    };
                    let Some(Value::Word(offset)) = args.pop_back() else {
                        panic!("missing Blob read offset")
                    };
                    let Some(Value::Blob(value)) = args.pop_back() else {
                        panic!("missing Blob handle")
                    };
                    let name = Blob::try_from(handles::resolve(&mut k, unpack_reference(&value)))
                        .expect("expected Blob Object");
                    let backing = self.get_blob_backing(name).unwrap();
                    let mut offset = offset.read() as usize;
                    let mut destination = destination.read() as usize;
                    let length = length.read() as usize;
                    assert!(offset <= backing.len() && length <= backing.len() - offset);
                    assert!(destination <= 1 << 32 && length <= (1 << 32) - destination);
                    let mut remaining = length;
                    while remaining != 0 {
                        let (source, position) = backing.page(offset);
                        let count = remaining
                            .min(source.len() - position)
                            .min(4096 - destination % 4096);
                        let mappings = k.inner_mut().arca_mut().unwrap().mappings_mut();
                        let base = destination & !4095;
                        let Entry::RWPage(mut page) = mappings.unmap(base).unwrap() else {
                            panic!("Blob read destination is not writable")
                        };
                        assert_eq!(page.len(), 4096);
                        source.with_ref(|bytes| {
                            assert_eq!(
                                page.write(destination % 4096, &bytes[position..position + count]),
                                count
                            );
                        });
                        assert!(mappings.map(base, Entry::RWPage(page)).is_ok());
                        offset += count;
                        destination += count;
                        remaining -= count;
                    }
                    k.apply(Word::new(length as u64))
                }
                b"get_blob" => {
                    let Some(Value::Blob(b)) = args.pop_back() else {
                        panic!()
                    };
                    let name = Blob::try_from(handles::resolve(&mut k, unpack_reference(&b)))
                        .expect("expected blob object");
                    k.apply(self.get_blob_backing(name).unwrap().attachment())
                }
                b"get_tree_element" => {
                    let started = profile.then(kernel::tsc::read);
                    let Some(Value::Word(index)) = args.pop_back() else {
                        panic!("missing Tree index")
                    };
                    let Some(Value::Blob(value)) = args.pop_back() else {
                        panic!("missing Tree handle")
                    };
                    let tree = Tree::try_from(handles::resolve(&mut k, unpack_reference(&value)))
                        .expect("expected Tree Object");
                    let index = index.read() as usize;
                    assert!(index < tree.len(), "Tree index out of bounds");
                    let child = handles::read_backing(&self.get_tree_backing(tree).unwrap(), index);
                    let result = handles::insert(&mut k, result_address, child);
                    let result = k.apply(pack_reference(result));
                    self.record_call_time(b"get_tree_element", started);
                    result
                }
                b"get_tree" => {
                    let started = profile.then(kernel::tsc::read);
                    let Some(Value::Blob(reference)) = args.pop_back() else {
                        panic!("expected Tree reference")
                    };
                    let name =
                        Tree::try_from(handles::resolve(&mut k, unpack_reference(&reference)))
                            .expect("expected tree object");
                    let backing = self.get_tree_backing(name).unwrap();
                    let result = k.apply(backing.attachment());
                    self.record_call_time(b"get_tree", started);
                    result
                }
                _ => {
                    todo!("handle effect {:?}", &*effect);
                }
            };
        }
    }
}

fn pack_reference(address: usize) -> ArcaBlob {
    ArcaBlob::new((address as u64).to_le_bytes())
}

fn unpack_reference(blob: &ArcaBlob) -> usize {
    assert_eq!(blob.len(), 8, "invalid handle reference length");
    let mut bytes = [0; 8];
    assert_eq!(blob.read(0, &mut bytes), bytes.len());
    handles::reference(&bytes)
}
