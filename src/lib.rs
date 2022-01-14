#[cfg(not(feature = "std"))]
extern crate alloc;

/// A facade around all the types we need from the `std`, `core`, and `alloc`
/// crates. This avoids elaborate import wrangling having to happen in every
/// module.
mod lib {
    mod core {
        #[cfg(not(feature = "std"))]
        pub use core::*;
        #[cfg(feature = "std")]
        pub use std::*;
    }

    pub use self::core::cell::{Cell, RefCell};
    pub use self::core::clone::{self, Clone};
    pub use self::core::convert::{self, From, Into};
    pub use self::core::default::{self, Default};
    pub use self::core::fmt::{self, Debug, Display};
    pub use self::core::hash::{self, Hash, Hasher};
    pub use self::core::iter::FusedIterator;
    pub use self::core::marker::{self, PhantomData};
    pub use self::core::ops::{Bound, RangeBounds};
    pub use self::core::result::{self, Result};
    pub use self::core::{borrow, char, cmp, iter, mem, num, ops, slice, str};

    #[cfg(not(feature = "std"))]
    pub use alloc::borrow::{Cow, ToOwned};
    #[cfg(feature = "std")]
    pub use std::borrow::{Cow, ToOwned};

    #[cfg(not(feature = "std"))]
    pub use alloc::string::{String, ToString};
    #[cfg(feature = "std")]
    pub use std::string::{String, ToString};

    #[cfg(not(feature = "std"))]
    pub use alloc::vec::{self, Vec};
    #[cfg(feature = "std")]
    pub use std::vec::{self, Vec};

    #[cfg(not(feature = "std"))]
    pub use alloc::boxed::Box;
    #[cfg(feature = "std")]
    pub use std::boxed::Box;

    #[cfg(not(feature = "std"))]
    pub use alloc::collections::{btree_map, BTreeMap};
    #[cfg(feature = "std")]
    pub use std::collections::{btree_map, BTreeMap};

    #[cfg(feature = "std")]
    pub use std::error;
}

// We only use our own error type; no need for From conversions provided by the
// standard library's try! macro. This reduces lines of LLVM IR by 4%.
macro_rules! tri {
    ($e:expr) => {
        match $e {
            crate::lib::Result::Ok(val) => val,
            crate::lib::Result::Err(err) => return crate::lib::Result::Err(err),
        }
    };
    ($e:expr,) => {
        tri!($e)
    };
}

mod bytestring;
pub mod map;
#[cfg(feature = "arbitrary_precision")]
mod number;
pub mod value;

pub use bytestring::ByteString;
pub use map::*;
pub use value::{from_value, to_value, Value};

impl From<serde_json::Value> for Value {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(b),
            serde_json::Value::Number(n) => Value::Number(n),
            serde_json::Value::String(s) => Value::String(s.into()),
            serde_json::Value::Array(v) => Value::Array(v.into_iter().map(Into::into).collect()),
            serde_json::Value::Object(o) => {
                Value::Object(o.into_iter().map(|(k, v)| (k.into(), v.into())).collect())
            }
        }
    }
}

pub use serde_json;

#[macro_export]
macro_rules! json {
    ($($json:tt)+) => {
        {
            let value: serde_json_bytes::Value = $crate::serde_json::json!($($json)+).into();
            value
        }
    };
}
