use crate::bridge::native::{HandleValue, fix_error};
use fixscheduler::{
    executor::{self, Scheduler},
    io::Io,
    kernel::{Advice, worker},
};
use fixwasm::{Error, backend::Value};
use rustpython_vm::{
    PyObjectRef, PyRef, PyResult, VirtualMachine,
    builtins::{PyBaseExceptionRef, PyTuple, PyUtf8StrRef},
    protocol::PyIterReturn,
};
use std::cell::RefCell;

struct PythonScheduler<'vm> {
    vm: &'vm VirtualMachine,
    policy: PyObjectRef,
    invoke: PyObjectRef,
    advice: PyObjectRef,
    exception: RefCell<Option<PyBaseExceptionRef>>,
}

impl PythonScheduler<'_> {
    fn report<T>(&self, result: PyResult<T>) -> Result<T, Error> {
        result.map_err(|exception| {
            self.exception.replace(Some(exception));
            Error::WrongType
        })
    }

    fn exception(&self, error: Error) -> PyBaseExceptionRef {
        self.exception
            .take()
            .unwrap_or_else(|| fix_error(error, self.vm))
    }

    async fn step(&self, operation: &str, input: &Value) -> PyResult<Value> {
        let vm = self.vm;
        let coroutine = self.invoke.call(
            (
                self.policy.clone(),
                vm.ctx.new_str(operation),
                HandleValue::wrap(input.clone(), vm),
            ),
            vm,
        )?;
        let send = coroutine.get_attr("send", vm)?;
        let mut state = send.call((vm.ctx.none(),), vm);
        loop {
            let request = match PyIterReturn::from_pyresult(state, vm)? {
                PyIterReturn::Return(value) => value
                    .downcast::<PyTuple>()
                    .map_err(|_| vm.new_type_error("expected a scheduler operation"))?,
                PyIterReturn::StopIteration(Some(result)) => {
                    return result
                        .downcast::<HandleValue>()
                        .map(|value| value.0.clone())
                        .map_err(|_| vm.new_type_error("expected a Fix value from the scheduler"));
                }
                PyIterReturn::StopIteration(None) => {
                    return Err(vm.new_type_error("expected a Fix value from the scheduler"));
                }
            };
            let [operation, value] = request.as_slice() else {
                return Err(vm.new_type_error("expected a scheduler operation and input"));
            };
            let operation: PyUtf8StrRef = operation.clone().try_into_value(vm)?;
            let value: PyRef<HandleValue> = value.clone().try_into_value(vm)?;
            let value = &value.0;
            let result = match operation.as_str() {
                "eval" => self.default_eval(value).await,
                "think" => self.default_think(value).await,
                "force" => self.default_force(value).await,
                "apply" => self.default_apply(value).await,
                "select" => self.default_select(value).await,
                "load" => self.default_load(value).await,
                _ => return Err(vm.new_value_error("unknown scheduler operation")),
            };
            state = match result {
                Ok(value) => send.call((HandleValue::wrap(value, vm),), vm),
                Err(error) => coroutine
                    .get_attr("throw", vm)?
                    .call((self.exception(error),), vm),
            };
        }
    }
}

macro_rules! steps {
    ($($operation:ident),+ $(,)?) => {$(
        async fn $operation(&self, value: &Value) -> Result<Value, Error> {
            self.report(Box::pin(self.step(stringify!($operation), value)).await)
        }
    )+};
}

impl Scheduler for PythonScheduler<'_> {
    steps!(eval, think, force, apply, select, load);

    fn on_advice(&self, advice: &Advice) -> Result<(), Error> {
        let vm = self.vm;
        let wrap = |value: &Value| HandleValue::wrap(value.clone(), vm);
        self.report(self.advice.call(
            (
                self.policy.clone(),
                wrap(&advice.target),
                wrap(&advice.key),
                wrap(&advice.payload),
            ),
            vm,
        ))?;
        Ok(())
    }

    fn session_result(&self, result: Value) -> Result<Value, Error> {
        Ok(result)
    }

    fn take_error_detail(&self) -> Option<String> {
        let exception = self.exception.take()?;
        let mut message = String::new();
        let _ = self.vm.write_exception(&mut message, &exception);
        Some(message)
    }
}

pub(crate) fn run(
    policy: PyObjectRef,
    invoke: PyObjectRef,
    advice: PyObjectRef,
    vm: &VirtualMachine,
) -> PyResult<PyRef<HandleValue>> {
    let combination = fixwasm::backend::combination();
    if combination.len() != 5 {
        return Err(vm.new_type_error("expected a scheduler invocation"));
    }
    if worker::returns_coupon().map_err(|e| fix_error(e, vm))? {
        return Err(vm.new_value_error("Python schedulers require --memoize never"));
    }
    let policy = PythonScheduler {
        vm,
        policy,
        invoke,
        advice,
        exception: RefCell::new(None),
    };
    let combination =
        fixscheduler::shell::as_tree(combination.as_value()).map_err(|e| fix_error(e, vm))?;
    let value =
        executor::run_inline(&combination, &policy, &Io).map_err(|e| policy.exception(e))?;
    worker::finish(&value).map_err(|e| fix_error(e, vm))?;
    Ok(HandleValue::wrap(value, vm))
}
