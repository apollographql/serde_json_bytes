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
    ) -> impl Iterator<Item = Cow<'value, Value>> + 'value {
        select(&self.path, value, None).map(|(_, value)| value)
    }

    pub fn select_paths_and_values<'path: 'value, 'value>(
        &'path self,
        value: &'value Value,
    ) -> impl Iterator<Item = (String, Cow<'value, Value>)> + 'value {
        select(&self.path, value, Some(String::new()))
            .map(|(opt_path, value)| (opt_path.unwrap(), value))
    }
}

fn root_path(path: &Option<String>) -> Option<String> {
    path.as_ref().map(|s| format!("{s}$"))
}

fn index_path(path: &Option<String>, index: usize) -> Option<String> {
    path.as_ref().map(|s| format!("{s}[{index}]"))
}

fn key_path(path: &Option<String>, key: &str) -> Option<String> {
    path.as_ref().map(|s| format!("{s}.['{key}']"))
}

fn select<'value, 'path: 'value>(
    path: &'path JsonPath,
    value: &'value Value,
    selected_path: Option<String>,
) -> Box<dyn Iterator<Item = (Option<String>, Cow<'value, Value>)> + 'value> {
    println!(
        "->select {:?} from {}",
        path,
        serde_json::to_string(value).unwrap()
    );
    let res: Box<dyn Iterator<Item = (Option<String>, Cow<'value, Value>)> + 'value> = match path {
        JsonPath::Root => Box::new(once((root_path(&selected_path), Cow::Borrowed(value)))),
        JsonPath::Empty => Box::new(once((selected_path, Cow::Borrowed(value)))),
        JsonPath::Field(f) => match value {
            Value::Object(o) => match o.get(f.as_str()) {
                Some(v) => Box::new(once((
                    key_path(&selected_path, f.as_str()),
                    Cow::Borrowed(v),
                ))),
                None => Box::new(empty()),
            },
            _ => Box::new(empty()),
        },
        JsonPath::Chain(chain) => Box::new(select_chain(&chain[..], value, selected_path)),
        JsonPath::Wildcard => match value {
            Value::Object(o) => Box::new(o.into_iter().map(move |(key, value)| {
                (key_path(&selected_path, key.as_str()), Cow::Borrowed(value))
            })),
            Value::Array(a) => Box::new(a.into_iter().enumerate().map(move |(index, value)| {
                (index_path(&selected_path, index), Cow::Borrowed(value))
            })),
            _ => Box::new(empty()),
        },
        JsonPath::Descent(descent) => match value {
            Value::Array(a) => Box::new(
                a.into_iter()
                    .enumerate()
                    .flat_map(move |(index, v)| select(path, v, index_path(&selected_path, index))),
            ),
            Value::Object(o) => match o.get(descent.as_str()) {
                Some(v) => Box::new(
                    once((key_path(&selected_path, descent.as_str()), Cow::Borrowed(v))).chain(
                        o.into_iter().flat_map(move |(key, v)| {
                            select(path, v, key_path(&selected_path, key.as_str()))
                        }),
                    ),
                ),
                None => Box::new(o.into_iter().flat_map(move |(key, v)| {
                    select(path, v, key_path(&selected_path, key.as_str()))
                })),
            },
            _ => Box::new(empty()),
        },
        JsonPath::DescentW => match value {
            Value::Array(a) => {
                let selected_path2 = selected_path.clone();
                Box::new(
                    a.into_iter()
                        .enumerate()
                        .map(move |(index, v)| {
                            (index_path(&selected_path, index), Cow::Borrowed(v))
                        })
                        .chain(a.into_iter().enumerate().flat_map(move |(index, v)| {
                            select(path, v, index_path(&selected_path2, index))
                        })),
                )
            }
            Value::Object(o) => {
                let selected_path2 = selected_path.clone();

                Box::new(
                    o.into_iter()
                        .map(move |(key, v)| {
                            (key_path(&selected_path, key.as_str()), Cow::Borrowed(v))
                        })
                        .chain(o.into_iter().flat_map(move |(key, v)| {
                            select(path, v, key_path(&selected_path2, key.as_str()))
                        })),
                )
            }
            _ => Box::new(empty()),
        },
        JsonPath::Index(index) => select_index(index, value, selected_path),
        JsonPath::Current(_) => todo!(),

        JsonPath::Fn(Function::Length) => {
            if let Value::Array(a) = value {
                Box::new(once((
                    selected_path,
                    Cow::Owned(Value::Number(a.len().into())),
                )))
            } else {
                Box::new(empty())
            }
        }
    };

    Box::new(res.map(|(path, v)| {
        println!("<-selected:{path:?} {}", serde_json::to_string(&v).unwrap());
        (path, v)
    })) as Box<dyn Iterator<Item = (Option<String>, Cow<'value, Value>)> + 'value>
}

fn select_chain<'value, 'path: 'value>(
    paths: &'path [JsonPath],
    value: &'value Value,
    selected_path: Option<String>,
) -> Box<dyn Iterator<Item = (Option<String>, Cow<'value, Value>)> + 'value> {
    println!(" -> select_chain: {paths:?}");
    match paths.get(0) {
        None => Box::new(once((selected_path, Cow::Borrowed(value)))),
        Some(p) => Box::new(
            select(p, value, selected_path).flat_map(move |(prefix_path, v)| {
                match v {
                    Cow::Borrowed(v) => select_chain(&paths[1..], v, prefix_path),
                    Cow::Owned(v) => {
                        // here we need to select all values because we would get a different lifetime as a result
                        let values = select_chain(&paths[1..], &v, prefix_path)
                            .map(|(path, v)| (path, Cow::Owned(v.into_owned())))
                            .collect::<Vec<_>>();
                        Box::new(values.into_iter())
                            as Box<
                                dyn Iterator<Item = (Option<String>, Cow<'value, Value>)> + 'value,
                            >
                    }
                }
            }),
        ),
    }
}

fn select_index<'value, 'path: 'value>(
    index: &'path JsonPathIndex,
    value: &'value Value,
    selected_path: Option<String>,
) -> Box<dyn Iterator<Item = (Option<String>, Cow<'value, Value>)> + 'value> {
    match index {
        JsonPathIndex::Single(index) => {
            let index = index.as_u64().unwrap() as usize;
            Box::new(
                value
                    .as_array()
                    .and_then(|a| a.get(index))
                    .into_iter()
                    .map(move |v| (index_path(&selected_path, index), Cow::Borrowed(v))),
            )
        }
        JsonPathIndex::UnionIndex(indexes) => {
            Box::new(indexes.into_iter().flat_map(move |index| {
                let index = index.as_u64().unwrap() as usize;
                value
                    .as_array()
                    .and_then(|a| a.get(index))
                    .map(|v| (index_path(&selected_path, index), Cow::Borrowed(v)))
                    .into_iter()
            }))
        }
        JsonPathIndex::UnionKeys(keys) => Box::new(keys.into_iter().flat_map(move |key| {
            value
                .as_object()
                .and_then(|o| o.get(key.as_str()))
                .map(|v| (key_path(&selected_path, key), Cow::Borrowed(v)))
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
                    .flat_map(move |index| {
                        a.get(index as usize)
                            .map(|v| (index_path(&selected_path, index), Cow::Borrowed(v)))
                            .into_iter()
                    }),
                )
            }
        },
        JsonPathIndex::Filter(_filter) => todo!(),
    }
}

#[cfg(test)]
mod tests {

    use crate::{json, Value};

    use super::PathSelector;

    #[track_caller]
    fn test(json: &str, path: &str, expected: Vec<(String, Value)>) {
        let value: Value = serde_json::from_str(json).unwrap();
        let selector = PathSelector::new(path).unwrap();
        let selected = selector
            .select_paths_and_values(&value)
            .map(|(path, v)| (path, v.into_owned()))
            .collect::<Vec<_>>();
        assert_eq!(selected, expected);
    }

    #[macro_export]
    macro_rules! jp_v {
    (&$v:expr) =>{
        (String::new(), $v.clone())
    };

     (&$v:expr ; $s:expr) =>{
        ($s.to_string(), $v.clone())
     };

    ($(&$v:expr;$s:expr),+ $(,)?) =>{
        {
        let mut res = Vec::new();
        $(
           res.push(jp_v!(&$v ; $s));
        )+
        res
        }
    };

    ($(&$v:expr),+ $(,)?) => {
        {
        let mut res = Vec::new();
        $(
           res.push(jp_v!(&$v));
        )+
        res
        }
    };

    ($v:expr) =>{
        JsonPathValue::NewValue($v)
    };

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
        test("[1,2,3]", "$[1]", jp_v![&j1;"$[1]",]);
    }

    #[test]
    fn root_test() {
        let js: Value = serde_json::from_str(template_json()).unwrap();
        test(template_json(), "$", jp_v![&js;"$",]);
    }

    #[test]
    fn descent_test() {
        let v1 = json!("reference");
        let v2 = json!("fiction");
        test(
            template_json(),
            "$..category",
            jp_v![
                 &v1;"$.['store'].['book'][0].['category']",
                 &v2;"$.['store'].['book'][1].['category']",
                 &v2;"$.['store'].['book'][2].['category']",
                 &v2;"$.['store'].['book'][3].['category']",],
        );
        let js1 = json!(19.95);
        let js2 = json!(8.95);
        let js3 = json!(12.99);
        let js4 = json!(8.99);
        let js5 = json!(22.99);
        test(
            template_json(),
            "$.store..price",
            jp_v![
                &js1;"$.['store'].['bicycle'].['price']",
                &js2;"$.['store'].['book'][0].['price']",
                &js3;"$.['store'].['book'][1].['price']",
                &js4;"$.['store'].['book'][2].['price']",
                &js5;"$.['store'].['book'][3].['price']",
            ],
        );
        let js1 = json!("Nigel Rees");
        let js2 = json!("Evelyn Waugh");
        let js3 = json!("Herman Melville");
        let js4 = json!("J. R. R. Tolkien");
        test(
            template_json(),
            "$..author",
            jp_v![
            &js1;"$.['store'].['book'][0].['author']",
            &js2;"$.['store'].['book'][1].['author']",
            &js3;"$.['store'].['book'][2].['author']",
            &js4;"$.['store'].['book'][3].['author']",],
        );
    }

    #[test]
    fn wildcard_test() {
        let js1 = json!("reference");
        let js2 = json!("fiction");
        test(
            template_json(),
            "$..book.[*].category",
            jp_v![
                &js1;"$.['store'].['book'][0].['category']",
                &js2;"$.['store'].['book'][1].['category']",
                &js2;"$.['store'].['book'][2].['category']",
                &js2;"$.['store'].['book'][3].['category']",],
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
