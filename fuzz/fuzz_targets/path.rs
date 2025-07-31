#![no_main]

use std::str::FromStr;

use jsonpath_rust::JsonPathFinder;
use lazy_static::lazy_static;
use libfuzzer_sys::{fuzz_target, Corpus};

lazy_static! {
    static ref JSON: serde_json::Value = serde_json::from_str(template_json()).unwrap();
    static ref JSON_BYTES: serde_json_bytes::Value = serde_json::from_str(template_json()).unwrap();
}

fuzz_target!(|path: String| -> Corpus {
    let json_selector = match jsonpath_rust::JsonPathInst::from_str(&path) {
        Ok(p) => p,
        Err(_) => return Corpus::Reject,
    };

    let json_bytes_selector = serde_json_bytes::path::JsonPathInst::new(&path).unwrap();

    let json_finder = JsonPathFinder::new(Box::new(JSON.clone()), Box::new(json_selector));

    let json_selected = match json_finder.find() {
        serde_json::Value::Array(mut v) => {
            if v.len() == 1 {
                v.pop().unwrap()
            } else {
                serde_json::Value::Array(v)
            }
        }
        value => value,
    };

    let json_bytes = json_bytes_selector.find(&JSON_BYTES);

    let json_s = serde_json::to_string(&json_selected).unwrap();
    let json_bytes_s = serde_json::to_string(&json_bytes).unwrap();

    assert_eq!(json_s, json_bytes_s, "from path: {path}");

    Corpus::Keep
});

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
