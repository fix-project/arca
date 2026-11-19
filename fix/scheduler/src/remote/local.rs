use crate::{
    abi::effect,
    executor::{self, Graph, Scheduler},
    io::Io,
    shell::{Value, capture, child},
    storage,
};
use fixwasm::{Blob, Error, Value as AsValue, tree};

pub(crate) async fn next(io: &Io) -> Result<Value, Error> {
    let (status, request) = io.request(tree![Blob::create(b"session_next")]).await?;
    if status != 0 {
        return Err(Error::WrongType);
    }
    Ok(request)
}

async fn execute(
    request: &Value,
    policy: &(impl Scheduler + ?Sized),
    graph: Option<&Graph>,
    io: &Io,
) -> Result<Value, Error> {
    let combination = child(request, 1)?;
    let mode = child(&combination, 2)?;
    let input = child(&combination, 4)?;
    let raw = AsValue::equals(&mode, Blob::create(b"load"))?;
    let todo = AsValue::equals(&mode, Blob::create(b"todo"))?;
    let serve = AsValue::equals(&mode, Blob::create(b"serve"))?;
    let result = if serve {
        let stdio = child(request, 2)?;
        let stdio = if stdio.len()? == 0 {
            None
        } else {
            Some(stdio.read_u64()?)
        };
        crate::remote::server::serve(&input, policy, graph, io, stdio).await?
    } else if todo {
        executor::todo(&input)?
    } else if raw {
        executor::load(policy, &input).await?
    } else {
        let node = executor::operation(&mode, &input)?;
        if let Some(graph) = graph {
            graph.defer(&node).await?
        } else {
            node.dispatch(policy).await?
        }
    };
    let result = if !raw && !todo && !serve && !crate::kernel::worker::returns_coupon()? {
        crate::collector::result(&result)?
    } else {
        result
    };
    if !raw && !todo && !serve && crate::kernel::worker::returns_coupon()? {
        storage::cache(&result, true)?;
    }
    Ok(result)
}

pub(crate) async fn serve(
    request: Value,
    policy: &(impl Scheduler + ?Sized),
    graph: Option<&Graph>,
    io: &Io,
) -> Result<(), Error> {
    let id = child(&request, 0)?;
    let result = execute(&request, policy, graph, io).await;
    let (status, result) = match result {
        Ok(result) => (0u64, result),
        Err(error) => {
            let message = if error == Error::MemoryUnavailable {
                alloc::string::String::from(
                    "memory admission rejected: no eligible worker can reserve the requested budget",
                )
            } else {
                alloc::format!("scheduler request failed: {error:?}")
            };
            (2, capture(Blob::create(message.as_bytes()))?)
        }
    };
    effect(b"session_reply", &[id, capture(status)?, result])?;
    Ok(())
}
