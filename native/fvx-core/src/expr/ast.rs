use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Eq,
    Ne,
    StrictEq,
    StrictNe,
    Lt,
    Gt,
    Le,
    Ge,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    UShr,
    In,
    InstanceOf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicOp {
    And,
    Or,
    Nullish,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Plus,
    Not,
    BitNot,
    TypeOf,
    Void,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclKind {
    Var,
    Let,
    Const,
}

#[derive(Clone, Debug)]
pub enum Pattern {
    Name(String),
    Array(Vec<Option<(Pattern, Option<Expr>)>>, Option<Box<Pattern>>),
    Object(Vec<(String, Pattern, Option<Expr>)>),
}

#[derive(Clone, Debug)]
pub struct Param {
    pub pat: Pattern,
    pub default: Option<Expr>,
    pub rest: bool,
}

#[derive(Clone, Debug)]
pub enum Body {
    Block(Vec<Stmt>),
    Expr(Box<Expr>),
}

#[derive(Debug)]
pub struct FuncDef {
    pub name: Option<String>,
    pub params: Vec<Param>,
    pub body: Body,
    pub arrow: bool,
}

#[derive(Clone, Debug)]
pub enum PropKey {
    Name(String),
    Computed(Expr),
}

#[derive(Clone, Debug)]
pub enum ObjEntry {
    Pair(PropKey, Expr),
    Spread(Expr),
}

#[derive(Clone, Debug)]
pub enum Arg {
    Plain(Expr),
    Spread(Expr),
}

#[derive(Clone, Debug)]
pub enum Expr {
    Num(f64),
    Str(String),
    Bool(bool),
    Null,
    Undefined,
    Tmpl(Vec<String>, Vec<Expr>),
    Ident(String),
    This,
    Array(Vec<Option<Arg>>),
    Object(Vec<ObjEntry>),
    Func(Arc<FuncDef>),
    Unary(UnOp, Box<Expr>),
    Update { inc: bool, prefix: bool, target: Box<Expr> },
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Logic(LogicOp, Box<Expr>, Box<Expr>),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>),
    Assign(Option<BinOp>, Box<Expr>, Box<Expr>),
    LogicAssign(LogicOp, Box<Expr>, Box<Expr>),
    DestructAssign(Pattern, Box<Expr>),
    Member(Box<Expr>, String, bool),
    Index(Box<Expr>, Box<Expr>, bool),
    Call(Box<Expr>, Vec<Arg>, bool),
    New(Box<Expr>, Vec<Arg>),
    Seq(Vec<Expr>),
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub line: u32,
    pub kind: StmtKind,
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    Expr(Expr),
    Decl(DeclKind, Vec<(Pattern, Option<Expr>)>),
    Func(Arc<FuncDef>),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    Block(Vec<Stmt>),
    For { init: Option<Box<Stmt>>, test: Option<Expr>, update: Option<Expr>, body: Box<Stmt> },
    ForEach { kind: Option<DeclKind>, pat: Pattern, of: bool, obj: Expr, body: Box<Stmt> },
    While(Expr, Box<Stmt>),
    DoWhile(Box<Stmt>, Expr),
    Break(Option<String>),
    Continue(Option<String>),
    Labeled(String, Box<Stmt>),
    Return(Option<Expr>),
    Throw(Expr),
    Try { block: Vec<Stmt>, param: Option<Pattern>, handler: Option<Vec<Stmt>>, finally: Option<Vec<Stmt>> },
    Switch(Expr, Vec<(Option<Expr>, Vec<Stmt>)>),
    Empty,
}

#[derive(Debug)]
pub struct Program {
    pub body: Vec<Stmt>,
}
