use std::{
    borrow::Cow,
    iter::{empty, once},
    str::FromStr,
};

use jsonpath_rust::parser::{
    errors::JsonPathParserError,
    model::{FilterExpression, FilterSign, Function, JsonPath, JsonPathIndex, Operand},
    parser::parse_json_path,
};
use regex::Regex;

use crate::Value;

#[derive(Clone)]
pub struct JsonPathInst {
    path: JsonPath,
}

impl JsonPathInst {
    pub fn new(path: &str) -> Result<Self, JsonPathParserError> {
        Ok(JsonPathInst {
            path: parse_json_path(path)?,
        })
    }

    pub fn select<'path: 'value, 'value>(
        &'path self,
        value: &'value Value,
    ) -> impl Iterator<Item = Cow<'value, Value>> + 'value {
        select(&self.path, value, value, None).map(|(_, value)| value)
    }

    pub fn select_paths_and_values<'path: 'value, 'value>(
        &'path self,
        value: &'value Value,
    ) -> impl Iterator<Item = (String, Cow<'value, Value>)> + 'value {
        select(&self.path, value, value, Some(String::new()))
            .map(|(opt_path, value)| (opt_path.unwrap(), value))
    }

    pub fn find<'path: 'value, 'value>(&'path self, value: &'value Value) -> Value {
        let mut v: Vec<_> = select(&self.path, value, value, None)
            .map(|(_, value)| value.into_owned())
            .collect();
        if v.len() == 0 {
            Value::Null
        } else if v.len() == 1 {
            v.pop()
                .expect("already checked the array had a length of 1; qed")
        } else {
            Value::Array(v)
        }
    }
}

impl FromStr for JsonPathInst {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(JsonPathInst {
            path: s.try_into()?,
        })
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
    root: &'path Value,
    value: &'value Value,
    selected_path: Option<String>,
) -> Box<dyn Iterator<Item = (Option<String>, Cow<'value, Value>)> + 'value> {
    println!(
        "->select {:?} from {}",
        path,
        serde_json::to_string(value).unwrap()
    );
    let res: Box<dyn Iterator<Item = (Option<String>, Cow<'value, Value>)> + 'value> = match path {
        JsonPath::Root => Box::new(once((root_path(&selected_path), Cow::Borrowed(root)))),
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
        JsonPath::Chain(chain) => Box::new(select_chain(&chain[..], root, value, selected_path)),
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
            Value::Array(a) => Box::new(a.into_iter().enumerate().flat_map(move |(index, v)| {
                select(path, root, v, index_path(&selected_path, index))
            })),
            Value::Object(o) => match o.get(descent.as_str()) {
                Some(v) => Box::new(
                    once((key_path(&selected_path, descent.as_str()), Cow::Borrowed(v))).chain(
                        o.into_iter().flat_map(move |(key, v)| {
                            select(path, root, v, key_path(&selected_path, key.as_str()))
                        }),
                    ),
                ),
                None => Box::new(o.into_iter().flat_map(move |(key, v)| {
                    select(path, root, v, key_path(&selected_path, key.as_str()))
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
                            select(path, root, v, index_path(&selected_path2, index))
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
                            select(path, root, v, key_path(&selected_path2, key.as_str()))
                        })),
                )
            }
            _ => Box::new(empty()),
        },
        JsonPath::Index(index) => {
            Box::new(select_index(index, root, value, selected_path).map(|v| {
                println!("select_index returning {v:?}");
                v
            }))
        }
        JsonPath::Current(current) => match current.as_ref() {
            JsonPath::Empty => Box::new(once((root_path(&selected_path), Cow::Borrowed(value)))),
            path => select(path, root, value, selected_path),
        },

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
        println!(
            "<-selected:{path:?} => {}",
            serde_json::to_string(&v).unwrap()
        );
        (path, v)
    })) as Box<dyn Iterator<Item = (Option<String>, Cow<'value, Value>)> + 'value>
}

