use crate::object_map::{Interfaces, NO_OBJECT, ObjectMap, Properties, Value};

/// Borrowed, typed access to one object's properties with UDisks defaults
/// (empty string, zero, false) for anything missing.
#[derive(Clone, Copy)]
pub(super) struct ObjectView<'map> {
    pub path: &'map str,
    interfaces: &'map Interfaces,
}

impl<'map> ObjectView<'map> {
    pub fn find(map: &'map ObjectMap, path: &str) -> Option<Self> {
        if path == NO_OBJECT {
            return None;
        }
        map.objects
            .get_key_value(path)
            .map(|(path, interfaces)| Self { path, interfaces })
    }

    pub fn all(map: &'map ObjectMap) -> impl Iterator<Item = Self> + 'map {
        map.objects
            .iter()
            .map(|(path, interfaces)| Self { path, interfaces })
    }

    pub fn has(self, interface: &str) -> bool {
        self.interfaces.contains_key(interface)
    }

    pub fn properties(self, interface: &str) -> Option<&'map Properties> {
        self.interfaces.get(interface)
    }

    fn value(self, interface: &str, property: &str) -> Option<&'map Value> {
        self.interfaces.get(interface)?.get(property)
    }

    pub fn text(self, interface: &str, property: &str) -> &'map str {
        self.value(interface, property)
            .and_then(Value::as_str)
            .unwrap_or_default()
    }

    /// An object-path property, `None` when it is UDisks' "no object" `/`.
    pub fn object(self, interface: &str, property: &str) -> Option<&'map str> {
        Some(self.text(interface, property)).filter(|path| !path.is_empty() && *path != NO_OBJECT)
    }

    pub fn uint(self, interface: &str, property: &str) -> u64 {
        self.value(interface, property)
            .and_then(Value::as_u64)
            .unwrap_or_default()
    }

    pub fn int(self, interface: &str, property: &str) -> Option<i64> {
        self.value(interface, property).and_then(Value::as_i64)
    }

    pub fn flag(self, interface: &str, property: &str) -> bool {
        self.value(interface, property)
            .and_then(Value::as_bool)
            .unwrap_or_default()
    }

    pub fn list(self, interface: &str, property: &str) -> &'map [String] {
        self.value(interface, property)
            .and_then(Value::as_list)
            .unwrap_or_default()
    }
}
