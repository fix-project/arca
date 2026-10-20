#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(crate::testing::test_runner)]
#![reexport_test_harness_main = "test_main"]

use kernel::host::os;
use kernel::prelude::*;

#[cfg(test)]
use fix::arca::FixOnArca;
#[cfg(test)]
use fix::*;

#[cfg(test)]
pub const PARSER: &[u8] = include_bytes!(env!("FIX_PARSER"));

#[cfg(test)]
mod testing;

#[cfg(test)]
#[kmain]
fn tests() {
    test_main();
}

#[cfg_attr(not(test), kmain)]
#[cfg_attr(test, allow(dead_code))]
fn main() {
    let argv = os::argv();
    let arguments = fix::cli::configure_directory(&argv[1..]).unwrap_or_else(|error| {
        println!("fix: {error}");
        kernel::exit(2);
    });

    match fix::cli::run(arguments) {
        Ok(0) => (),
        Ok(status) => kernel::exit(status),
        Err(error) => {
            println!("fix: {error}");
            kernel::exit(2);
        }
    }

    kernel::shutdown();
}

#[cfg(test)]
fn test_runtime() -> FixOnArca {
    let author = fixhandle::Blob::canonical(stdlib::PRIMITIVES[0].1).unwrap();
    FixOnArca::default()
        .with_trust(fix::arca::trust::Trust::parse(&format!("{}\n", author.as_handle())).unwrap())
}

#[cfg(test)]
fn test_program(storage: &dyn fix::storage::Storage, bytes: &[u8]) -> fixhandle::Tree {
    let author = storage.add_blob(stdlib::PRIMITIVES[0].1);
    let authors = storage.add_tree(&[author.into()]);
    storage
        .add_tree(&[
            authors.into(),
            storage.add_blob(bytes).into(),
            fixhandle::Blob::literal(b"Runnable").unwrap().into(),
        ])
        .tag_descriptor()
}

