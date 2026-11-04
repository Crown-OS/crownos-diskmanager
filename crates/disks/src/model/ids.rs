use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

macro_rules! object_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(Arc<str>);

        impl $name {
            pub fn new(object_path: impl Into<Arc<str>>) -> Self {
                Self(object_path.into())
            }

            pub fn object_path(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

object_id!(DiskId);
object_id!(BlockId);
