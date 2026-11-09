use crate::{Backend, Evidence, Request, Root, as_tree, fields, root, shell};
use alloc::vec::Vec;
use fix::{
    Error, ThunkType, Tree, Value, ValueType,
    coupon::{Exchange, Statement},
};

pub fn evaluate(backend: &impl Backend, request: &Request) -> Result<Evidence, Error> {
    if let Some(value) = backend.cached(request)? {
        return Ok(value);
    }
    let input = &request.input;
    let result = match request.statement {
        Statement::Apply => backend.apply(input)?,
        Statement::Select => backend.select(input)?,
        Statement::Eval => {
            if input.value_type()? == ValueType::Encode {
                let thunk = thunk(backend, input)?;
                let forced = evaluate(backend, &Request::new(Statement::Force, &thunk)?)?;
                let data = if input.is_strict()? {
                    backend.object(&forced.result)?
                } else {
                    shell::reference(&forced.result)?
                };
                let evaluated = evaluate(backend, &Request::new(Statement::Eval, &data)?)?;
                let eq = backend.exchange(Exchange::ForceToEncode, input, &data, &[forced])?;
                let sym = backend.exchange(Exchange::Symmetry, &data, input, &[eq])?;
                backend.exchange(
                    Exchange::EvalEq,
                    input,
                    &evaluated.result,
                    &[evaluated.clone(), sym],
                )?
            } else if input.is_value()? {
                backend.eval(input)?
            } else {
                let premises = fields(input)?
                    .iter()
                    .map(|child| evaluate(backend, &Request::new(Statement::Eval, child)?))
                    .collect::<Result<Vec<_>, Error>>()?;
                let result = root(Tree::from_iter(
                    premises.iter().map(|evidence| &evidence.result),
                ))?;
                backend.exchange(Exchange::Eval, input, &result, &premises)?
            }
        }
        Statement::Think => {
            let data = backend.object(input)?;
            match input.unsound_thunk_type()? {
                ThunkType::Identification => {
                    backend.exchange(Exchange::Think, input, &shell::reference(&data)?, &[])?
                }
                kind => {
                    let evaluated = evaluate(backend, &Request::new(Statement::Eval, &data)?)?;
                    let statement = match kind {
                        ThunkType::Application => Statement::Apply,
                        ThunkType::Selection => Statement::Select,
                        ThunkType::Identification => unreachable!(),
                    };
                    let applied = evaluate(backend, &Request::new(statement, &evaluated.result)?)?;
                    backend.exchange(
                        Exchange::Think,
                        input,
                        &applied.result,
                        &[evaluated, applied.clone()],
                    )?
                }
            }
        }
        Statement::Force => {
            let thought = evaluate(backend, &Request::new(Statement::Think, input)?)?;
            let kind = thought.result.value_type()?;
            if matches!(kind, ValueType::Object | ValueType::Ref) {
                backend.exchange(
                    Exchange::ThinkToForce,
                    input,
                    &thought.result,
                    &[thought.clone()],
                )?
            } else {
                let next = thunk(backend, &thought.result)?;
                let forced = evaluate(backend, &Request::new(Statement::Force, &next)?)?;
                let rule = if kind == ValueType::Thunk {
                    Exchange::ThunkEq
                } else if thought.result.is_strict()? {
                    Exchange::ThunkStrictEq
                } else {
                    Exchange::ThunkShallowEq
                };
                let eq = backend.exchange(rule, input, &next, &[thought])?;
                let sym = backend.exchange(Exchange::Symmetry, &next, input, &[eq])?;
                backend.exchange(
                    Exchange::ForceEq,
                    input,
                    &forced.result,
                    &[sym, forced.clone()],
                )?
            }
        }
        _ => return Err(Error::WrongType),
    };
    backend.record(request, &result)?;
    Ok(result)
}

fn thunk(backend: &impl Backend, value: &Root) -> Result<Root, Error> {
    let data = backend.object(value)?;
    match value.unsound_thunk_type()? {
        ThunkType::Application => root(as_tree(&data)?.application()),
        ThunkType::Identification => root(data.identification()),
        ThunkType::Selection => root(as_tree(&data)?.selection()),
    }
}
