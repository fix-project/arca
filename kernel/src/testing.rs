#![allow(dead_code)]

pub struct TestDescAndFn {
    pub name: &'static str,
    pub function: fn() -> (),
}

impl TestDescAndFn {
    fn run(&self) {
        (self.function)();
        log::info!("Test {} PASS", self.name);
    }
}

pub struct ModuleDesc {
    pub name: &'static str,
    pub functions: &'static [TestDescAndFn],
}

impl ModuleDesc {
    pub fn run(&self) {
        log::info!(
            "Running {} tests for module {}",
            self.functions.len(),
            self.name
        );
        for f in self.functions {
            f.run();
        }
    }
}
