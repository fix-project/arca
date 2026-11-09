use super::test_program;
use fix::{
    Blob, Handle, Runtime, Storage, Thunk, Tree, arca::FixOnArca, stdlib, user_scheduler_driver,
};
use kernel::prelude::{Arc, format};

pub(super) struct Evaluation {
    runtime: Arc<FixOnArca>,
    scheduler: Tree,
    collector: Tree,
    workers: usize,
}

impl Evaluation {
    pub(super) fn new(runtime: FixOnArca) -> Self {
        Self::with_scheduler(runtime, 1, "FIX_SERIAL_SCHEDULER")
    }

    pub(super) fn with_workers(runtime: FixOnArca, workers: usize) -> Self {
        Self::with_scheduler(runtime, workers, "FIX_SCHEDULER")
    }

    fn with_scheduler(runtime: FixOnArca, workers: usize, fixture: &str) -> Self {
        let scheduler = test_program(runtime.storage(), kernel::testing::fixture(fixture));
        let collector = test_program(runtime.storage(), kernel::testing::fixture("FIX_COLLECTOR"));
        let compiler = Blob::canonical(stdlib::PRIMITIVES[0].1)
            .unwrap()
            .object()
            .into_ref();
        let mut trust =
            fix::arca::trust::Trust::parse(&format!("{}\n", compiler.as_handle())).unwrap();
        trust.add_tagger(runtime.storage(), collector).unwrap();
        Self {
            runtime: Arc::new(runtime.with_trust(trust)),
            scheduler,
            collector,
            workers,
        }
    }

    pub(super) fn runtime(&self) -> &FixOnArca {
        &self.runtime
    }

    pub(super) fn storage(&self) -> &dyn Storage {
        self.runtime.storage()
    }

    fn run(&self, mode: &str, input: Handle) -> Handle {
        let mode = self.storage().add_blob(mode.as_bytes());
        let combination = self.storage().add_tree(&[
            mode.into(),
            self.scheduler.into(),
            self.collector.into(),
            input,
        ]);
        let destination = self.runtime.clone();
        let report = user_scheduler_driver::start(
            self.runtime.clone(),
            combination,
            self.workers,
            user_scheduler_driver::PersistenceOnly(move |value| {
                destination
                    .storage()
                    .canonicalize(value)
                    .map(Into::into)
                    .map_err(|_| "save failed")
            }),
        )
        .unwrap();
        let coupon = Tree::try_from(report.result.unwrap()).unwrap();
        let claim = self.storage().certificate_claim(coupon).unwrap();
        self.storage().get_tree(claim).unwrap()[2]
    }

    pub(super) fn eval(&self, input: Handle) -> Handle {
        self.run("eval", input)
    }

    pub(super) fn think(&self, input: Thunk) -> Handle {
        self.run("think", input.into())
    }
}
