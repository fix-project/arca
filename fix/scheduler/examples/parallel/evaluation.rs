use crate::{Evidence, Request, Root, Scheduler, as_tree, fields, root};
use alloc::vec::Vec;
use fix::{
    Any, Error, Handle, Object, ThunkType, Tree, Value, ValueType,
    coupon::{Exchange, Statement},
};
use fixscheduler::shell;

fn thunk(
    object: &mut impl FnMut(&Root) -> Result<Root, Error>,
    value: &Root,
) -> Result<Root, Error> {
    let data = object(value)?;
    match value.unsound_thunk_type()? {
        ThunkType::Application => root(as_tree(&data)?.application()),
        ThunkType::Identification => {
            root(Handle::<Object<Any>, _>::try_from(data.as_ref())?.identification())
        }
        ThunkType::Selection => root(as_tree(&data)?.selection()),
    }
}

pub enum EvaluationStep {
    Complete(Evidence),
    Dependencies(Vec<Request>),
    Primitive(Request),
}

struct Context<'a, R, O> {
    proofs: &'a Scheduler,
    resolve: R,
    object: O,
    missing: Vec<Request>,
}
impl<R, O> Context<'_, R, O>
where
    R: FnMut(&Request) -> Result<Option<Evidence>, Error>,
{
    fn require(&mut self, request: &Request) -> Result<Option<Evidence>, Error> {
        let value = (self.resolve)(request)?;
        if value.is_none() {
            self.missing.push(request.clone());
        }
        Ok(value)
    }
}

#[derive(Default)]
pub struct EvaluationState(Stage);

#[derive(Default)]
enum Stage {
    #[default]
    Start,
    Tree(Vec<(Request, Option<Evidence>)>),
    EncodeForce(Request),
    EncodeEval(Evidence, Request),
    ThinkEval(Request, Statement),
    ThinkPrimitive(Evidence, Request),
    ForceThink(Request),
    ForceNext(Evidence, Request, Exchange),
}

