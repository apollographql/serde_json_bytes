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
    println!(
        "->select {:?} from {}",
        path,
        serde_json::to_string(value).unwrap()
    );
    let res: Box<dyn Iterator<Item = Cow<'value, Value>> + 'value> = match path {
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
    };

    Box::new(res.map(|v| {
        println!("<-selected: {}", serde_json::to_string(&v).unwrap());
        v
    })) as Box<dyn Iterator<Item = Cow<'value, Value>> + 'value>
}

fn select_chain<'value, 'path: 'value>(
    paths: &'path [JsonPath],
    value: &'value Value,
) -> Box<dyn Iterator<Item = Cow<'value, Value>> + 'value> {
    println!(" -> select_chain: {paths:?}");
    match paths.get(0) {
        None => Box::new(once(Cow::Borrowed(value))),
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

#[cfg(test)]
mod tests {
    /*use crate::JsonPathQuery;
    use crate::JsonPathValue::{NoValue, Slice};
    use crate::{jp_v, JsonPathFinder, JsonPathInst, JsonPathValue};
    */
    //use serde_json::{json, Value};
    use std::ops::Deref;
    use std::str::FromStr;

    use crate::{json, Value};

    use super::PathSelector;

    #[track_caller]
    fn test(json: &str, path: &str, expected: Vec<Value>) {
        let value: Value = serde_json::from_str(json).unwrap();
        let selector = PathSelector::new(path).unwrap();
        let selected = selector
            .select(&value)
            .map(|v| {
                println!("got: {v:?})");
                v.into_owned()
            })
            .collect::<Vec<_>>();
        assert_eq!(selected, expected);
    }

    fn template_json<'a>() -> &'a str {
        r#" {"store": { "book": [
             {
                 "category": "reference",
                 "author": "Nigel Rees",
                 "title": "Sayings of the Century",
                 "price": 8.95
             },
             {
                 "category": "fiction",
                 "author": "Evelyn Waugh",
                 "title": "Sword of Honour",
                 "price": 12.99
             },
             {
                 "category": "fiction",
                 "author": "Herman Melville",
                 "title": "Moby Dick",
                 "isbn": "0-553-21311-3",
                 "price": 8.99
             },
             {
                 "category": "fiction",
                 "author": "J. R. R. Tolkien",
                 "title": "The Lord of the Rings",
                 "isbn": "0-395-19395-8",
                 "price": 22.99
             }
         ],
         "bicycle": {
             "color": "red",
             "price": 19.95
         }
     },
     "array":[0,1,2,3,4,5,6,7,8,9],
     "orders":[
         {
             "ref":[1,2,3],
             "id":1,
             "filled": true
         },
         {
             "ref":[4,5,6],
             "id":2,
             "filled": false
         },
         {
             "ref":[7,8,9],
             "id":3,
             "filled": null
         }
      ],
     "expensive": 10 }"#
    }

    #[test]
    fn simple_test() {
        let j1 = json!(2);
        test("[1,2,3]", "$[1]", vec![j1]);
    }

    #[test]
    fn root_test() {
        let js = serde_json::from_str(template_json()).unwrap();
        test(template_json(), "$", vec![js]);
    }

    #[test]
    fn descent_test() {
        let v1 = json!("reference");
        let v2 = json!("fiction");
        test(
            template_json(),
            "$..category",
            vec![v1, v2.clone(), v2.clone(), v2],
        );
        let js1 = json!(19.95);
        let js2 = json!(8.95);
        let js3 = json!(12.99);
        let js4 = json!(8.99);
        let js5 = json!(22.99);
        test(
            template_json(),
            "$.store..price",
            vec![js1, js2, js3, js4, js5],
        );
        let js1 = json!("Nigel Rees");
        let js2 = json!("Evelyn Waugh");
        let js3 = json!("Herman Melville");
        let js4 = json!("J. R. R. Tolkien");
        test(template_json(), "$..author", vec![js1, js2, js3, js4]);
    }

    #[test]
    fn wildcard_test() {
        let js1 = json!("reference");
        let js2 = json!("fiction");
        test(
            template_json(),
            "$..book.[*].category",
            vec![js1, js2.clone(), js2.clone(), js2.clone()],
        );
        let js1 = json!("Nigel Rees");
        let js2 = json!("Evelyn Waugh");
        let js3 = json!("Herman Melville");
        let js4 = json!("J. R. R. Tolkien");
        test(
            template_json(),
            "$.store.book[*].author",
            vec![js1, js2, js3, js4],
        );
    }
}
