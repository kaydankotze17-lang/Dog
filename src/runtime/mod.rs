use crate::ast::BindingKind;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    Number(i64),
    Decimal(f64),
    Bool(bool),
    List(Vec<Value>),
    Void,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Text(s) => f.write_str(s),
            Value::Number(n) => write!(f, "{n}"),
            Value::Decimal(d) => write!(f, "{d}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::List(items) => {
                f.write_str("[")?;
                for (idx, item) in items.iter().enumerate() {
                    if idx > 0 {
                        f.write_str(", ")?;
                    }
                    match item {
                        Value::Text(s) => write!(f, "\"{s}\"")?,
                        other => write!(f, "{other}")?,
                    }
                }
                f.write_str("]")
            }
            Value::Void => Ok(()),
        }
    }
}

#[derive(Debug, Clone)]
struct Slot {
    kind: BindingKind,
    value: Value,
}

#[derive(Debug, Default)]
pub struct Environment {
    scopes: Vec<HashMap<String, Slot>>,
}

impl Environment {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn define(&mut self, name: String, kind: BindingKind, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, Slot { kind, value });
        }
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(slot) = scope.get(name) {
                return Some(&slot.value);
            }
        }
        None
    }

    pub fn get_mut_var(&mut self, name: &str) -> Result<&mut Value, &'static str> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(slot) = scope.get_mut(name) {
                if slot.kind != BindingKind::Var {
                    return Err("immutable");
                }
                return Ok(&mut slot.value);
            }
        }
        Err("undefined")
    }

    pub fn assign(&mut self, name: &str, value: Value) -> Result<(), &'static str> {
        let slot_val = self.get_mut_var(name)?;
        *slot_val = value;
        Ok(())
    }
}
