use serde::{Deserialize, Serialize};

/// An owned D-Bus property value, reduced to the shapes UDisks2 uses.
///
/// Byte strings (`ay`, e.g. `Block.Device`) are stored as text without the
/// trailing NUL; arrays of byte strings (`aay`, e.g. `Filesystem.MountPoints`)
/// and of strings or object paths (`as`, `ao`) are stored as `List`.
/// Untagged so fixtures read like plain JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Uint(u64),
    Int(i64),
    Double(f64),
    Text(String),
    List(Vec<String>),
}

impl Value {
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Uint(value) => Some(*value),
            Self::Int(value) => u64::try_from(*value).ok(),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            Self::Uint(value) => i64::try_from(*value).ok(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Double(value) => Some(*value),
            Self::Uint(value) => Some(*value as f64),
            Self::Int(value) => Some(*value as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            Self::List(values) => Some(values),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_shapes_map_to_the_expected_variants() {
        let parsed: Vec<Value> =
            serde_json::from_str(r#"[true, 5, -1, 313.5, "/dev/sda", ["/", "/home"]]"#).unwrap();
        assert_eq!(
            parsed,
            [
                Value::Bool(true),
                Value::Uint(5),
                Value::Int(-1),
                Value::Double(313.5),
                Value::Text("/dev/sda".into()),
                Value::List(vec!["/".into(), "/home".into()]),
            ]
        );
    }

    #[test]
    fn whole_doubles_stay_doubles() {
        let json = serde_json::to_string(&Value::Double(300.0)).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&json).unwrap(),
            Value::Double(300.0)
        );
    }

    #[test]
    fn integers_convert_between_signedness() {
        assert_eq!(Value::Int(-1).as_u64(), None);
        assert_eq!(Value::Int(7).as_u64(), Some(7));
        assert_eq!(Value::Uint(7).as_f64(), Some(7.0));
    }
}
