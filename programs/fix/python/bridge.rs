#[rustpython_vm::pymodule(name = "_fix")]
pub(crate) mod native {
    use fix::FixShell;
    use fixwasm::{
        Any, DataType, EncodeType, Error, Evaluation, Handle, ValueType,
        backend::{Blob, Object, Reference, Shell, Thunk, Tree, Value},
        files::{self, Stat},
    };
    use rustpython_vm::{
        PyObjectRef, PyPayload, PyRef, PyResult, VirtualMachine,
        builtins::{PyBaseExceptionRef, PyBytesRef, PyUtf8StrRef},
        function::ArgIterable,
        pyclass,
    };

    #[pyattr]
    #[pyclass(name = "_Handle")]
    #[derive(Debug, PyPayload)]
    pub(crate) struct HandleValue(pub(crate) Value);

    pub(crate) fn fix_error(error: Error, vm: &VirtualMachine) -> PyBaseExceptionRef {
        vm.new_runtime_error(format!("Fix error: {error:?}"))
    }

    fn wrong_type(_: fix::TypeError, vm: &VirtualMachine) -> PyBaseExceptionRef {
        vm.new_type_error("wrong Fix value type")
    }

    #[pyclass]
    impl HandleValue {
        #[pymethod]
        fn kind(&self) -> &'static str {
            match self.0.value_type() {
                ValueType::Object => match Shell::data_type(self.0.inner()) {
                    DataType::Blob => "blob",
                    DataType::Tree => "tree",
                },
                ValueType::Ref => "reference",
                ValueType::Thunk => "thunk",
                ValueType::Encode => "encode",
            }
        }

