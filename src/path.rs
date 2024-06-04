use std::iter::{empty, once};

use jsonpath_rust::parser::{
    errors::JsonPathParserError,
    model::{Function, JsonPath, JsonPathIndex},
    parser::parse_json_path,
};

use crate::Value;

impl Value {
    fn select<'a>(&self, path: &'a str) -> Result<Value, JsonPathParserError<'a>> {
        let path = parse_json_path(path)?;

        todo!()
    }
}

/*
struct PathSelector<'a> {
    path: JsonPath,
    value: &'a Value,
    //current_it: PathIt<'a>
}

impl<'a> PathSelector<'a> {
    fn new(path: JsonPath, value: &'a Value) -> Self {
        PathSelector { path, value }
    }

    fn select(&self) -> impl Iterator<Item = &'a Value> {
        match self.path {
            JsonPath::Root => Box::new(once(self.value)),
            JsonPath::Field(f) => todo!(),
            JsonPath::Chain(_) => todo!(),
            JsonPath::Descent(_) => todo!(),
            JsonPath::DescentW => todo!(),
            JsonPath::Index(_) => todo!(),
            JsonPath::Current(_) => todo!(),
            JsonPath::Wildcard => todo!(),
            JsonPath::Empty => todo!(),
            JsonPath::Fn(_) => todo!(),
        }
    }
}*/

fn select<'value, 'path: 'value>(
    path: &'path JsonPath,
    value: &'value Value,
) -> Box<dyn Iterator<Item = &'value Value> + 'value> {
    match path {
        JsonPath::Root => Box::new(once(value)),
        JsonPath::Empty => Box::new(once(value)),
        JsonPath::Field(f) => match value {
            Value::Object(o) => match o.get(f.as_str()) {
                Some(v) => Box::new(once(v)),
                None => Box::new(empty()),
            },
            _ => Box::new(empty()),
        },
        JsonPath::Chain(chain) => Box::new(select_chain(&chain[..], value)),
        JsonPath::Wildcard => match value {
            Value::Object(o) => Box::new(o.values().into_iter()),
            Value::Array(a) => Box::new(a.into_iter()),
            _ => Box::new(empty()),
        },
        JsonPath::Descent(descent) => match value {
            Value::Array(a) => Box::new(a.into_iter().flat_map(|v| select(path, v))),
            Value::Object(o) => match o.get(descent.as_str()) {
                Some(v) => {
                    Box::new(once(v).chain(o.values().into_iter().flat_map(|v| select(path, v))))
                }
                None => Box::new(o.values().into_iter().flat_map(|v| select(path, v))),
            },
            _ => Box::new(empty()),
        },
        JsonPath::DescentW => match value {
            Value::Array(a) => Box::new(
                a.into_iter()
                    .chain(a.into_iter().flat_map(|v| select(path, v))),
            ),
            Value::Object(o) => Box::new(
                o.values()
                    .into_iter()
                    .chain(o.values().into_iter().flat_map(|v| select(path, v))),
            ),
            _ => Box::new(empty()),
        },

        JsonPath::Index(index) => select_index(index, value),
        JsonPath::Current(_) => todo!(),

        JsonPath::Fn(Function::Length) => {
            todo!()
            /*if let Value::Array(a) = value {
                Box::new(once(Value::Number(a.len().into())))
            } else {
                Box::new(empty())
            }*/
        }
    }
}

fn select_chain<'value, 'path: 'value>(
    paths: &'path [JsonPath],
    value: &'value Value,
) -> Box<dyn Iterator<Item = &'value Value> + 'value> {
    match paths.get(0) {
        None => Box::new(empty()),
        Some(p) => Box::new(select(p, value).flat_map(|v| select_chain(&paths[1..], v))),
    }
}

fn select_index<'value, 'path: 'value>(
    index: &'path JsonPathIndex,
    value: &'value Value,
) -> Box<dyn Iterator<Item = &'value Value> + 'value> {
    match index {
        JsonPathIndex::Single(index) => {
            let index = index.as_u64().unwrap() as usize;
            Box::new(value.as_array().and_then(|a| a.get(index)).into_iter())
        }
        JsonPathIndex::UnionIndex(indexes) => Box::new(indexes.into_iter().flat_map(|index| {
            let index = index.as_u64().unwrap() as usize;
            value.as_array().and_then(|a| a.get(index)).into_iter()
        })),
        JsonPathIndex::UnionKeys(keys) => Box::new(keys.into_iter().flat_map(|key| {
            value
                .as_object()
                .and_then(|o| o.get(key.as_str()))
                .into_iter()
        })),
        JsonPathIndex::Slice(start, end, step) => match value.as_array() {
            None => Box::new(empty()),
            Some(a) => {
                let mut index = None;
                Box::new(
                    std::iter::from_fn(move || {
                        let new_index = match index.take() {
                            None => *start as usize,

                            Some(i) => i + step,
                        };

                        if new_index >= a.len() || new_index > *end as usize {
                            None
                        } else {
                            index = Some(new_index);
                            Some(new_index)
                        }
                    })
                    .flat_map(|i| a.get(i as usize).into_iter()),
                )
            }
        },
        JsonPathIndex::Filter(filter) => todo!(),
    }
}