#[cfg(test)]
fn eval_program(source: &str, evaluator: &Evaluator<FixOnArca>) -> Handle {
    let processed = Preprocessor::new(source)
        .preprocess(|path| {
            kernel::host::fs::File::open(path, true, false, false, false, false)
                .and_then(|mut file| file.try_read_to_end())
                .map_err(|error| format!("{error:?}"))
        })
        .unwrap();
    let parser: Handle = test_program(evaluator.storage(), PARSER).into();
    let source = evaluator.storage().add_blob(processed.as_bytes());
    let environment = stdlib::build_environment(evaluator.storage());
    let combination = evaluator.storage().add_tree(&[
        fixhandle::Blob::literal(b"").unwrap().into(),
        parser,
        source.into(),
        environment,
    ]);

    evaluator.eval(combination.into_ref().application().strict().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fix::runtime::Runtime;
    use fix::storage::CanonicalHandle;
    use fixhandle::{Blob, Tree};

    #[test_case]
    fn runnable_authorities() {
        use fix::arca::trust::{Trust, TrustError};

        let runtime = test_runtime();
        let storage = runtime.storage();
        let compiler = storage.add_blob(&[1; 64]);
        let operator = storage.add_blob(&[2; 64]);
        let executable = storage.add_blob(b"executable");
        let canonical_compiler: Handle = storage.canonicalize(compiler.into()).unwrap().into();
        let canonical_operator: Handle = storage.canonicalize(operator.into()).unwrap().into();
        let trust = Trust::parse(&format!("{canonical_compiler}\n")).unwrap();
        let certificate = |authors: &[Handle]| {
            storage.add_tree(&[
                storage.add_tree(authors).into(),
                executable.into(),
                Blob::literal(b"Runnable").unwrap().into(),
            ])
        };
        let unconditional = certificate(&[]).tag_descriptor();
        assert!(Trust::default().runnable(storage, unconditional).is_ok());
        let compiled = certificate(&[compiler.into()]);
        assert!(matches!(
            trust.runnable(storage, compiled),
            Err(TrustError::InvalidRunnable)
        ));
        assert!(matches!(
            Trust::default().runnable(storage, compiled.tag_descriptor()),
            Err(TrustError::UntrustedAuthority)
        ));
        assert!(trust.runnable(storage, compiled.tag_descriptor()).is_ok());
        let conditional = certificate(&[compiler.into(), operator.into()]).tag_descriptor();
        assert!(matches!(
            trust.runnable(storage, conditional),
            Err(TrustError::UntrustedAuthority)
        ));
        let trust = Trust::parse(&format!("{canonical_compiler}\n{canonical_operator}\n")).unwrap();
        assert!(trust.runnable(storage, conditional).is_ok());
        assert!(Trust::parse(&format!("{}\n", compiler.as_handle())).is_err());
    }

    #[test_case]
    fn evaluation_certificates() {
        use fix::coupon::Exchange;

        fn number(value: u64) -> Handle {
            Blob::literal(&value.to_le_bytes()).unwrap().into()
        }

        fn witness(storage: &dyn Storage, coupon: Tree) -> Handle {
            let claim = storage.certificate_claim(coupon).unwrap();
            storage.add_tree(&[coupon.into(), claim.into()]).into()
        }

        fn collect(
            runtime: &FixOnArca,
            collector: Tree,
            exchange: Exchange,
            input: Handle,
            result: Handle,
            witnesses: &[Handle],
        ) -> Handle {
            let storage = runtime.storage();
            let request = storage.add_tree(&[
                Blob::literal(b"").unwrap().into(),
                collector.into(),
                number(exchange as u64),
                input,
                result,
                storage.add_tree(witnesses).into(),
            ]);
            let coupon = Tree::try_from(runtime.execute_privileged(request)).unwrap();
            assert!(coupon.is_tag());
            let claim = storage.certificate_claim(coupon).unwrap();
            let assertion = storage.get_tree(claim).unwrap();
            assert!(assertion[0].same_encoding(&number(exchange.statement() as u64)));
            assert!(assertion[1].same_encoding(&input));
            assert!(assertion[2].same_encoding(&result));
            let fields = storage.get_tree(coupon).unwrap();
            let authors = storage
                .get_tree(Tree::try_from(fields[0]).unwrap())
                .unwrap();
            assert!(
                authors
                    .iter()
                    .any(|author| author.same_encoding(&collector.into()))
            );
            witness(storage, coupon)
        }

        fn value(runtime: &FixOnArca, collector: Tree, input: Handle) -> Handle {
            if let Ok(tree) = Tree::try_from(input) {
                let witnesses: Vec<_> = runtime
                    .storage()
                    .get_tree(tree)
                    .unwrap()
                    .iter()
                    .map(|child| value(runtime, collector, *child))
                    .collect();
                collect(
                    runtime,
                    collector,
                    Exchange::EvalTree,
                    input,
                    input,
                    &witnesses,
                )
            } else {
                collect(runtime, collector, Exchange::EvalLeaf, input, input, &[])
            }
        }

        let evaluator = Evaluator::new(test_runtime());
        let runtime = evaluator.runtime();
        let storage = runtime.storage();
        let collector = test_program(storage, include_bytes!(env!("FIX_COLLECTOR")));
        let collect = |rule, input, result, witnesses: &[Handle]| {
            collect(runtime, collector, rule, input, result, witnesses)
        };
        let value = |input| value(runtime, collector, input);
        let metadata: Handle = Blob::literal(b"").unwrap().into();
        let blob = storage.add_blob(&[42; 64]);
        let reference: Handle = blob.into_ref().into();
        let thunk = blob.into_ref().identification();
        let thought = collect(Exchange::ThinkIdentification, thunk.into(), reference, &[]);
        let forced = collect(Exchange::ThinkToForce, thunk.into(), reference, &[thought]);
        let equivalent = collect(
            Exchange::ForceToEncodeStrict,
            thunk.strict().into(),
            blob.into(),
            &[forced],
        );
        let reverse = collect(
            Exchange::Symmetry,
            blob.into(),
            thunk.strict().into(),
            &[equivalent],
        );
        let strict = collect(
            Exchange::EvalEq,
            thunk.strict().into(),
            blob.into(),
            &[value(blob.into()), reverse],
        );

        let program = test_program(storage, stdlib::PRIMITIVES[0].1);
        let combination = storage.add_tree(&[metadata, program.into(), thunk.strict().into()]);
        let ready = storage.add_tree(&[metadata, program.into(), blob.into()]);
        let evaluated = collect(
            Exchange::EvalTree,
            combination.into(),
            ready.into(),
            &[value(metadata), value(program.into()), strict],
        );
        let applied = witness(storage, runtime.execute(ready));
        let application = combination.into_ref().application();
        let thought = collect(
            Exchange::ThinkApplication,
            application.into(),
            ready.into(),
            &[evaluated, applied],
        );
        let forced = collect(
            Exchange::ThinkToForce,
            application.into(),
            ready.into(),
            &[thought],
        );
        let equivalent = collect(
            Exchange::ForceToEncodeShallow,
            application.shallow().into(),
            ready.into_ref().into(),
            &[forced],
        );
        collect(Exchange::Reflexivity, reference, reference, &[]);
        collect(
            Exchange::Transitivity,
            application.shallow().into(),
            ready.into_ref().into(),
            &[
                equivalent,
                collect(
                    Exchange::Reflexivity,
                    ready.into_ref().into(),
                    ready.into_ref().into(),
                    &[],
                ),
            ],
        );

        let selection = storage.add_tree(&[combination.into(), number(2)]);
        let selection_ready = storage.add_tree(&[ready.into(), number(2)]);
        let evaluated = collect(
            Exchange::EvalTree,
            selection.into(),
            selection_ready.into(),
            &[evaluated, value(number(2))],
        );
        let selected = witness(
            storage,
            storage.selection_certificate(selection_ready).unwrap(),
        );
        let selection_thunk = selection.into_ref().selection();
        let thought = collect(
            Exchange::ThinkSelection,
            selection_thunk.into(),
            blob.into(),
            &[evaluated, selected],
        );
        let forced = collect(
            Exchange::ThinkToForce,
            selection_thunk.into(),
            blob.into(),
            &[thought],
        );
        collect(
            Exchange::ForceToEncodeStrict,
            selection_thunk.strict().into(),
            blob.into(),
            &[forced],
        );
        assert!(
            evaluator
                .eval(selection_thunk.strict().into())
                .same_encoding(&blob.into())
        );
        let parallel = parallel_evaluator::Evaluator::new(test_runtime(), 2);
        let storage = parallel.storage();
        let blob = storage.add_blob(&[42; 64]);
        let target = storage.add_tree(&[blob.into_ref().identification().strict().into()]);
        let selection = storage
            .add_tree(&[target.into(), number(0)])
            .into_ref()
            .selection();
        assert!(
            parallel
                .eval(selection.strict().into())
                .same_encoding(&blob.into())
        );
        parallel.shutdown();
    }

    #[test_case]
    fn privileged_collector() {
        let runtime = test_runtime();
        let storage = runtime.storage();
        let program = test_program(storage, include_bytes!(env!("FIX_COLLECTOR")));
        let number = |n: u64| Handle::from(Blob::literal(&n.to_le_bytes()).unwrap());
        let left: Handle = storage
            .add_blob(b"left")
            .into_ref()
            .identification()
            .strict()
            .into();
        let middle: Handle = storage.add_tree(&[left]).into();
        let right: Handle = storage
            .add_blob(b"right")
            .into_ref()
            .identification()
            .into();
        let coupon = |author, lhs, rhs| {
            let authors = storage.add_tree(&[author]);
            let claim = storage.add_tree(&[number(0), lhs, rhs]);
            let tag = storage
                .add_tree(&[authors.into(), claim.into_ref().into()])
                .tag_descriptor();
            storage.add_tree(&[tag.into(), claim.into()]).into()
        };
        let coupons = storage.add_tree(&[
            coupon(number(100), left, middle),
            coupon(number(200), middle, right),
        ]);
        let combination = storage.add_tree(&[
            Blob::literal(b"").unwrap().into(),
            program.into(),
            number(fix::coupon::Exchange::Transitivity as u64),
            left,
            right,
            coupons.into(),
        ]);
        let result = Tree::try_from(runtime.execute_privileged(combination)).unwrap();
        assert!(result.is_tag());
        let fields = storage.get_tree(result).unwrap();
        let claim = storage
            .get_tree(storage.certificate_claim(result).unwrap())
            .unwrap();
        assert!(claim[0].same_encoding(&number(0)));
        assert!(claim[1].same_encoding(&left));
        assert!(claim[2].same_encoding(&right));
        let authors = storage
            .get_tree(Tree::try_from(fields[0]).unwrap())
            .unwrap();
        assert_eq!(authors.len(), 3);
        for (actual, expected) in authors
            .iter()
            .zip([number(100), number(200), program.into()])
        {
            assert!(actual.same_encoding(&expected));
        }
    }

    #[test_case]
    fn application_coupons() {
        let runtime = test_runtime();
        let storage = runtime.storage();
        let program = test_program(storage, stdlib::PRIMITIVES[0].1);
        let number = |n: u64| Handle::from(Blob::literal(&n.to_le_bytes()).unwrap());
        let pending = storage.add_tree(&[program.into_ref().application().strict().into()]);
        let arguments = storage.add_tree(&[
            pending.into_ref().into(),
            pending.into_ref().application().into(),
        ]);
        let combination = storage.add_tree(&[
            Blob::literal(b"").unwrap().into(),
            program.into(),
            arguments.tag_descriptor().into(),
        ]);
        assert!(!combination.as_handle().is_comparable());
        let coupon = runtime.execute(combination);
        let fields = storage.get_tree(coupon).unwrap();
        assert!(coupon.is_tag());
        assert_eq!(fields.len(), 2);
        assert!(
            storage
                .get_tree(Tree::try_from(fields[0]).unwrap())
                .unwrap()
                .is_empty()
        );
        let claim = storage.certificate_claim(coupon).unwrap();
        let assertion = storage.get_tree(claim).unwrap();
        assert!(assertion[0].same_encoding(&number(2)));
        assert!(assertion[1].same_encoding(&combination.into()));
        assert!(assertion[2].same_encoding(&combination.into()));
    }

    #[test_case]
    fn canonicalization_certificates() {
        let runtime = test_runtime();
        let storage = runtime.storage();
        let blob = storage.add_blob(&[0xff; 64]);
        let tree = storage.add_tree(&[
            blob.into(),
            blob.into_ref().identification().strict().into(),
        ]);
        let number = |n: u64| Handle::from(Blob::literal(&n.to_le_bytes()).unwrap());
        for value in [
            Handle::from(blob),
            blob.into_ref().into(),
            tree.tag_descriptor().into(),
            tree.into_ref().into(),
            tree.into_ref().application().into(),
            tree.into_ref().selection().shallow().into(),
        ] {
            let certificate = storage.canonicalization_certificate(value).unwrap();
            assert!(certificate.is_tag());
            let fields = storage.get_tree(certificate).unwrap();
            assert_eq!(fields.len(), 2);
            assert!(
                storage
                    .get_tree(Tree::try_from(fields[0]).unwrap())
                    .unwrap()
                    .is_empty()
            );
            let claim = storage.certificate_claim(certificate).unwrap();
            let assertion = storage.get_tree(claim).unwrap();
            assert!(assertion[0].same_encoding(&number(0)));
            assert!(assertion[1].same_encoding(&value));
            let canonical = assertion[2];
            assert!(canonical.is_canonical());
            assert_eq!(canonical.as_bytes()[31], value.as_bytes()[31]);
            assert_eq!(canonical.is_comparable(), value.is_comparable());
        }
    }

    #[test_case]
    fn privileged_handle_bytes() {
        let runtime = test_runtime();
        let storage = runtime.storage();
        let elf = include_bytes!(env!("FIX_PRIVILEGED"));
        let program = test_program(storage, elf);
        let combination = storage.add_tree(&[Blob::literal(b"").unwrap().into(), program.into()]);
        assert!(!combination.as_handle().is_canonical());
        let result = runtime.execute_privileged(combination);
        assert_eq!(
            storage
                .get_blob(Blob::try_from(result).unwrap())
                .unwrap()
                .as_ref(),
            combination.as_handle().as_bytes()
        );
        let function = common::elfloader::load_elf(elf).unwrap();
        let argument =
            kernel::prelude::Blob::new(Blob::literal(b"").unwrap().as_handle().into_bytes());
        let effect: kernel::prelude::Function =
            function.apply(argument).force().try_into().unwrap();
        let description: kernel::prelude::Tuple = effect.into_inner().read().try_into().unwrap();
        let name: kernel::prelude::Blob = description.get(1).try_into().unwrap();
        assert_eq!(&*name, b"is_privileged");
        let mut arguments: kernel::prelude::Tuple = description.get(2).try_into().unwrap();
        let continuation: kernel::prelude::Function = arguments.take(0).try_into().unwrap();
        let trapped: kernel::prelude::Function = continuation
            .apply(kernel::prelude::Word::new(0))
            .force()
            .try_into()
            .unwrap();
        let description: kernel::prelude::Tuple = trapped.into_inner().read().try_into().unwrap();
        let arguments: kernel::prelude::Tuple = description.get(2).try_into().unwrap();
        let reason: kernel::prelude::Blob = arguments.get(0).try_into().unwrap();
        assert_eq!(&*reason, b"exception");
    }

    #[test_case]
    fn canonical_blob_round_trip() {
        let storage = storage::memory::MemoryStorage::default();
        let bytes = [42u8; 64];
        let local = storage.add_blob(&bytes);
        let canonical = storage.canonicalize_blob(local).unwrap();
        let handle = Handle::from(canonical);
        let blob = Blob::try_from(handle).unwrap();
        assert_eq!(storage.get_blob(blob).unwrap().as_ref(), &bytes);

        let duplicate = storage.add_blob(&bytes);
        assert_ne!(
            local.as_handle().as_bytes(),
            duplicate.as_handle().as_bytes()
        );
        assert_eq!(storage.canonicalize_blob(duplicate), Ok(canonical));

        let decoded =
            Blob::try_from(unsafe { Handle::parse(*handle.as_bytes()) }.unwrap()).unwrap();
        assert_eq!(CanonicalHandle::try_from(decoded).unwrap(), canonical);
        assert_eq!(storage.get_blob(decoded).unwrap().as_ref(), &bytes);
    }

    #[test_case]
    fn canonical_tree_round_trip() {
        fn build(storage: &storage::memory::MemoryStorage) -> Tree {
            let blob = storage.add_blob(&[42; 64]);
            let tree = storage.add_tree(&[blob.into(), Blob::literal(b"leaf").unwrap().into()]);
            storage.add_tree(&[
                blob.into(),
                tree.into(),
                tree.tag_descriptor().into(),
                blob.object().into_ref().into(),
                tree.into_ref().into(),
                blob.object().into_ref().identification().into(),
                tree.into_ref().application().into(),
                tree.into_ref().selection().into(),
                tree.into_ref().application().strict().into(),
                tree.into_ref().selection().shallow().into(),
            ])
        }

        let storage = storage::memory::MemoryStorage::default();
        let other = storage::memory::MemoryStorage::default();
        let original = build(&storage);
        let original_children = storage.get_tree(original).unwrap();
        let canonical = storage.canonicalize_tree(original).unwrap();
        assert_eq!(
            storage.canonicalize_tree(build(&storage)).unwrap(),
            canonical
        );
        assert_eq!(other.canonicalize_tree(build(&other)).unwrap(), canonical);
        assert_eq!(
            storage
                .canonicalize_tree(Tree::try_from(Handle::from(canonical)).unwrap())
                .unwrap(),
            canonical
        );

        let children = storage
            .get_tree(Tree::try_from(Handle::from(canonical)).unwrap())
            .unwrap();
        for (original, child) in original_children.iter().zip(children.iter()) {
            assert_eq!(original.as_bytes()[31], child.as_bytes()[31]);
            assert!(child.is_canonical());
        }
        let blob = Blob::try_from(children[0]).unwrap();
        assert_eq!(storage.get_blob(blob).unwrap().as_ref(), &[42; 64]);
        let tree = Tree::try_from(children[1]).unwrap();
        let tagged = Tree::try_from(children[2]).unwrap();
        assert!(tagged.is_tag());
        let nested = storage.get_tree(tree).unwrap();
        let tagged_children = storage.get_tree(tagged).unwrap();
        assert_eq!(nested[0].as_bytes(), tagged_children[0].as_bytes());
        assert_eq!(
            Blob::try_from(nested[1]).unwrap().literal_bytes().unwrap(),
            b"leaf"
        );
        assert_eq!(
            storage
                .get_blob(Blob::try_from(nested[0]).unwrap())
                .unwrap()
                .as_ref(),
            &[42; 64]
        );
    }

    #[test_case]
    fn equality_of_aliases_and_computed_values() {
        use fix::storage::{StorageError, memory::MemoryStorage};
        let storage = MemoryStorage::default();
        let a = storage.add_blob(&[7; 64]);
        let b = storage.add_blob(&[7; 64]);
        let c = storage.add_blob(&[8; 64]);
        assert!(a.as_handle().is_comparable());
        assert_eq!(storage.equals(a.into(), b.into()), Ok(true));
        assert_eq!(storage.equals(a.into(), c.into()), Ok(false));
        let canonical: Handle = storage.canonicalize_blob(a).unwrap().into();
        assert_eq!(storage.equals(a.into(), canonical), Ok(true));
        assert_eq!(storage.equals(a.into(), a.into_ref().into()), Ok(false));
        assert_eq!(
            storage.equals(a.into_ref().into(), b.into_ref().into()),
            Ok(true)
        );
        let left = storage.add_tree(&[a.into(), storage.add_tree(&[a.into_ref().into()]).into()]);
        let right = storage.add_tree(&[b.into(), storage.add_tree(&[b.into_ref().into()]).into()]);
        assert!(left.as_handle().is_comparable());
        assert_eq!(storage.equals(left.into(), right.into()), Ok(true));
        let canonical: Handle = storage.canonicalize_tree(left).unwrap().into();
        assert_eq!(storage.equals(right.into(), canonical), Ok(true));
        let other = MemoryStorage::default();
        assert_eq!(other.equals(canonical, canonical), Ok(true));
        let reference = Object::try_from(canonical).unwrap().into_ref().handle();
        assert_eq!(other.equals(reference, reference), Ok(true));
        assert_eq!(
            storage.equals(left.into_ref().into(), right.into_ref().into()),
            Ok(true)
        );
        let different = storage.add_tree(&[
            b.into_ref().into(),
            storage.add_tree(&[b.into_ref().into()]).into(),
        ]);
        assert_eq!(storage.equals(left.into(), different.into()), Ok(false));
        let thunk = a.into_ref().identification();
        for value in [
            Handle::from(thunk),
            thunk.strict().into(),
            thunk.shallow().into(),
            left.into_ref().application().into(),
        ] {
            assert!(!value.is_comparable());
            assert_eq!(
                storage.equals(value, value),
                Err(StorageError::NotComparable)
            );
            let tree = storage.add_tree(&[value]);
            let canonical: Handle = storage.canonicalize_tree(tree).unwrap().into();
            assert!(canonical.is_canonical());
            assert!(!canonical.is_comparable());
            assert_eq!(
                storage.equals(canonical, canonical),
                Err(StorageError::NotComparable)
            );
            assert!(
                !Tree::try_from(canonical)
                    .unwrap()
                    .into_ref()
                    .as_handle()
                    .is_comparable()
            );
        }
        let empty = storage.add_tree(&[]);
        assert!(empty.as_handle().is_comparable());
        assert_eq!(storage.equals(empty.into(), empty.into()), Ok(true));
        for (bit, error) in [
            (4, StorageError::InvalidComparisonBit),
            (8, StorageError::InvalidValueBit),
        ] {
            let mut forged = *left.as_handle().as_bytes();
            forged[30] ^= bit;
            assert_eq!(
                storage
                    .get_tree(Tree::try_from(unsafe { Handle::parse(forged) }.unwrap()).unwrap())
                    .unwrap_err(),
                error
            );
        }
    }

    #[test_case]
    fn equality_after_encoded_substitution() {
        let evaluator = Evaluator::new(test_runtime());
        let storage = evaluator.storage();
        let blob = storage.add_blob(&[42; 64]);
        let identification = blob.into_ref().identification().strict();
        let source = storage.add_tree(&[blob.into()]);
        let index = storage.add_blob(&0u64.to_le_bytes());
        let selection = storage
            .add_tree(&[source.into_ref().into(), index.into()])
            .into_ref()
            .selection()
            .strict();
        let first = storage.add_tree(&[identification.into()]);
        let second = storage.add_tree(&[selection.into()]);
        assert!(!first.as_handle().is_comparable() && !second.as_handle().is_comparable());
        assert_ne!(
            storage.canonicalize_tree(first).unwrap(),
            storage.canonicalize_tree(second).unwrap()
        );
        let first = evaluator.eval(
            first
                .into_ref()
                .reference()
                .identification()
                .strict()
                .into(),
        );
        let second = evaluator.eval(
            second
                .into_ref()
                .reference()
                .identification()
                .strict()
                .into(),
        );
        assert!(first.is_comparable() && second.is_comparable());
        assert_eq!(storage.equals(first, second), Ok(true));
    }

    #[test_case]
    fn evaluation_preserves_access_and_tags() {
        fn check(storage: &dyn storage::Storage, eval: impl Fn(Handle) -> Handle) {
            let blob = storage.add_blob(b"ok");
            let thunk = blob.into_ref().identification();
            assert!(matches!(
                eval(thunk.shallow().into()).view(),
                HandleView::Ref(_)
            ));
            assert!(matches!(
                eval(thunk.strict().into()).view(),
                HandleView::Object(_)
            ));

            let index = storage.add_blob(&0u64.to_le_bytes());
            let invalid_index = storage.add_blob(&99u64.to_le_bytes());
            let invalid = storage.add_tree(&[blob.into_ref().into(), invalid_index.into()]);
            let source =
                storage.add_tree(&[blob.into(), invalid.into_ref().selection().strict().into()]);
            for index in [
                Handle::from(index),
                index.into_ref().identification().strict().into(),
            ] {
                let selection = storage.add_tree(&[source.into_ref().into(), index]);
                let selected = eval(selection.into_ref().selection().strict().into());
                assert_eq!(storage.equals(selected, blob.into()), Ok(true));
            }

            let tag = storage
                .add_tree(&[blob.into(), thunk.strict().into(), blob.into()])
                .tag_descriptor();
            let result = Tree::try_from(eval(tag.into())).unwrap();
            assert!(result.is_tag());
            assert!(
                storage
                    .get_tree(result)
                    .unwrap()
                    .iter()
                    .all(Handle::is_comparable)
            );
            let plain = storage.add_tree(&storage.get_tree(result).unwrap());
            assert_eq!(storage.equals(result.into(), plain.into()), Ok(false));

            let leaf = storage.add_tree(&[blob.into()]);
            let pending = storage.add_tree(&[thunk.strict().into()]).into_ref();
            assert_eq!(
                storage.equals(pending.into(), pending.into()),
                Err(storage::StorageError::NotComparable)
            );
            assert_eq!(
                storage.equals(leaf.into_ref().into(), leaf.into_ref().into()),
                Ok(true)
            );
            assert_eq!(eval(leaf.into()).as_bytes(), leaf.as_handle().as_bytes());
            for target in [Handle::from(blob), Handle::from(leaf)] {
                let end = storage.add_blob(&(target.len() as u64).to_le_bytes());
                let entire = storage.add_tree(&[target, index.into(), end.into()]);
                let entire = eval(entire.into_ref().selection().strict().into());
                assert_eq!(storage.equals(entire, target), Ok(true));
                let empty = storage.add_tree(&[target, end.into(), end.into()]);
                let empty = eval(empty.into_ref().selection().strict().into());
                assert_eq!(empty.len(), 0);
            }
            let references = storage.add_tree(&[leaf.into_ref().into(), blob.into_ref().into()]);
            let loaded = eval(references.into());
            let children = storage.get_tree(Tree::try_from(loaded).unwrap()).unwrap();
            assert!(
                children
                    .iter()
                    .all(|child| matches!(child.view(), HandleView::Ref(_)))
            );
            assert_eq!(eval(references.into()).as_bytes(), loaded.as_bytes());
        }
        let serial = Evaluator::new(test_runtime());
        check(serial.storage(), |value| serial.eval(value));
        let parallel = fix::parallel_evaluator::Evaluator::new(test_runtime(), 2);
        check(parallel.storage(), |value| parallel.eval(value));
        parallel.shutdown();
    }

    #[test_case]
    fn independent_applications_run_in_parallel() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        struct Runtime {
            storage: storage::memory::MemoryStorage,
            started: AtomicUsize,
        }
        impl fix::runtime::Runtime for Runtime {
            fn storage(&self) -> &dyn storage::Storage {
                &self.storage
            }
            fn execute(&self, combination: Tree) -> Tree {
                self.started.fetch_add(1, Ordering::SeqCst);
                while self.started.load(Ordering::SeqCst) < 2 {
                    kernel::kthread::yield_now();
                }
                let result = self.storage.add_blob(b"done").into();
                self.storage.certificate(2, combination.into(), result)
            }
        }
        let evaluator = fix::parallel_evaluator::Evaluator::new(
            Runtime {
                storage: storage::memory::MemoryStorage::default(),
                started: AtomicUsize::new(0),
            },
            2,
        );
        let storage = evaluator.storage();
        let metadata = storage.add_blob(b"");
        let combination = storage.add_tree(&[metadata.into(), metadata.into()]);
        let application = combination.into_ref().application().strict();
        let requests = storage.add_tree(&[application.into(), application.into()]);
        let result = Tree::try_from(evaluator.eval(requests.into())).unwrap();
        let nested = storage.add_tree(&[application.into()]);
        let batch = storage.add_tree(&vec![
            nested
                .into_ref()
                .reference()
                .identification()
                .strict()
                .into();
            2048
        ]);
        let evaluated = evaluator.eval(batch.into());
        assert!(evaluated.is_comparable());
        assert_eq!(Tree::try_from(evaluated).unwrap().len(), 2048);
        let expected = storage.add_blob(b"done");
        for child in storage.get_tree(result).unwrap().iter().copied() {
            assert_eq!(storage.equals(child, expected.into()), Ok(true));
        }
        let weak = Arc::downgrade(&evaluator);
        evaluator.shutdown();
        drop(evaluator);
        while weak.upgrade().is_some() {
            kernel::kthread::yield_now();
        }
    }

    fn eval_value(source: &str, evaluator: &Evaluator<FixOnArca>) -> Vec<u8> {
        let handle = eval_program(source, evaluator);
        let object = match handle.view() {
            HandleView::Object(object) => *object,
            HandleView::Ref(reference) => reference.object_descriptor(),
            _ => panic!(),
        };
        match object.view() {
            ObjectView::Blob(blob) => evaluator.storage().get_blob(*blob).unwrap().into(),
            ObjectView::Tree(tree) => tree.len().to_le_bytes().into(),
        }
    }

    #[test_case]
    fn test_memory_storage_handle_boundaries() {
        use fix::storage::{StorageError, memory::MemoryStorage};

        let storage = MemoryStorage::default();
        let other = MemoryStorage::default();
        let contents = [0x5a; 31];
        let blob = storage.add_blob(&contents);
        let empty = storage.add_blob(&[]);
        let children = [empty.into(), blob.into()];
        let tree = storage.add_tree(&children);

        assert_eq!(storage.get_blob(blob).unwrap().as_ref(), &contents);
        assert!(storage.get_blob(empty).unwrap().is_empty());
        assert!(other.get_blob(empty).unwrap().is_empty());
        assert!(matches!(
            other.get_blob(blob),
            Err(StorageError::WrongStorage)
        ));
        assert!(matches!(
            other.get_tree(tree),
            Err(StorageError::WrongStorage)
        ));
        assert!(matches!(
            other.canonicalization_certificate(blob.into()),
            Err(StorageError::WrongStorage)
        ));

        let Name::Machine { index, storage_id } = blob.as_handle().name() else {
            panic!("expected machine blob");
        };
        let wrong_length =
            unsafe { fixhandle::Blob::named(Name::Machine { index, storage_id }, 32, false) }
                .unwrap();
        assert!(matches!(
            storage.get_blob(wrong_length),
            Err(StorageError::LengthMismatch)
        ));
        let missing = unsafe {
            fixhandle::Blob::named(
                Name::Machine {
                    index: index + 1,
                    storage_id,
                },
                contents.len() as u64,
                false,
            )
        }
        .unwrap();
        assert!(matches!(
            storage.get_blob(missing),
            Err(StorageError::NotFound)
        ));
        let local = unsafe {
            fixhandle::Blob::named(
                Name::Local {
                    address: contents.as_ptr() as u64,
                },
                contents.len() as u64,
                true,
            )
        }
        .unwrap();
        assert!(matches!(
            storage.get_blob(local),
            Err(StorageError::UnsupportedName)
        ));

        let wrong_tree_length =
            unsafe { Tree::named(tree.as_handle().name(), HANDLE_SIZE as u64, false, false) }
                .unwrap();
        assert!(matches!(
            storage.get_tree(wrong_tree_length),
            Err(StorageError::LengthMismatch)
        ));
        let local_tree = unsafe {
            Tree::named(
                Name::Local {
                    address: children.as_ptr() as u64,
                },
                core::mem::size_of_val(&children) as u64,
                false,
                false,
            )
        }
        .unwrap();
        assert!(matches!(
            storage.get_tree(local_tree),
            Err(StorageError::UnsupportedName)
        ));

        assert_eq!(tree.len(), children.len());
        assert_eq!(tree.byte_len(), (HANDLE_SIZE * children.len()) as u64);
        assert_eq!(
            &tree.as_handle().as_bytes()[24..30],
            &((HANDLE_SIZE * children.len()) as u64).to_le_bytes()[..6]
        );
        let retrieved = storage.get_tree(tree).unwrap();
        assert_eq!(
            retrieved.as_ptr() as usize % core::mem::align_of::<Handle>(),
            0
        );
        assert_eq!(retrieved.len(), children.len());
        for (actual, expected) in retrieved.iter().zip(children.iter()) {
            assert_eq!(actual.as_bytes(), expected.as_bytes());
        }
    }

    #[test_case]
    fn test_shared_storage_pages() {
        use fix::storage::memory::MemoryStorage;
        use kernel::types::Entry;
        let storage = MemoryStorage::default();
        let bytes = [0x5a; 8193];
        let blob = storage.add_blob(&bytes);
        let first = storage.get_blob_backing(blob).unwrap();
        let second = storage.get_blob_backing(blob).unwrap();
        let Entry::ROPage(mut a) = kernel::prelude::Table::try_from(first.value())
            .unwrap()
            .get(0)
            .unwrap()
        else {
            panic!()
        };
        let Entry::ROPage(b) = kernel::prelude::Table::try_from(second.value())
            .unwrap()
            .get(0)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(a.with_ref(|p| p.as_ptr()), b.with_ref(|p| p.as_ptr()));
        a.write(0, &[1]);
        assert_ne!(a.with_ref(|p| p.as_ptr()), b.with_ref(|p| p.as_ptr()));
        assert_eq!(storage.get_blob(blob).unwrap().as_ref(), &bytes);
        let tree = storage.add_tree(&[blob.into(), blob.into()]);
        let first = storage.get_tree_backing(tree).unwrap();
        let second = storage.get_tree_backing(tree).unwrap();
        let Entry::ROPage(a) = kernel::prelude::Table::try_from(first.value())
            .unwrap()
            .get(0)
            .unwrap()
        else {
            panic!()
        };
        let Entry::ROPage(b) = kernel::prelude::Table::try_from(second.value())
            .unwrap()
            .get(0)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(a.with_ref(|p| p.as_ptr()), b.with_ref(|p| p.as_ptr()));
        let backing = fix::storage::ImmutableBytes::from_tables(
            vec![first.value().try_into().unwrap()],
            first.len(),
        );
        let adopted = unsafe { storage.add_tree_backing(backing) };
        let adopted_backing = storage.get_tree_backing(adopted).unwrap();
        let Entry::ROPage(adopted_page) = kernel::prelude::Table::try_from(adopted_backing.value())
            .unwrap()
            .get(0)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(
            a.with_ref(|p| p.as_ptr()),
            adopted_page.with_ref(|p| p.as_ptr())
        );
        assert_eq!(storage.equals(tree.into(), adopted.into()), Ok(true));
        assert_eq!(first.len(), 2 * HANDLE_SIZE);
        assert!(a.with_ref(|p| p[2 * HANDLE_SIZE..].iter().all(|b| *b == 0)));
    }

    #[test_case]
    fn test_nested_shared_backing() {
        use fix::storage::ImmutableBytes;
        use kernel::types::Entry;
        let mut bytes = Vec::new();
        bytes.resize((1 << 21) + 1, 0x29);
        let backing = ImmutableBytes::new(&bytes);
        let a: kernel::prelude::Table = backing.value().try_into().unwrap();
        let b = a.clone();
        let (Entry::ROTable(a) | Entry::RWTable(a)) = a.get(1).unwrap() else {
            panic!()
        };
        let (Entry::ROTable(b) | Entry::RWTable(b)) = b.get(1).unwrap() else {
            panic!()
        };
        match (a.inner(), b.inner()) {
            (
                kernel::types::internal::Table::Table2MB(kernel::prelude::CowPage::Shared(a)),
                kernel::types::internal::Table::Table2MB(kernel::prelude::CowPage::Shared(b)),
            ) => {
                assert!(core::ptr::eq(&**a, &**b));
            }
            _ => panic!("expected shared child Tables"),
        }
        let Entry::ROPage(a) = a.get(0).unwrap() else {
            panic!()
        };
        let Entry::ROPage(b) = b.get(0).unwrap() else {
            panic!()
        };
        assert_eq!(
            a.with_ref(|page| page.as_ptr()),
            b.with_ref(|page| page.as_ptr())
        );
        assert_eq!(&*backing.to_bytes(), bytes.as_slice());
    }

    #[test_case]
    fn immutable_backing_edits_share_unchanged_pages() {
        use fix::storage::ImmutableBytes;
        let bytes = vec![0x29; 1 << 21];
        let original = ImmutableBytes::new(&bytes);
        let edited = original.clone().with_bytes(4094, &[1; 4]);
        let extended = edited.clone().with_bytes(edited.len(), &[2; 32]);
        assert_eq!(&*original.to_bytes(), bytes.as_slice());
        let mut expected = bytes;
        expected[4094..4098].fill(1);
        assert_eq!(&*edited.to_bytes(), expected.as_slice());
        expected.extend_from_slice(&[2; 32]);
        assert_eq!(&*extended.to_bytes(), expected.as_slice());
        for offset in [8192, (1 << 21) - 4096] {
            let before = original.page(offset).0;
            let after = extended.page(offset).0;
            assert_eq!(
                before.with_ref(|p| p.as_ptr()),
                after.with_ref(|p| p.as_ptr())
            );
        }
        let original = ImmutableBytes::from_page(Page::new(8));
        let length = original.len();
        let extended = original.clone().with_bytes(length, &[7; 32]);
        assert!(original.to_bytes().iter().all(|byte| *byte == 0));
        assert_eq!(&extended.to_bytes()[length..], &[7; 32]);
    }

    #[test_case]
    fn test_sparse_chunk_container() {
        use kernel::types::{Entry, Page, Table, Value};
        let mut first = Table::new(1 << 30);
        let mut last = Page::new(4096);
        last.write(4095, &[0x29]);
        assert!(first.map((1 << 30) - 4096, Entry::ROPage(last)).is_ok());
        let mut second = Table::new(1 << 30);
        let mut start = Page::new(4096);
        start.write(0, &[0x5a]);
        assert!(second.map(0, Entry::ROPage(start)).is_ok());
        let Value::Tuple(chunks) = fix::storage::backing_value(&[first, second]) else {
            panic!()
        };
        assert_eq!(chunks.len(), 2);
        let Value::Table(first) = chunks.get(0) else {
            panic!()
        };
        let Value::Table(second) = chunks.get(1) else {
            panic!()
        };
        assert_eq!(first.len(), 1 << 30);
        assert_eq!(second.len(), 1 << 30);
        let Entry::RWTable(first) = first.get(511).unwrap() else {
            panic!()
        };
        let Entry::ROPage(last) = first.get(511).unwrap() else {
            panic!()
        };
        let Entry::RWTable(second) = second.get(0).unwrap() else {
            panic!()
        };
        let Entry::ROPage(start) = second.get(0).unwrap() else {
            panic!()
        };
        assert_eq!(last.with_ref(|page| page[4095]), 0x29);
        assert_eq!(start.with_ref(|page| page[0]), 0x5a);
    }

    #[test_case]
    fn test_number() {
        let evaluator = Evaluator::new(test_runtime());

        {
            assert_eq!(eval_value("42u8", &evaluator), 42u8.to_le_bytes());
            //assert_eq!(eval_value("-1", &evaluator), (-1i64).to_le_bytes());
            assert_eq!(eval_value("\"hello\"", &evaluator), b"hello");

            assert_eq!(eval_value("(1u8 2u8 3u8)", &evaluator), 3u64.to_le_bytes());
            assert_eq!(eval_value("()", &evaluator), 0u64.to_le_bytes());

            assert_eq!(eval_value("&\"hello\"", &evaluator), b"hello");
            assert_eq!(eval_value("&(1u8 2u8 3u8)", &evaluator), 3u64.to_le_bytes());

            assert_eq!(eval_value("*'&2u8", &evaluator), 2u8.to_le_bytes());
            assert_eq!(
                eval_value("*[(1u8 2u8 3u8 4u8) 2u64]", &evaluator),
                3u8.to_le_bytes()
            );

            assert_eq!(eval_value("x = 42u64\nx", &evaluator), 42u64.to_le_bytes());
            assert_eq!(
                eval_value("x = 1u8\ny = 2u8\n(x y)", &evaluator),
                2u64.to_le_bytes()
            );

            assert_eq!(
                eval_value("x = 1u8\nx = 2u64\nx", &evaluator),
                2u64.to_le_bytes()
            );
            assert_eq!(
                eval_value("x = 1u64\ny = x\nx = 2u8\ny", &evaluator),
                1u64.to_le_bytes()
            );
        }

        // primitive test
        {
            assert_eq!(
                eval_value("*#(\"\" $identity 2u8)", &evaluator),
                3u64.to_le_bytes()
            );
        }
    }
}