        #[pymethod]
        fn data_type(&self, vm: &VirtualMachine) -> PyResult<&'static str> {
            let kind = match self.0.value_type() {
                ValueType::Object | ValueType::Ref => Shell::data_type(self.0.inner()),
                _ => return Err(vm.new_type_error("expected a Fix object or reference")),
            };
            Ok(match kind {
                DataType::Blob => "blob",
                DataType::Tree => "tree",
            })
        }

        #[pymethod]
        fn encode_type(&self, vm: &VirtualMachine) -> PyResult<&'static str> {
            let encode =
                fixwasm::backend::Encode::try_from(&self.0).map_err(|e| wrong_type(e, vm))?;
            Ok(match encode.encode_type() {
                EncodeType::Strict => "strict",
                EncodeType::Shallow => "shallow",
            })
        }

        #[pymethod]
        fn len(&self, vm: &VirtualMachine) -> PyResult<u64> {
            self.data_type(vm)?;
            Ok(Shell::len(self.0.inner()))
        }

        #[pymethod]
        fn read(&self, offset: usize, count: usize, vm: &VirtualMachine) -> PyResult<PyBytesRef> {
            let blob = Blob::try_from(&self.0).map_err(|e| wrong_type(e, vm))?;
            let mut bytes = vec![0; count.min(blob.len().saturating_sub(offset))];
            blob.read_exact(offset, &mut bytes)
                .map_err(|e| fix_error(e, vm))?;
            Ok(vm.ctx.new_bytes(bytes))
        }

        #[pymethod]
        fn get(&self, index: usize, vm: &VirtualMachine) -> PyResult<PyRef<Self>> {
            let tree = self.tree(vm)?;
            let value = tree
                .get(index)
                .map_err(|e| fix_error(e, vm))?
                .ok_or_else(|| vm.new_index_error("Fix tree index out of range"))?;
            Ok(Self::wrap(value, vm))
        }

        #[pymethod]
        fn is_tag(&self) -> bool {
            self.0.value_type() == ValueType::Object && Shell::is_tag(self.0.inner())
        }

        #[pymethod]
        fn is_comparable(&self) -> bool {
            self.0.is_comparable()
        }

        #[pymethod]
        fn is_value(&self, vm: &VirtualMachine) -> PyResult<bool> {
            fixwasm::Value::is_value(self.0.inner()).map_err(|e| fix_error(e, vm))
        }

        #[pymethod]
        fn equals(&self, other: PyRef<Self>, vm: &VirtualMachine) -> PyResult<bool> {
            self.0.equals(&other.0).map_err(|e| fix_error(e, vm))
        }

        #[pymethod]
        fn reference(&self, vm: &VirtualMachine) -> PyResult<PyRef<Self>> {
            let object = Object::try_from(&self.0).map_err(|e| wrong_type(e, vm))?;
            Ok(Self::wrap(object.reference().into(), vm))
        }

        #[pymethod]
        fn identification(&self, vm: &VirtualMachine) -> PyResult<PyRef<Self>> {
            let reference = Reference::try_from(&self.0).map_err(|e| wrong_type(e, vm))?;
            Ok(Self::wrap(reference.identification().into(), vm))
        }

        #[pymethod]
        fn application(&self, vm: &VirtualMachine) -> PyResult<PyRef<Self>> {
            Ok(Self::wrap(self.tree(vm)?.application().into(), vm))
        }

        #[pymethod]
        fn selection(&self, vm: &VirtualMachine) -> PyResult<PyRef<Self>> {
            Ok(Self::wrap(self.tree(vm)?.selection().into(), vm))
        }

        #[pymethod]
        fn strict(&self, vm: &VirtualMachine) -> PyResult<PyRef<Self>> {
            let thunk = Thunk::try_from(&self.0).map_err(|e| wrong_type(e, vm))?;
            Ok(Self::wrap(thunk.strict().into(), vm))
        }

        #[pymethod]
        fn shallow(&self, vm: &VirtualMachine) -> PyResult<PyRef<Self>> {
            let thunk = Thunk::try_from(&self.0).map_err(|e| wrong_type(e, vm))?;
            Ok(Self::wrap(thunk.shallow().into(), vm))
        }

        #[pymethod]
        fn eval(&self, vm: &VirtualMachine) -> PyResult<PyRef<Self>> {
            let value = self.0.eval().map_err(|e| fix_error(e, vm))?;
            Ok(Self::wrap(value, vm))
        }

        #[pymethod]
        fn advise(&self, key: PyUtf8StrRef, payload: PyBytesRef) {
            self.0.advise(key.as_str(), payload.as_bytes());
        }
    }

    #[pyfunction]
    fn effect(value: PyRef<HandleValue>, vm: &VirtualMachine) -> PyResult<PyRef<HandleValue>> {
        let tree = value.tree(vm)?;
        Ok(HandleValue::wrap(tree.as_value().effect(), vm))
    }

    #[pyfunction]
    fn run_scheduler(
        policy: PyObjectRef,
        invoke: PyObjectRef,
        advice: PyObjectRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyRef<HandleValue>> {
        crate::scheduler::run(policy, invoke, advice, vm)
    }

    impl HandleValue {
        pub(crate) fn wrap(value: Value, vm: &VirtualMachine) -> PyRef<Self> {
            Self(value).into_ref(&vm.ctx)
        }

        fn tree(&self, vm: &VirtualMachine) -> PyResult<Tree> {
            Tree::try_from(&self.0).map_err(|e| wrong_type(e, vm))
        }
    }

    fn values(
        children: ArgIterable<PyRef<HandleValue>>,
        vm: &VirtualMachine,
    ) -> PyResult<Vec<Value>> {
        children
            .iter(vm)?
            .map(|child| Ok(child?.0.clone()))
            .collect()
    }

    #[pyfunction]
    fn blob(bytes: PyBytesRef, vm: &VirtualMachine) -> PyRef<HandleValue> {
        HandleValue::wrap(Blob::new(bytes.as_bytes()).into(), vm)
    }

    #[pyfunction]
    fn tree(
        children: ArgIterable<PyRef<HandleValue>>,
        vm: &VirtualMachine,
    ) -> PyResult<PyRef<HandleValue>> {
        Ok(HandleValue::wrap(
            Tree::new(&values(children, vm)?).into(),
            vm,
        ))
    }

    #[pyfunction]
    fn tag(
        children: ArgIterable<PyRef<HandleValue>>,
        vm: &VirtualMachine,
    ) -> PyResult<PyRef<HandleValue>> {
        let tag = Tree::tag(&values(children, vm)?).map_err(|e| fix_error(e, vm))?;
        Ok(HandleValue::wrap(tag.into(), vm))
    }

    #[pyfunction]
    fn combination(vm: &VirtualMachine) -> PyRef<HandleValue> {
        HandleValue::wrap(fixwasm::backend::combination().into(), vm)
    }

    #[pyfunction]
    fn call(
        metadata: PyRef<HandleValue>,
        function: PyRef<HandleValue>,
        arguments: ArgIterable<PyRef<HandleValue>>,
        vm: &VirtualMachine,
    ) -> PyResult<PyRef<HandleValue>> {
        let arguments = values(arguments, vm)?;
        let thunk = fixwasm::call(
            metadata.0.inner(),
            function.0.inner(),
            arguments.iter().map(Value::inner),
        )
        .map_err(|e| fix_error(e, vm))?;
        Ok(HandleValue::wrap(Value::from_inner(thunk), vm))
    }

    #[pyfunction]
    fn prepare_argument(
        value: PyRef<HandleValue>,
        shallow: bool,
        vm: &VirtualMachine,
    ) -> PyResult<Option<PyRef<HandleValue>>> {
        let evaluation = if shallow {
            Evaluation::Shallow
        } else {
            Evaluation::Strict
        };
        let prepared = fixwasm::normalize_argument(value.0.inner(), evaluation)
            .map_err(|error| fix_error(error, vm))?;
        Ok(prepared.map(|value| HandleValue::wrap(Value::from_inner(value), vm)))
    }

    #[pyfunction]
    fn reapply(
        combination: PyRef<HandleValue>,
        arguments: ArgIterable<PyRef<HandleValue>>,
        vm: &VirtualMachine,
    ) -> PyResult<PyRef<HandleValue>> {
        let combination =
            Handle::<fixwasm::Object<fixwasm::Tree>>::try_from(combination.0.inner().clone())
                .map_err(|error| fix_error(error, vm))?;
        let mut prepared =
            fixwasm::arguments(&combination).map_err(|error| fix_error(error, vm))?;
        prepared.truncate(1);
        prepared.extend(values(arguments, vm)?.into_iter().map(Value::into_inner));
        let result =
            fixwasm::reapply(&combination, &prepared).map_err(|error| fix_error(error, vm))?;
        Ok(HandleValue::wrap(Value::from_inner(result), vm))
    }

    #[pyfunction]
    fn apply_remaining(
        metadata: PyRef<HandleValue>,
        result: PyRef<HandleValue>,
        arguments: ArgIterable<PyRef<HandleValue>>,
        vm: &VirtualMachine,
    ) -> PyResult<PyRef<HandleValue>> {
        let remaining: Vec<Handle<Any>> = values(arguments, vm)?
            .into_iter()
            .map(Value::into_inner)
            .collect();
        let value = fixwasm::apply_remaining(metadata.0.inner(), result.0.inner(), &remaining)
            .map_err(|e| fix_error(e, vm))?;
        Ok(HandleValue::wrap(Value::from_inner(value), vm))
    }

    #[pyfunction]
    fn stat(
        device: u64,
        filetype: u8,
        links: u64,
        size: u64,
        atime: u64,
        mtime: u64,
        ctime: u64,
        vm: &VirtualMachine,
    ) -> PyRef<HandleValue> {
        let stat = files::write_stat(&Stat {
            dev: device,
            filetype,
            nlink: links,
            size,
            atim: atime,
            mtim: mtime,
            ctim: ctime,
        });
        HandleValue::wrap(Value::from_inner(stat.into_any()), vm)
    }
}
