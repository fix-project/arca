mod bridge;
mod scheduler;

use fixwasm::backend::{Blob, Tree, Value};
use fixwasm::{Any, Evaluation, Handle, Value as _};
use rustpython_vm::{
    Interpreter, PyObjectRef, PyRef, PyResult, Settings, VirtualMachine, builtins::PyCode,
    compiler::Mode, scope::Scope,
};

use bridge::native::HandleValue;

fn force(value: Handle<Any>) -> Result<Value, String> {
    let result = match fixwasm::normalize_argument(&value, Evaluation::Strict) {
        Ok(Some(encode)) => encode.eval(),
        Ok(None) => Ok(value),
        Err(error) => Err(error),
    };
    result
        .map(Value::from_inner)
        .map_err(|error| format!("reading the Python entry: {error:?}"))
}

fn source_and_name(entry: Value) -> Result<(Blob, Option<String>), String> {
    let Ok(bundle) = Tree::try_from(&entry) else {
        return Blob::try_from(entry)
            .map(|source| (source, None))
            .map_err(|_| "expected a Python script Blob".to_owned());
    };
    if bundle.len() != 2 {
        return Err("expected a Tree with a script Blob and a function name Blob".to_owned());
    }
    let mut fields = bundle
        .into_iter()
        .map(|field| field.map_err(|error| format!("reading the Python entry: {error:?}")));
    let source = fields.next().unwrap()?;
    let name = fields.next().unwrap()?;
    let source =
        Blob::try_from(force(source.into_inner())?).map_err(|_| "expected a Python script Blob")?;
    let name =
        Blob::try_from(force(name.into_inner())?).map_err(|_| "expected a function name Blob")?;
    let name = String::from_utf8(
        name.to_vec()
            .map_err(|error| format!("reading the Python function name: {error:?}"))?,
    )
    .map_err(|_| "expected a UTF-8 function name")?;
    Ok((source, Some(name)))
}

fn script(combination: &Handle<fixwasm::Object<fixwasm::Tree>>) -> Result<Value, String> {
    let mut arguments = fixwasm::arguments(combination)
        .map_err(|error| format!("invalid Python invocation: {error:?}"))?
        .into_iter();
    let entry = arguments.next().ok_or("expected a Python script Blob")?;
    let (source, name) = source_and_name(force(entry)?)?;
    let bytes = source
        .to_vec()
        .map_err(|error| format!("reading the Python script: {error:?}"))?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "expected a UTF-8 Python script")?;

    let mut settings = Settings::default();
    settings.hash_seed = Some(0);
    settings.ignore_environment = true;
    settings.install_signal_handlers = false;
    settings.import_site = false;
    settings.allow_external_library = false;
    let builder = Interpreter::builder(settings);
    let native = bridge::native::module_def(&builder.ctx);
    builder.add_native_module(native).build().enter(|vm| {
        let result: PyResult<Value> = (|| {
            let fix = install_module(vm, "fix", include_str!("fix.py"))?;
            install_module(vm, "fix._do", include_str!("do.py"))?;
            let scheduler = install_module(vm, "fix.scheduler", include_str!("scheduler.py"))?;
            fix.set_attr("scheduler", scheduler, vm)?;
            install_module(vm, "flatware", include_str!("flatware.py"))?;
            fix.set_attr("_source", HandleValue::wrap(source.into(), vm), vm)?;
            let scope = vm.new_scope_with_main()?;
            let code = compile(vm, text, "<script>".to_owned())?;

            let args: Vec<PyObjectRef> = arguments
                .map(|arg| HandleValue::wrap(Value::from_inner(arg), vm).into())
                .collect();
            let invocation =
                HandleValue::wrap(Value::from_inner(combination.clone().into_any()), vm);
            let result = fix.get_attr("_run_script", vm)?.call(
                (
                    code,
                    scope.globals,
                    invocation,
                    vm.ctx.new_tuple(args),
                    vm.new_pyobj(name),
                ),
                vm,
            )?;
            let result = result
                .downcast::<HandleValue>()
                .map_err(|_| vm.new_type_error("expected a Fix value"))?;
            Ok(result.0.clone())
        })();
        result.map_err(|exception| {
            let mut text = String::new();
            let _ = vm.write_exception(&mut text, &exception);
            text
        })
    })
}

fn install_module(vm: &VirtualMachine, name: &str, source: &str) -> PyResult<PyObjectRef> {
    let scope = vm.new_scope_with_builtins();
    let module = vm.new_module(name, scope.globals.clone(), None);
    execute(vm, scope, source, format!("<{name}>"))?;
    let module: PyObjectRef = module.into();
    vm.sys_module
        .get_attr("modules", vm)?
        .set_item(name, module.clone(), vm)?;
    Ok(module)
}

fn execute(vm: &VirtualMachine, scope: Scope, source: &str, filename: String) -> PyResult<()> {
    vm.run_code_obj(compile(vm, source, filename)?, scope)?;
    Ok(())
}

fn compile(vm: &VirtualMachine, source: &str, filename: String) -> PyResult<PyRef<PyCode>> {
    vm.compile(source, Mode::Exec, filename)
        .map_err(|error| vm.new_syntax_error(&error, Some(source)))
}

#[fixwasm::apply]
fn apply(
    combination: &Handle<fixwasm::Object<fixwasm::Tree>>,
) -> Result<Handle<Any>, fixwasm::Error> {
    let result = match script(combination) {
        Ok(result) => result,
        Err(message) => Tree::new(&[
            Blob::from("Fix.error").into(),
            Blob::from(message.as_str()).into(),
        ])
        .into_value(),
    };
    Ok(result.into_inner())
}
