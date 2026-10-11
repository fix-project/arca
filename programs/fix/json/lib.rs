#![cfg_attr(target_arch = "wasm32", no_std)]
extern crate alloc;

mod cache;

use alloc::{collections::BTreeMap, rc::Rc, string::String, vec::Vec};
use fix::{Any, Blob, Cast, Error, Handle, Object, Thunk, Tree, Value};
use serde_json::Value as Json;

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

type Expression<'a> = Rc<dyn Value<Type = Any> + 'a>;

fn expression<'a>(
    json: Json,
    bindings: &BTreeMap<String, Expression<'a>>,
    nodes: &[Expression<'a>],
) -> Result<Expression<'a>, Error> {
    Ok(match json {
        Json::String(text) => Rc::new(Blob::create(text.into_bytes()).erase()),
        Json::Array(children) => {
            let children = children
                .into_iter()
                .map(|child| expression(child, bindings, nodes))
                .collect::<Result<Vec<_>, _>>()?;
            Rc::new(Tree::create(children).erase())
        }
        Json::Object(object) => {
            assert_eq!(object.len(), 1, "expected one Fix constructor");
            let (operation, argument) = object.into_iter().next().unwrap();
            match operation.as_str() {
                "node" => nodes
                    .get(usize::try_from(argument.as_u64().expect("expected node index")).unwrap())
                    .expect("node must refer to an earlier expression")
                    .clone(),
                "binding" => bindings
                    .get(argument.as_str().expect("expected binding name"))
                    .expect("undefined binding")
                    .clone(),
                "hex" => Rc::new(
                    Blob::create(
                        hex::decode(argument.as_str().expect("expected hexadecimal string"))
                            .expect("invalid hexadecimal Blob"),
                    )
                    .erase(),
                ),
                "u8" => Rc::new(
                    Blob::create(
                        u8::try_from(argument.as_u64().expect("expected unsigned integer"))
                            .expect("integer exceeds u8")
                            .to_le_bytes(),
                    )
                    .erase(),
                ),
                "u16" => Rc::new(
                    Blob::create(
                        u16::try_from(argument.as_u64().expect("expected unsigned integer"))
                            .expect("integer exceeds u16")
                            .to_le_bytes(),
                    )
                    .erase(),
                ),
                "u32" => Rc::new(
                    Blob::create(
                        u32::try_from(argument.as_u64().expect("expected unsigned integer"))
                            .expect("integer exceeds u32")
                            .to_le_bytes(),
                    )
                    .erase(),
                ),
                "u64" => Rc::new(
                    Blob::create(
                        argument
                            .as_u64()
                            .expect("expected unsigned integer")
                            .to_le_bytes(),
                    )
                    .erase(),
                ),
                "create-ref" => {
                    let value: Cast<Object<Any>, _> =
                        expression(argument, bindings, nodes)?.try_into()?;
                    Rc::new(value.reference().erase())
                }
                "create-identification-thunk" => Rc::new(
                    expression(argument, bindings, nodes)?
                        .identification()
                        .erase(),
                ),
                "create-application-thunk" => {
                    Rc::new(expression(argument, bindings, nodes)?.application().erase())
                }
                "create-selection-thunk" => {
                    Rc::new(expression(argument, bindings, nodes)?.selection().erase())
                }
                "create-strict-encode" => {
                    let value: Cast<Thunk, _> =
                        expression(argument, bindings, nodes)?.try_into()?;
                    Rc::new(value.strict().erase())
                }
                "create-shallow-encode" => {
                    let value: Cast<Thunk, _> =
                        expression(argument, bindings, nodes)?.try_into()?;
                    Rc::new(value.shallow().erase())
                }
                _ => panic!("unknown Fix constructor: {operation}"),
            }
        }
        _ => panic!("expected a string, Tree array, or Fix constructor"),
    })
}

#[fix::apply]
pub fn apply<'a>(
    combination: &'a Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Any> + 'a, Error> {
    let arguments = combination.len()?;
    assert!(
        (2..=3).contains(&arguments),
        "expected JSON Blob and optional environment"
    );
    let source = Handle::<Object<Blob>, _>::try_from(combination.get(1))?.read()?;
    let mut bindings: BTreeMap<String, Expression<'a>> = BTreeMap::new();
    if arguments == 3 {
        let environment = Handle::<Object<Tree>, _>::try_from(combination.get(2))?;
        for index in 0..environment.len()? {
            let pair = Handle::<Object<Tree>, _>::try_from(environment.get(index))?;
            assert_eq!(pair.len()?, 2, "expected environment pair");
            let name = String::from_utf8(Handle::<Object<Blob>, _>::try_from(pair.get(0))?.read()?)
                .expect("expected UTF-8 binding name");
            assert!(
                bindings.insert(name, Rc::new(pair.get(1))).is_none(),
                "duplicate binding"
            );
        }
    }
    let mut nodes = Vec::new();
    for json in serde_json::Deserializer::from_slice(&source).into_iter::<Json>() {
        let value = expression(
            json.expect("invalid Fix JSON expression"),
            &bindings,
            &nodes,
        )?;
        nodes.push(Rc::new(cache::Node {
            index: nodes.len() as u32,
            built: core::cell::Cell::new(false),
            value,
        }) as Expression<'a>);
    }
    cache::prepare(nodes.len())?;
    Ok(nodes.pop().expect("expected a Fix expression"))
}