impl EvaluationState {
    pub fn step(
        &mut self,
        request: &Request,
        proofs: &Scheduler,
        resolve: impl FnMut(&Request) -> Result<Option<Evidence>, Error>,
        object: impl FnMut(&Root) -> Result<Root, Error>,
    ) -> Result<EvaluationStep, Error> {
        let mut context = Context {
            proofs,
            resolve,
            object,
            missing: Vec::new(),
        };
        let stage = &mut self.0;
        let input = &request.input;
        loop {
            match stage {
                Stage::Start => match request.statement {
                    Statement::Eval => {
                        if input.value_type()? == ValueType::Encode {
                            *stage = Stage::EncodeForce(Request::new(
                                Statement::Force,
                                &thunk(&mut context.object, input)?,
                            )?);
                        } else if input.is_value()? {
                            return Ok(EvaluationStep::Primitive(request.clone()));
                        } else {
                            *stage = Stage::Tree(
                                fields(input)?
                                    .iter()
                                    .map(|value| Ok((Request::new(Statement::Eval, value)?, None)))
                                    .collect::<Result<_, Error>>()?,
                            );
                        }
                    }
                    Statement::Think => {
                        let data = (context.object)(input)?;
                        let statement = match input.unsound_thunk_type()? {
                            ThunkType::Identification => {
                                return context
                                    .proofs
                                    .exchange(
                                        Exchange::Think,
                                        input,
                                        &shell::reference(&data)?,
                                        &[],
                                    )
                                    .map(EvaluationStep::Complete);
                            }
                            ThunkType::Application => Statement::Apply,
                            ThunkType::Selection => Statement::Select,
                        };
                        *stage = Stage::ThinkEval(Request::new(Statement::Eval, &data)?, statement);
                    }
                    Statement::Force => {
                        *stage = Stage::ForceThink(Request::new(Statement::Think, input)?)
                    }
                    Statement::Apply | Statement::Select => {
                        return Ok(EvaluationStep::Primitive(request.clone()));
                    }
                    _ => unreachable!(),
                },
                Stage::Tree(values) => {
                    let mut complete = true;
                    for (request, evidence) in values.iter_mut() {
                        if evidence.is_none() {
                            *evidence = context.require(request)?;
                        }
                        complete &= evidence.is_some();
                    }
                    if !complete {
                        return Ok(EvaluationStep::Dependencies(core::mem::take(
                            &mut context.missing,
                        )));
                    }
                    let results: Vec<_> = values
                        .iter()
                        .map(|(_, evidence)| evidence.as_ref().unwrap().result.clone())
                        .collect();
                    let witnesses: Vec<_> = values
                        .iter()
                        .map(|(_, evidence)| evidence.as_ref().unwrap().clone())
                        .collect();
                    return context
                        .proofs
                        .exchange(
                            Exchange::Eval,
                            input,
                            &root(Tree::create(&results))?,
                            &witnesses,
                        )
                        .map(EvaluationStep::Complete);
                }
                Stage::EncodeForce(thunk) => {
                    let Some(forced) = context.require(thunk)? else {
                        return Ok(EvaluationStep::Dependencies(core::mem::take(
                            &mut context.missing,
                        )));
                    };
                    let result = if input.is_strict()? {
                        (context.object)(&forced.result)?
                    } else {
                        shell::reference(&forced.result)?
                    };
                    *stage = Stage::EncodeEval(forced, Request::new(Statement::Eval, &result)?);
                }
                Stage::EncodeEval(forced, result) => {
                    let Some(evaluated) = context.require(result)? else {
                        return Ok(EvaluationStep::Dependencies(core::mem::take(
                            &mut context.missing,
                        )));
                    };
                    let eq = context.proofs.exchange(
                        Exchange::ForceToEncode,
                        input,
                        &result.input,
                        core::slice::from_ref(forced),
                    )?;
                    let sym =
                        context
                            .proofs
                            .exchange(Exchange::Symmetry, &result.input, input, &[eq])?;
                    let result = evaluated.result.clone();
                    return context
                        .proofs
                        .exchange(Exchange::EvalEq, input, &result, &[evaluated, sym])
                        .map(EvaluationStep::Complete);
                }
                Stage::ThinkEval(data, statement) => {
                    let Some(evaluated) = context.require(data)? else {
                        return Ok(EvaluationStep::Dependencies(core::mem::take(
                            &mut context.missing,
                        )));
                    };
                    let primitive = Request::new(*statement, &evaluated.result)?;
                    *stage = Stage::ThinkPrimitive(evaluated, primitive);
                }
                Stage::ThinkPrimitive(evaluated, primitive) => {
                    let Some(primitive) = context.require(primitive)? else {
                        return Ok(EvaluationStep::Dependencies(core::mem::take(
                            &mut context.missing,
                        )));
                    };
                    let result = primitive.result.clone();
                    return context
                        .proofs
                        .exchange(
                            Exchange::Think,
                            input,
                            &result,
                            &[evaluated.clone(), primitive],
                        )
                        .map(EvaluationStep::Complete);
                }
                Stage::ForceThink(request) => {
                    let Some(thought) = context.require(request)? else {
                        return Ok(EvaluationStep::Dependencies(core::mem::take(
                            &mut context.missing,
                        )));
                    };
                    let kind = thought.result.value_type()?;
                    if matches!(kind, ValueType::Object | ValueType::Ref) {
                        return context
                            .proofs
                            .exchange(
                                Exchange::ThinkToForce,
                                input,
                                &thought.result,
                                core::slice::from_ref(&thought),
                            )
                            .map(EvaluationStep::Complete);
                    }
                    let next = thunk(&mut context.object, &thought.result)?;
                    let rule = if kind == ValueType::Thunk {
                        Exchange::ThunkEq
                    } else if thought.result.is_strict()? {
                        Exchange::ThunkStrictEq
                    } else {
                        Exchange::ThunkShallowEq
                    };
                    *stage =
                        Stage::ForceNext(thought, Request::new(Statement::Force, &next)?, rule);
                }
                Stage::ForceNext(thought, next, rule) => {
                    let Some(forced) = context.require(next)? else {
                        return Ok(EvaluationStep::Dependencies(core::mem::take(
                            &mut context.missing,
                        )));
                    };
                    let eq = context.proofs.exchange(
                        *rule,
                        input,
                        &next.input,
                        core::slice::from_ref(thought),
                    )?;
                    let sym =
                        context
                            .proofs
                            .exchange(Exchange::Symmetry, &next.input, input, &[eq])?;
                    let result = forced.result.clone();
                    return context
                        .proofs
                        .exchange(Exchange::ForceEq, input, &result, &[sym, forced])
                        .map(EvaluationStep::Complete);
                }
            }
        }
    }
}
