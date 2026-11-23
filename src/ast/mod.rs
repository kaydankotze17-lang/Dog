use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(file: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRef {
    Text,
    Number,
    Decimal,
    Bool,
    Void,
    List(Box<TypeRef>),
    Map(Box<TypeRef>, Box<TypeRef>),
    Named(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: TypeRef,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField {
    pub name: String,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchCase {
    pub pattern: Expr,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKind {
    Let,
    Var,
    Const,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Gt,
    Lt,
    GtEq,
    LtEq,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterpPart {
    Literal(String),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    StringLit {
        value: String,
        span: Span,
    },
    InterpString {
        parts: Vec<InterpPart>,
        span: Span,
    },
    NumberLit {
        value: i64,
        span: Span,
    },
    DecimalLit {
        value: f64,
        span: Span,
    },
    BoolLit {
        value: bool,
        span: Span,
    },
    ListLit {
        elements: Vec<Expr>,
        span: Span,
    },
    MapLit {
        entries: Vec<(Expr, Expr)>,
        span: Span,
    },
    StructInit {
        name: String,
        fields: Vec<(String, Expr)>,
        span: Span,
    },
    Identifier {
        name: String,
        span: Span,
    },
    FieldAccess {
        target: Box<Expr>,
        field: String,
        span: Span,
    },
    Index {
        target: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    Binary {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
        span: Span,
    },
    Call {
        callee: String,
        args: Vec<Expr>,
        span: Span,
    },
    NamespaceCall {
        module: String,
        func: String,
        args: Vec<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> &Span {
        match self {
            Expr::StringLit { span, .. }
            | Expr::InterpString { span, .. }
            | Expr::NumberLit { span, .. }
            | Expr::DecimalLit { span, .. }
            | Expr::BoolLit { span, .. }
            | Expr::ListLit { span, .. }
            | Expr::MapLit { span, .. }
            | Expr::StructInit { span, .. }
            | Expr::Identifier { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::Index { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Call { span, .. }
            | Expr::NamespaceCall { span, .. } => span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Use {
        path: String,
        span: Span,
    },
    StructDef {
        name: String,
        fields: Vec<StructField>,
        span: Span,
    },
    EnumDef {
        name: String,
        variants: Vec<String>,
        span: Span,
    },
    Binding {
        kind: BindingKind,
        name: String,
        type_ann: Option<TypeRef>,
        value: Expr,
        span: Span,
    },
    Assign {
        name: String,
        value: Expr,
        span: Span,
    },
    CompoundAssign {
        name: String,
        op: BinOp,
        value: Expr,
        span: Span,
    },
    IndexAssign {
        target: String,
        index: Expr,
        value: Expr,
        span: Span,
    },
    FieldAssign {
        target: String,
        field: String,
        value: Expr,
        span: Span,
    },
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        otherwise_branch: Option<Vec<Stmt>>,
        span: Span,
    },
    Match {
        subject: Expr,
        cases: Vec<MatchCase>,
        otherwise_branch: Option<Vec<Stmt>>,
        span: Span,
    },
    TryCatch {
        try_body: Vec<Stmt>,
        err_name: String,
        catch_body: Vec<Stmt>,
        span: Span,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    Repeat {
        count: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    For {
        var_name: String,
        iterable: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    FnDef {
        name: String,
        params: Vec<Param>,
        return_type: TypeRef,
        body: Vec<Stmt>,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    ExprStmt {
        expr: Expr,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Stmt>,
}
