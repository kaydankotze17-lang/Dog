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
    Map(Box<DogType>, Box<DogType>),
    Struct(String),
    Enum(String),
}

impl DogType {
    pub fn from_ref_with<F>(tr: &TypeRef, resolve_named: &F) -> Result<Self, String>
    where
        F: Fn(&str) -> Option<DogType>,
    {
        match tr {
            TypeRef::Text => Ok(DogType::Text),
            TypeRef::Number => Ok(DogType::Number),
            TypeRef::Decimal => Ok(DogType::Decimal),
            TypeRef::Bool => Ok(DogType::Bool),
            TypeRef::Void => Ok(DogType::Void),
            TypeRef::List(inner) => Ok(DogType::List(Box::new(Self::from_ref_with(
                inner,
                resolve_named,
            )?))),
            TypeRef::Map(k, v) => Ok(DogType::Map(
                Box::new(Self::from_ref_with(k, resolve_named)?),
                Box::new(Self::from_ref_with(v, resolve_named)?),
            )),
            TypeRef::Named(name) => resolve_named(name)
                .ok_or_else(|| format!("Unknown type '{name}'.")),
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
            DogType::Map(k, v) => format!("map<{}, {}>", k.label(), v.label()),
            DogType::Struct(name) => name.clone(),
            DogType::Enum(name) => name.clone(),
        }
    }
}

impl fmt::Display for DogType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}
