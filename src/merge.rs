use serde_yaml::{Mapping, Value};
use std::{collections::HashMap, hash::Hash};

pub trait Merge {
    fn merge(&mut self, other: Self);
}

impl Merge for Value {
    fn merge(&mut self, other: Self) {
        match other {
            Value::Null => (),
            Value::Mapping(other) => {
                if let Value::Mapping(this) = self {
                    this.merge(other);
                }
            }
            Value::Sequence(other) => {
                if let Value::Sequence(this) = self {
                    this.extend(other);
                }
            }
            _ => *self = other,
        }
    }
}

impl Merge for Mapping {
    fn merge(&mut self, other: Self) {
        use serde_yaml::mapping::Entry;

        for (key, val) in other {
            match self.entry(key) {
                Entry::Vacant(v) => {
                    v.insert(val);
                }
                Entry::Occupied(mut v) => v.get_mut().merge(val),
            }
        }
    }
}

impl<K, V> Merge for HashMap<K, V>
where
    K: Hash + Eq,
    V: Merge,
{
    fn merge(&mut self, other: Self) {
        use std::collections::hash_map::Entry;

        for (key, val) in other {
            match self.entry(key) {
                Entry::Vacant(v) => {
                    v.insert(val);
                }
                Entry::Occupied(mut v) => v.get_mut().merge(val),
            }
        }
    }
}

impl<T> Merge for Option<T> {
    fn merge(&mut self, other: Self) {
        if let Some(other) = other {
            *self = Some(other)
        }
    }
}