fn select_chain<'value, 'path: 'value>(
    paths: &'path [JsonPath],
    root: &'path Value,

    value: &'value Value,
    selected_path: Option<String>,
) -> Box<dyn Iterator<Item = (Option<String>, Cow<'value, Value>)> + 'value> {
    println!(" -> select_chain: {paths:?}");
    match paths.get(0) {
        None => Box::new(once((selected_path, Cow::Borrowed(value)))),
        Some(p) => Box::new(select(p, root, value, selected_path).flat_map(
            move |(prefix_path, v)| {
                match v {
                    Cow::Borrowed(v) => select_chain(&paths[1..], root, v, prefix_path),
                    Cow::Owned(v) => {
                        // here we need to select all values because we would get a different lifetime as a result
                        let values = select_chain(&paths[1..], root, &v, prefix_path)
                            .map(|(path, v)| (path, Cow::Owned(v.into_owned())))
                            .collect::<Vec<_>>();
                        Box::new(values.into_iter())
                            as Box<
                                dyn Iterator<Item = (Option<String>, Cow<'value, Value>)> + 'value,
                            >
                    }
                }
            },
        )),
    }
}

fn select_index<'value, 'path: 'value>(
    index: &'path JsonPathIndex,
    root: &'path Value,
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
        JsonPathIndex::Filter(filter) => match value {
            Value::Array(a) => Box::new(a.into_iter().enumerate().filter_map(move |(index, v)| {
                if select_filter(filter, root, v) {
                    println!(
                        "TRUE => returning at path {selected_path:?}[{index}]: {}",
                        serde_json::to_string(&v).unwrap()
                    );
                    Some((index_path(&selected_path, index), Cow::Borrowed(v)))
                } else {
                    println!("FALSE => returning none at path {selected_path:?}",);
                    None
                }
            })),
            value => {
                if select_filter(filter, root, value) {
                    println!(
                        "TRUE => returning at path {selected_path:?}: {}",
                        serde_json::to_string(&value).unwrap()
                    );

                    Box::new(once((selected_path, Cow::Borrowed(value))))
                } else {
                    println!("FALSE => returning nothing at path {selected_path:?}",);
                    Box::new(empty())
                }
            }
        },
    }
}

fn select_filter<'value, 'path: 'value>(
    filter: &'path FilterExpression,
    root: &'path Value,

    value: &'value Value,
) -> bool {
    let res = match filter {
        FilterExpression::And(left, right) => {
            select_filter(left, root, value) && select_filter(right, root, value)
        }
        FilterExpression::Or(left, right) => {
            select_filter(left, root, value) || select_filter(right, root, value)
        }
        FilterExpression::Not(expr) => !select_filter(expr, root, value),
        FilterExpression::Atom(left, op, right) => {
            let left = select_operand(left, root, value);
            let right = select_operand(right, root, value);
            match op {
                FilterSign::Equal => left == right,
                FilterSign::Unequal => left != right,
                FilterSign::Less => less(&left, &right),
                FilterSign::Greater => less(&right, &left),
                FilterSign::LeOrEq => less(&left, &right) || (left == right),
                FilterSign::GrOrEq => less(&right, &left) || (left == right),
                FilterSign::Regex => regex(&left, &right),
                FilterSign::In => inside(&left, &right),
                FilterSign::Nin => !inside(&left, &right),
                FilterSign::Size => size(&left, &right),
                FilterSign::NoneOf => !any_of(&left, &right),
                FilterSign::AnyOf => any_of(&left, &right),
                FilterSign::SubSetOf => sub_set_of(&left, &right),
                FilterSign::Exists => !left.is_empty(),
            }
        }
    };

    println!(
        "filter {filter:?} on {} => {res}",
        serde_json::to_string(&value).unwrap()
    );

    res
}

fn select_operand<'value, 'path: 'value>(
    operand: &'path Operand,
    root: &'path Value,

    value: &'value Value,
) -> Vec<Cow<'value, Value>> {
    println!("-operand-");
    match operand {
        Operand::Static(s) => vec![Cow::Owned(s.to_owned().into())],
        Operand::Dynamic(path) => select(path, root, value, None).map(|t| t.1).collect(),
    }
}

