use std::{
    borrow::Cow,
    iter::{empty, once},
};

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

pub struct PathSelector {
    path: JsonPath,
}

impl PathSelector {
    pub fn new(path: &str) -> Result<Self, JsonPathParserError> {
        Ok(PathSelector {
            path: parse_json_path(path)?,
        })
    }

    pub fn select<'path: 'value, 'value>(
        &'path self,
        value: &'value Value,
    ) -> Box<dyn Iterator<Item = Cow<'value, Value>> + 'value> {
        select(&self.path, value)
    }
}

fn select<'value, 'path: 'value>(
    path: &'path JsonPath,
    value: &'value Value,
) -> Box<dyn Iterator<Item = Cow<'value, Value>> + 'value> {
    match path {
        JsonPath::Root => Box::new(once(Cow::Borrowed(value))),
        JsonPath::Empty => Box::new(once(Cow::Borrowed(value))),
        JsonPath::Field(f) => match value {
            Value::Object(o) => match o.get(f.as_str()) {
                Some(v) => Box::new(once(Cow::Borrowed(v))),
                None => Box::new(empty()),
            },
            _ => Box::new(empty()),
        },
        JsonPath::Chain(chain) => Box::new(select_chain(&chain[..], value)),
        JsonPath::Wildcard => match value {
            Value::Object(o) => Box::new(o.values().into_iter().map(Cow::Borrowed)),
            Value::Array(a) => Box::new(a.into_iter().map(Cow::Borrowed)),
            _ => Box::new(empty()),
        },
        JsonPath::Descent(descent) => match value {
            Value::Array(a) => Box::new(a.into_iter().flat_map(|v| select(path, v))),
            Value::Object(o) => match o.get(descent.as_str()) {
                Some(v) => Box::new(
                    once(Cow::Borrowed(v))
                        .chain(o.values().into_iter().flat_map(|v| select(path, v))),
                ),
                None => Box::new(o.values().into_iter().flat_map(|v| select(path, v))),
            },
            _ => Box::new(empty()),
        },
        JsonPath::DescentW => match value {
            Value::Array(a) => Box::new(
                a.into_iter()
                    .map(Cow::Borrowed)
                    .chain(a.into_iter().flat_map(|v| select(path, v))),
            ),
            Value::Object(o) => Box::new(
                o.values()
                    .into_iter()
                    .map(Cow::Borrowed)
                    .chain(o.values().into_iter().flat_map(|v| select(path, v))),
            ),
            _ => Box::new(empty()),
        },

        JsonPath::Index(index) => select_index(index, value),
        JsonPath::Current(_) => todo!(),

        JsonPath::Fn(Function::Length) => {
            if let Value::Array(a) = value {
                Box::new(once(Cow::Owned(Value::Number(a.len().into()))))
            } else {
                Box::new(empty())
            }
        }
    }
}

fn select_chain<'value, 'path: 'value>(
    paths: &'path [JsonPath],
    value: &'value Value,
) -> Box<dyn Iterator<Item = Cow<'value, Value>> + 'value> {
    match paths.get(0) {
        None => Box::new(empty()),
        Some(p) => Box::new(select(p, value).flat_map(move |v| {
            match v {
                Cow::Borrowed(v) => select_chain(&paths[1..], v),
                Cow::Owned(v) => {
                    // here we need to select all values because we would get a different lifetime as a result
                    let values = select_chain(&paths[1..], &v)
                        .map(|v| v.into_owned())
                        .collect::<Vec<_>>();
                    Box::new(values.into_iter().map(Cow::Owned))
                        as Box<dyn Iterator<Item = Cow<'value, Value>> + 'value>
                }
            }
        })),
    }
}

fn select_index<'value, 'path: 'value>(
    index: &'path JsonPathIndex,
    value: &'value Value,
) -> Box<dyn Iterator<Item = Cow<'value, Value>> + 'value> {
    match index {
        JsonPathIndex::Single(index) => {
            let index = index.as_u64().unwrap() as usize;
            Box::new(
                value
                    .as_array()
                    .and_then(|a| a.get(index))
                    .into_iter()
                    .map(Cow::Borrowed),
            )
        }
        JsonPathIndex::UnionIndex(indexes) => Box::new(
            indexes
                .into_iter()
                .flat_map(|index| {
                    let index = index.as_u64().unwrap() as usize;
                    value.as_array().and_then(|a| a.get(index)).into_iter()
                })
                .map(Cow::Borrowed),
        ),
        JsonPathIndex::UnionKeys(keys) => Box::new(
            keys.into_iter()
                .flat_map(|key| {
                    value
                        .as_object()
                        .and_then(|o| o.get(key.as_str()))
                        .into_iter()
                })
                .map(Cow::Borrowed),
        ),
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
                    .flat_map(|i| a.get(i as usize).into_iter())
                    .map(Cow::Borrowed),
                )
            }
        },
        JsonPathIndex::Filter(filter) => todo!(),
    }
}
