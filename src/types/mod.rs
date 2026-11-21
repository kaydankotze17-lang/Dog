use crate::ast::TypeRef;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DogType {
    Text,
    Number,
    Decimal,
    Bool,
    Void,
    List(Box<DogType>),
}

impl DogType {
    pub fn from_ref(tr: &TypeRef) -> Self {
        match tr {
            TypeRef::Text => DogType::Text,
            TypeRef::Number => DogType::Number,
            TypeRef::Decimal => DogType::Decimal,
            TypeRef::Bool => DogType::Bool,
            TypeRef::Void => DogType::Void,
            TypeRef::List(inner) => DogType::List(Box::new(Self::from_ref(inner))),
        }
    }

    pub fn label(&self) -> String {
        match self {
            DogType::Text => "text".to_string(),
            DogType::Number => "number".to_string(),
            DogType::Decimal => "decimal".to_string(),
            DogType::Bool => "bool".to_string(),
            DogType::Void => "void".to_string(),
            DogType::List(inner) => format!("list<{}>", inner.label()),
        }
    }
}

impl fmt::Display for DogType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}