pub fn less<'value>(left: &Vec<Cow<'value, Value>>, right: &Vec<Cow<'value, Value>>) -> bool {
    if left.len() == 1 && right.len() == 1 {
        match (
            left.get(0).map(|v| v.as_ref()),
            right.get(0).map(|v| v.as_ref()),
        ) {
            (Some(Value::Number(l)), Some(Value::Number(r))) => l
                .as_f64()
                .and_then(|v1| r.as_f64().map(|v2| v1 < v2))
                .unwrap_or(false),
            _ => false,
        }
    } else {
        false
    }
}

pub fn inside<'value>(left: &Vec<Cow<'value, Value>>, right: &Vec<Cow<'value, Value>>) -> bool {
    if left.is_empty() {
        return false;
    }

    match right.get(0).map(|v| v.as_ref()) {
        Some(Value::Array(elems)) => {
            for el in left.iter() {
                if elems.contains(el) {
                    return true;
                }
            }
            false
        }
        Some(Value::Object(elems)) => {
            for el in left.iter().map(|v| v.as_ref()) {
                for r in elems.values() {
                    if el.eq(r) {
                        return true;
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// compare sizes of json elements
/// The method expects to get a number on the right side and array or string or object on the left
/// where the number of characters, elements or fields will be compared respectively.
pub fn size<'value>(left: &Vec<Cow<'value, Value>>, right: &Vec<Cow<'value, Value>>) -> bool {
    if let Some(Value::Number(n)) = right.get(0).map(|v| v.as_ref()) {
        if let Some(sz) = n.as_f64() {
            for el in left.iter().map(|v| v.as_ref()) {
                match el {
                    Value::String(v) if v.as_str().len() == sz as usize => true,
                    Value::Array(elems) if elems.len() == sz as usize => true,
                    Value::Object(fields) if fields.len() == sz as usize => true,
                    _ => return false,
                };
            }
            return true;
        }
    }
    false
}

pub fn sub_set_of<'value>(left: &Vec<Cow<'value, Value>>, right: &Vec<Cow<'value, Value>>) -> bool {
    if left.is_empty() {
        return true;
    }
    if right.is_empty() {
        return false;
    }

    if let Some(elems) = left.first().and_then(|e| e.as_array()) {
        if let Some(Value::Array(right_elems)) = right.get(0).map(|v| v.as_ref()) {
            if right_elems.is_empty() {
                return false;
            }

            for el in elems {
                let mut res = false;

                for r in right_elems.iter() {
                    if el.eq(r) {
                        res = true
                    }
                }
                if !res {
                    return false;
                }
            }
            return true;
        }
    }
    false
}

pub fn any_of<'value>(left: &Vec<Cow<'value, Value>>, right: &Vec<Cow<'value, Value>>) -> bool {
    if left.is_empty() {
        return true;
    }
    if right.is_empty() {
        return false;
    }

    if let Some(Value::Array(elems)) = right.get(0).map(|v| v.as_ref()) {
        if elems.is_empty() {
            return false;
        }

        for el in left.iter().map(|v| v.as_ref()) {
            if let Some(left_elems) = el.as_array() {
                for l in left_elems.iter() {
                    for r in elems.iter() {
                        if l.eq(r) {
                            return true;
                        }
                    }
                }
            } else {
                for r in elems.iter() {
                    if el.eq(r) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

pub fn regex<'value>(left: &Vec<Cow<'value, Value>>, right: &Vec<Cow<'value, Value>>) -> bool {
    if left.is_empty() || right.is_empty() {
        return false;
    }

    match right.get(0).map(|v| v.as_ref()) {
        Some(Value::String(str)) => {
            if let Ok(regex) = Regex::new(str.as_str()) {
                for el in left.iter() {
                    if let Some(v) = el.as_str() {
                        if regex.is_match(v) {
                            return true;
                        }
                    }
                }
            }
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {

    use crate::{json, Value};

    use super::JsonPathInst;

    #[track_caller]
    fn test(json: &str, path: &str, expected: Vec<(String, Value)>) {
        let value: Value = serde_json::from_str(json).unwrap();
        let selector = JsonPathInst::new(path).unwrap();
        let selected = selector
            .select_paths_and_values(&value)
            .map(|(path, v)| (path, v.into_owned()))
            .collect::<Vec<_>>();
        println!("Testing path {path}:");
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
            jp_v![
                &js1;"$.['store'].['book'][0].['author']",
                &js2;"$.['store'].['book'][1].['author']",
                &js3;"$.['store'].['book'][2].['author']",
                &js4;"$.['store'].['book'][3].['author']",],
        );
    }

    #[test]
    fn descendent_wildcard_test() {
        let js1 = json!("Moby Dick");
        let js2 = json!("The Lord of the Rings");
        test(
            template_json(),
            "$..*.[?(@.isbn)].title",
            jp_v![
                &js1;"$.['store'].['book'][2].['title']",
                &js2;"$.['store'].['book'][3].['title']",
                &js1;"$.['store'].['book'][2].['title']",
                &js2;"$.['store'].['book'][3].['title']"],
        );
    }

    #[test]
    fn field_test() {
        let value = json!({"active":1});
        test(
            r#"{"field":{"field":[{"active":1},{"passive":1}]}}"#,
            "$.field.field[?(@.active)]",
            jp_v![&value;"$.['field'].['field'][0]",],
        );
    }

    #[test]
    fn index_index_test() {
        let value = json!("0-553-21311-3");
        test(
            template_json(),
            "$..book[2].isbn",
            jp_v![&value;"$.['store'].['book'][2].['isbn']",],
        );
    }

    #[test]
    fn index_unit_index_test() {
        let value = json!("0-553-21311-3");
        test(
            template_json(),
            "$..book[2,4].isbn",
            jp_v![&value;"$.['store'].['book'][2].['isbn']",],
        );
        let value1 = json!("0-395-19395-8");
        test(
            template_json(),
            "$..book[2,3].isbn",
            jp_v![&value;"$.['store'].['book'][2].['isbn']", &value1;"$.['store'].['book'][3].['isbn']",],
        );
    }

    #[test]
    fn index_unit_keys_test() {
        let js1 = json!("Moby Dick");
        let js2 = json!(8.99);
        let js3 = json!("The Lord of the Rings");
        let js4 = json!(22.99);
        test(
            template_json(),
            "$..book[2,3]['title','price']",
            jp_v![
                &js1;"$.['store'].['book'][2].['title']",
                &js2;"$.['store'].['book'][2].['price']",
                &js3;"$.['store'].['book'][3].['title']",
                &js4;"$.['store'].['book'][3].['price']",],
        );
    }

    #[test]
    fn index_slice_test() {
        let i0 = "$.['array'][0]";
        let i1 = "$.['array'][1]";
        let i2 = "$.['array'][2]";
        let i3 = "$.['array'][3]";
        let i4 = "$.['array'][4]";
        let i5 = "$.['array'][5]";
        let i6 = "$.['array'][6]";
        let i7 = "$.['array'][7]";
        let i8 = "$.['array'][8]";
        let i9 = "$.['array'][9]";

        let j0 = json!(0);
        let j1 = json!(1);
        let j2 = json!(2);
        let j3 = json!(3);
        let j4 = json!(4);
        let j5 = json!(5);
        let j6 = json!(6);
        let j7 = json!(7);
        let j8 = json!(8);
        let j9 = json!(9);
        test(
            template_json(),
            "$.array[:]",
            jp_v![
                &j0;&i0,
                &j1;&i1,
                &j2;&i2,
                &j3;&i3,
                &j4;&i4,
                &j5;&i5,
                &j6;&i6,
                &j7;&i7,
                &j8;&i8,
                &j9;&i9,],
        );
        test(template_json(), "$.array[1:4:2]", jp_v![&j1;&i1, &j3;&i3,]);
        test(
            template_json(),
            "$.array[::3]",
            jp_v![&j0;&i0, &j3;&i3, &j6;&i6, &j9;&i9,],
        );
        test(template_json(), "$.array[-1:]", jp_v![&j9;&i9,]);
        test(template_json(), "$.array[-2:-1]", jp_v![&j8;&i8,]);
    }

    #[test]
    fn index_filter_test() {
        let moby = json!("Moby Dick");
        let rings = json!("The Lord of the Rings");
        test(
            template_json(),
            "$..book[?(@.isbn)].title",
            jp_v![
                &moby;"$.['store'].['book'][2].['title']",
                &rings;"$.['store'].['book'][3].['title']",],
        );
        let sword = json!("Sword of Honour");
        test(
            template_json(),
            "$..book[?(@.price != 8.95)].title",
            jp_v![
                &sword;"$.['store'].['book'][1].['title']",
                &moby;"$.['store'].['book'][2].['title']",
                &rings;"$.['store'].['book'][3].['title']",],
        );
        let sayings = json!("Sayings of the Century");
        test(
            template_json(),
            "$..book[?(@.price == 8.95)].title",
            jp_v![&sayings;"$.['store'].['book'][0].['title']",],
        );
        let js895 = json!(8.95);
        test(
            template_json(),
            "$..book[?(@.author ~= '.*Rees')].price",
            jp_v![&js895;"$.['store'].['book'][0].['price']",],
        );
        let js12 = json!(12.99);
        let js899 = json!(8.99);
        let js2299 = json!(22.99);
        test(
            template_json(),
            "$..book[?(@.price >= 8.99)].price",
            jp_v![
                &js12;"$.['store'].['book'][1].['price']",
                &js899;"$.['store'].['book'][2].['price']",
                &js2299;"$.['store'].['book'][3].['price']",
            ],
        );
        test(
            template_json(),
            "$..book[?(@.price > 8.99)].price",
            jp_v![
                &js12;"$.['store'].['book'][1].['price']",
                &js2299;"$.['store'].['book'][3].['price']",],
        );
        test(
            template_json(),
            "$..book[?(@.price < 8.99)].price",
            jp_v![&js895;"$.['store'].['book'][0].['price']",],
        );
        test(
            template_json(),
            "$..book[?(@.price <= 8.99)].price",
            jp_v![
                &js895;"$.['store'].['book'][0].['price']",
                &js899;"$.['store'].['book'][2].['price']",
            ],
        );
        test(
            template_json(),
            "$..book[?(@.price <= $.expensive)].price",
            jp_v![
                &js895;"$.['store'].['book'][0].['price']",
                &js899;"$.['store'].['book'][2].['price']",
            ],
        );
        test(
            template_json(),
            "$..book[?(@.price >= $.expensive)].price",
            jp_v![
                &js12;"$.['store'].['book'][1].['price']",
                &js2299;"$.['store'].['book'][3].['price']",
            ],
        );
        test(
            template_json(),
            "$..book[?(@.title in ['Moby Dick','Shmoby Dick','Big Dick','Dicks'])].price",
            jp_v![&js899;"$.['store'].['book'][2].['price']",],
        );
        test(
            template_json(),
            "$..book[?(@.title nin ['Moby Dick','Shmoby Dick','Big Dick','Dicks'])].title",
            jp_v![
                &sayings;"$.['store'].['book'][0].['title']",
                &sword;"$.['store'].['book'][1].['title']",
                &rings;"$.['store'].['book'][3].['title']",],
        );
        test(
            template_json(),
            "$..book[?(@.author size 10)].title",
            jp_v![&sayings;"$.['store'].['book'][0].['title']",],
        );
        let filled_true = json!(1);
        test(
            template_json(),
            "$.orders[?(@.filled == true)].id",
            jp_v![&filled_true;"$.['orders'][0].['id']",],
        );
        let filled_null = json!(3);
        test(
            template_json(),
            "$.orders[?(@.filled == null)].id",
            jp_v![&filled_null;"$.['orders'][2].['id']",],
        );
    }

    #[test]
    fn index_filter_sets_test() {
        let j1 = json!(1);
        test(
            template_json(),
            "$.orders[?(@.ref subsetOf [1,2,3,4])].id",
            jp_v![&j1;"$.['orders'][0].['id']",],
        );
        let j2 = json!(2);
        test(
            template_json(),
            "$.orders[?(@.ref anyOf [1,4])].id",
            jp_v![&j1;"$.['orders'][0].['id']", &j2;"$.['orders'][1].['id']",],
        );
        let j3 = json!(3);
        test(
            template_json(),
            "$.orders[?(@.ref noneOf [3,6])].id",
            jp_v![&j3;"$.['orders'][2].['id']",],
        );
    }
}
