//! Database formula language: a small, side-effect-free expression language (no general code execution).
//!
//! Examples: `prop("Price") * prop("Qty")`, `if(prop("Done"), "✓", "")`,
//! `dateBetween(prop("Due"), today(), "days")`, `concat(upper(prop("Name")), " #", prop("No"))`.

use chrono::{Datelike, Duration, NaiveDate};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq)]
pub enum FV {
    Null,
    Num(f64),
    Text(String),
    Bool(bool),
    Date(NaiveDate),
    List(Vec<FV>),
}

impl FV {
    pub fn to_json(&self) -> Value {
        match self {
            FV::Null => Value::Null,
            FV::Num(n) if n.is_finite() => json!(n),
            FV::Num(_) => Value::Null,
            FV::Text(s) => json!(s),
            FV::Bool(b) => json!(b),
            FV::Date(d) => json!({ "start": d.to_string() }),
            FV::List(l) => Value::Array(l.iter().map(|x| x.to_json()).collect()),
        }
    }

    pub fn as_text(&self) -> String {
        match self {
            FV::Null => String::new(),
            FV::Num(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", *n as i64),
            FV::Num(n) => format!("{}", (n * 1e6).round() / 1e6),
            FV::Text(s) => s.clone(),
            FV::Bool(b) => if *b { "true" } else { "false" }.into(),
            FV::Date(d) => d.to_string(),
            FV::List(l) => l.iter().map(|x| x.as_text()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", "),
        }
    }

    fn truthy(&self) -> bool {
        match self {
            FV::Null => false,
            FV::Num(n) => *n != 0.0,
            FV::Text(s) => !s.is_empty(),
            FV::Bool(b) => *b,
            FV::Date(_) => true,
            FV::List(l) => !l.is_empty(),
        }
    }

    fn num(&self) -> Result<f64, String> {
        match self {
            FV::Num(n) => Ok(*n),
            FV::Null => Ok(0.0),
            FV::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            FV::Text(s) => s.trim().parse().map_err(|_| format!("“{s}” is not a number")),
            other => Err(format!("expected a number, got {}", other.kind())),
        }
    }

    fn date(&self) -> Result<NaiveDate, String> {
        match self {
            FV::Date(d) => Ok(*d),
            FV::Text(s) => NaiveDate::parse_from_str(s.get(0..10).unwrap_or(""), "%Y-%m-%d").map_err(|_| format!("“{s}” is not a date")),
            other => Err(format!("expected a date, got {}", other.kind())),
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            FV::Null => "empty",
            FV::Num(_) => "number",
            FV::Text(_) => "text",
            FV::Bool(_) => "true/false",
            FV::Date(_) => "date",
            FV::List(_) => "list",
        }
    }
}

// ---------- lexer ----------

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Op(&'static str),
    LParen,
    RParen,
    Comma,
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            out.push(Tok::Num(s.parse().map_err(|_| format!("bad number {s}"))?));
        } else if c == '"' || c == '“' || c == '\'' {
            let close = match c { '“' => '”', other => other };
            i += 1;
            let mut s = String::new();
            loop {
                match chars.get(i) {
                    None => return Err("text is missing its closing quote".into()),
                    Some('\\') => {
                        s.push(*chars.get(i + 1).unwrap_or(&'\\'));
                        i += 2;
                    }
                    Some(ch) if *ch == close || (close == '"' && *ch == '”') => {
                        i += 1;
                        break;
                    }
                    Some(ch) => {
                        s.push(*ch);
                        i += 1;
                    }
                }
            }
            out.push(Tok::Str(s));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let w: String = chars[start..i].iter().collect();
            match w.as_str() {
                "and" => out.push(Tok::Op("&&")),
                "or" => out.push(Tok::Op("||")),
                "not" => out.push(Tok::Op("!")),
                _ => out.push(Tok::Ident(w)),
            }
        } else {
            let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
            let op = match two.as_str() {
                "==" | "!=" | ">=" | "<=" | "&&" | "||" => Some(two.clone()),
                _ => None,
            };
            if let Some(op) = op {
                out.push(Tok::Op(match op.as_str() { "==" => "==", "!=" => "!=", ">=" => ">=", "<=" => "<=", "&&" => "&&", _ => "||" }));
                i += 2;
                continue;
            }
            out.push(match c {
                '(' => Tok::LParen,
                ')' => Tok::RParen,
                ',' => Tok::Comma,
                '+' => Tok::Op("+"),
                '-' => Tok::Op("-"),
                '*' => Tok::Op("*"),
                '/' => Tok::Op("/"),
                '%' => Tok::Op("%"),
                '>' => Tok::Op(">"),
                '<' => Tok::Op("<"),
                '!' => Tok::Op("!"),
                '=' => Tok::Op("=="),
                other => return Err(format!("unexpected character “{other}”")),
            });
            i += 1;
        }
    }
    Ok(out)
}

// ---------- parser ----------

#[derive(Clone, Debug)]
pub enum Expr {
    Lit(FV),
    Call(String, Vec<Expr>),
    Unary(&'static str, Box<Expr>),
    Bin(&'static str, Box<Expr>, Box<Expr>),
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }
    fn eat_op(&mut self, ops: &[&'static str]) -> Option<&'static str> {
        if let Some(Tok::Op(o)) = self.peek() {
            if let Some(found) = ops.iter().find(|x| *x == o) {
                self.pos += 1;
                return Some(found);
            }
        }
        None
    }
    fn binary(&mut self, ops: &[&'static str], next: fn(&mut Parser) -> Result<Expr, String>) -> Result<Expr, String> {
        let mut left = next(self)?;
        while let Some(op) = self.eat_op(ops) {
            let right = next(self)?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn or(&mut self) -> Result<Expr, String> {
        self.binary(&["||"], Parser::and)
    }
    fn and(&mut self) -> Result<Expr, String> {
        self.binary(&["&&"], Parser::cmp)
    }
    fn cmp(&mut self) -> Result<Expr, String> {
        self.binary(&["==", "!=", ">=", "<=", ">", "<"], Parser::add)
    }
    fn add(&mut self) -> Result<Expr, String> {
        self.binary(&["+", "-"], Parser::mul)
    }
    fn mul(&mut self) -> Result<Expr, String> {
        self.binary(&["*", "/", "%"], Parser::unary)
    }
    fn unary(&mut self) -> Result<Expr, String> {
        if let Some(op) = self.eat_op(&["-", "!"]) {
            return Ok(Expr::Unary(op, Box::new(self.unary()?)));
        }
        self.primary()
    }
    fn primary(&mut self) -> Result<Expr, String> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Expr::Lit(FV::Num(n))),
            Some(Tok::Str(s)) => Ok(Expr::Lit(FV::Text(s))),
            Some(Tok::LParen) => {
                let e = self.or()?;
                match self.next() {
                    Some(Tok::RParen) => Ok(e),
                    _ => Err("missing “)”".into()),
                }
            }
            Some(Tok::Ident(name)) => match name.as_str() {
                "true" => Ok(Expr::Lit(FV::Bool(true))),
                "false" => Ok(Expr::Lit(FV::Bool(false))),
                "empty" | "null" if self.peek() != Some(&Tok::LParen) => Ok(Expr::Lit(FV::Null)),
                _ => {
                    if self.next() != Some(Tok::LParen) {
                        return Err(format!("“{name}” must be followed by ( … ) — use prop(\"Name\") to read a property"));
                    }
                    let mut args = vec![];
                    if self.peek() == Some(&Tok::RParen) {
                        self.pos += 1;
                    } else {
                        loop {
                            args.push(self.or()?);
                            match self.next() {
                                Some(Tok::Comma) => continue,
                                Some(Tok::RParen) => break,
                                _ => return Err(format!("missing “)” after arguments of {name}")),
                            }
                        }
                    }
                    if !FUNCTIONS.contains(&name.as_str()) {
                        return Err(format!("unknown function {name}()"));
                    }
                    Ok(Expr::Call(name, args))
                }
            },
            Some(t) => Err(format!("unexpected {t:?}")),
            None => Err("the formula ends too early".into()),
        }
    }
}

pub const FUNCTIONS: &[&str] = &[
    "prop", "if", "concat", "length", "upper", "lower", "contains", "replace", "round", "floor", "ceil", "abs", "min", "max",
    "sum", "average", "toNumber", "toText", "now", "today", "dateAdd", "dateSubtract", "dateBetween", "formatDate", "year",
    "month", "day", "empty", "not", "and", "or",
];

pub fn parse(src: &str) -> Result<Expr, String> {
    if src.trim().is_empty() {
        return Ok(Expr::Lit(FV::Null));
    }
    let mut p = Parser { toks: lex(src)?, pos: 0 };
    let e = p.or()?;
    if p.pos < p.toks.len() {
        return Err(format!("unexpected {:?}", p.toks[p.pos]));
    }
    Ok(e)
}

/// Names of properties referenced with prop("…") (for dependency/cycle checks).
pub fn references(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Call(name, args) => {
            if name == "prop" {
                if let Some(Expr::Lit(FV::Text(s))) = args.first() {
                    out.push(s.clone());
                }
            }
            args.iter().for_each(|a| references(a, out));
        }
        Expr::Unary(_, a) => references(a, out),
        Expr::Bin(_, a, b) => {
            references(a, out);
            references(b, out);
        }
        Expr::Lit(_) => {}
    }
}

// ---------- evaluator ----------

fn add_months(d: NaiveDate, n: i64) -> NaiveDate {
    let total = d.year() as i64 * 12 + d.month0() as i64 + n;
    let (y, m) = (total.div_euclid(12) as i32, total.rem_euclid(12) as u32 + 1);
    let mut day = d.day();
    loop {
        if let Some(x) = NaiveDate::from_ymd_opt(y, m, day) {
            return x;
        }
        day -= 1;
    }
}

fn shift(d: NaiveDate, n: f64, unit: &str) -> Result<NaiveDate, String> {
    let n = n.round() as i64;
    Ok(match unit {
        "days" | "day" => d + Duration::days(n),
        "weeks" | "week" => d + Duration::weeks(n),
        "months" | "month" => add_months(d, n),
        "years" | "year" => add_months(d, n * 12),
        other => return Err(format!("unit must be days, weeks, months or years (got {other})")),
    })
}

fn flatten(vals: Vec<FV>) -> Vec<FV> {
    vals.into_iter().flat_map(|v| match v { FV::List(l) => l, x => vec![x] }).collect()
}

pub fn eval(e: &Expr, prop: &mut dyn FnMut(&str) -> Result<FV, String>, today: NaiveDate, depth: usize) -> Result<FV, String> {
    if depth > 200 {
        return Err("formula is nested too deeply".into());
    }
    let ev = |x: &Expr, prop: &mut dyn FnMut(&str) -> Result<FV, String>| eval(x, prop, today, depth + 1);
    match e {
        Expr::Lit(v) => Ok(v.clone()),
        Expr::Unary("-", a) => Ok(FV::Num(-ev(a, prop)?.num()?)),
        Expr::Unary(_, a) => Ok(FV::Bool(!ev(a, prop)?.truthy())),
        Expr::Bin(op, a, b) => {
            // Short-circuit logic.
            if *op == "&&" {
                return Ok(FV::Bool(ev(a, prop)?.truthy() && ev(b, prop)?.truthy()));
            }
            if *op == "||" {
                return Ok(FV::Bool(ev(a, prop)?.truthy() || ev(b, prop)?.truthy()));
            }
            let (x, y) = (ev(a, prop)?, ev(b, prop)?);
            match *op {
                "+" => match (&x, &y) {
                    (FV::Text(_), _) | (_, FV::Text(_)) => Ok(FV::Text(x.as_text() + &y.as_text())),
                    _ => Ok(FV::Num(x.num()? + y.num()?)),
                },
                "-" => Ok(FV::Num(x.num()? - y.num()?)),
                "*" => Ok(FV::Num(x.num()? * y.num()?)),
                "/" => {
                    let d = y.num()?;
                    if d == 0.0 { Err("division by zero".into()) } else { Ok(FV::Num(x.num()? / d)) }
                }
                "%" => {
                    let d = y.num()?;
                    if d == 0.0 { Err("division by zero".into()) } else { Ok(FV::Num(x.num()? % d)) }
                }
                cmp => {
                    let ord = match (&x, &y) {
                        (FV::Num(_), _) | (_, FV::Num(_)) if !matches!((&x, &y), (FV::Text(_), FV::Text(_))) => x.num()?.partial_cmp(&y.num()?),
                        (FV::Date(_), _) | (_, FV::Date(_)) => Some(x.date()?.cmp(&y.date()?)),
                        _ => Some(x.as_text().cmp(&y.as_text())),
                    };
                    let ord = ord.ok_or("cannot compare these values")?;
                    use std::cmp::Ordering::*;
                    Ok(FV::Bool(match cmp {
                        "==" => ord == Equal,
                        "!=" => ord != Equal,
                        ">" => ord == Greater,
                        "<" => ord == Less,
                        ">=" => ord != Less,
                        _ => ord != Greater,
                    }))
                }
            }
        }
        Expr::Call(name, args) => {
            let need = |n: usize| -> Result<(), String> {
                if args.len() < n { Err(format!("{name}() needs {n} argument(s)")) } else { Ok(()) }
            };
            match name.as_str() {
                "prop" => {
                    need(1)?;
                    match ev(&args[0], prop)? {
                        FV::Text(n) => prop(&n),
                        _ => Err("prop() takes a property name in quotes".into()),
                    }
                }
                "if" => {
                    need(2)?;
                    if ev(&args[0], prop)?.truthy() { ev(&args[1], prop) } else { args.get(2).map(|a| ev(a, prop)).unwrap_or(Ok(FV::Null)) }
                }
                "empty" => {
                    need(1)?;
                    Ok(FV::Bool(!ev(&args[0], prop)?.truthy() && !matches!(ev(&args[0], prop)?, FV::Num(_) | FV::Bool(_))))
                }
                "not" => {
                    need(1)?;
                    Ok(FV::Bool(!ev(&args[0], prop)?.truthy()))
                }
                "and" | "or" => {
                    let vals: Vec<bool> = args.iter().map(|a| ev(a, prop).map(|v| v.truthy())).collect::<Result<_, _>>()?;
                    Ok(FV::Bool(if name == "and" { vals.iter().all(|x| *x) } else { vals.iter().any(|x| *x) }))
                }
                "concat" => Ok(FV::Text(args.iter().map(|a| ev(a, prop).map(|v| v.as_text())).collect::<Result<Vec<_>, _>>()?.join(""))),
                "length" => {
                    need(1)?;
                    Ok(FV::Num(match ev(&args[0], prop)? {
                        FV::List(l) => l.len() as f64,
                        v => v.as_text().chars().count() as f64,
                    }))
                }
                "upper" => { need(1)?; Ok(FV::Text(ev(&args[0], prop)?.as_text().to_uppercase())) }
                "lower" => { need(1)?; Ok(FV::Text(ev(&args[0], prop)?.as_text().to_lowercase())) }
                "toText" => { need(1)?; Ok(FV::Text(ev(&args[0], prop)?.as_text())) }
                "toNumber" => { need(1)?; Ok(FV::Num(ev(&args[0], prop)?.num()?)) }
                "contains" => {
                    need(2)?;
                    let (h, n) = (ev(&args[0], prop)?, ev(&args[1], prop)?.as_text());
                    Ok(FV::Bool(match h {
                        FV::List(l) => l.iter().any(|x| x.as_text() == n),
                        v => v.as_text().to_lowercase().contains(&n.to_lowercase()),
                    }))
                }
                "replace" => {
                    need(3)?;
                    Ok(FV::Text(ev(&args[0], prop)?.as_text().replace(&ev(&args[1], prop)?.as_text(), &ev(&args[2], prop)?.as_text())))
                }
                "round" => {
                    need(1)?;
                    let n = ev(&args[0], prop)?.num()?;
                    let digits = args.get(1).map(|a| ev(a, prop).and_then(|v| v.num())).transpose()?.unwrap_or(0.0).clamp(0.0, 10.0) as i32;
                    let f = 10f64.powi(digits);
                    Ok(FV::Num((n * f).round() / f))
                }
                "floor" => { need(1)?; Ok(FV::Num(ev(&args[0], prop)?.num()?.floor())) }
                "ceil" => { need(1)?; Ok(FV::Num(ev(&args[0], prop)?.num()?.ceil())) }
                "abs" => { need(1)?; Ok(FV::Num(ev(&args[0], prop)?.num()?.abs())) }
                "min" | "max" | "sum" | "average" => {
                    let vals = flatten(args.iter().map(|a| ev(a, prop)).collect::<Result<_, _>>()?);
                    let nums: Vec<f64> = vals.iter().filter(|v| !matches!(v, FV::Null)).map(|v| v.num()).collect::<Result<_, _>>()?;
                    if nums.is_empty() {
                        return Ok(if name == "sum" { FV::Num(0.0) } else { FV::Null });
                    }
                    Ok(FV::Num(match name.as_str() {
                        "min" => nums.iter().cloned().fold(f64::INFINITY, f64::min),
                        "max" => nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                        "sum" => nums.iter().sum(),
                        _ => nums.iter().sum::<f64>() / nums.len() as f64,
                    }))
                }
                "now" | "today" => Ok(FV::Date(today)),
                "dateAdd" | "dateSubtract" => {
                    need(3)?;
                    let d = ev(&args[0], prop)?;
                    if d == FV::Null {
                        return Ok(FV::Null);
                    }
                    let n = ev(&args[1], prop)?.num()? * if name == "dateSubtract" { -1.0 } else { 1.0 };
                    Ok(FV::Date(shift(d.date()?, n, &ev(&args[2], prop)?.as_text())?))
                }
                "dateBetween" => {
                    need(3)?;
                    let (a, b) = (ev(&args[0], prop)?, ev(&args[1], prop)?);
                    if a == FV::Null || b == FV::Null {
                        return Ok(FV::Null);
                    }
                    let (a, b) = (a.date()?, b.date()?);
                    let days = (a - b).num_days() as f64;
                    Ok(FV::Num(match ev(&args[2], prop)?.as_text().as_str() {
                        "days" | "day" => days,
                        "weeks" | "week" => (days / 7.0).trunc(),
                        "months" | "month" => ((a.year() - b.year()) * 12 + a.month() as i32 - b.month() as i32) as f64,
                        "years" | "year" => (a.year() - b.year()) as f64,
                        other => return Err(format!("unit must be days, weeks, months or years (got {other})")),
                    }))
                }
                "formatDate" => {
                    need(1)?;
                    let d = ev(&args[0], prop)?;
                    if d == FV::Null {
                        return Ok(FV::Text(String::new()));
                    }
                    let fmt = args.get(1).map(|a| ev(a, prop).map(|v| v.as_text())).transpose()?.unwrap_or_else(|| "%Y-%m-%d".into());
                    let fmt = fmt.replace("YYYY", "%Y").replace("MM", "%m").replace("DD", "%d");
                    Ok(FV::Text(d.date()?.format(&fmt).to_string()))
                }
                "year" | "month" | "day" => {
                    need(1)?;
                    let d = ev(&args[0], prop)?;
                    if d == FV::Null {
                        return Ok(FV::Null);
                    }
                    let d = d.date()?;
                    Ok(FV::Num(match name.as_str() { "year" => d.year() as f64, "month" => d.month() as f64, _ => d.day() as f64 }))
                }
                other => Err(format!("unknown function {other}()")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(src: &str, props: &[(&str, FV)]) -> Result<FV, String> {
        let e = parse(src)?;
        let mut get = |n: &str| props.iter().find(|(k, _)| *k == n).map(|(_, v)| v.clone()).ok_or(format!("no property “{n}”"));
        eval(&e, &mut get, NaiveDate::from_ymd_opt(2026, 10, 21).unwrap(), 0)
    }

    #[test]
    fn arithmetic_text_and_logic() {
        assert_eq!(run("prop(\"ราคา\") * prop(\"จำนวน\") + 1", &[("ราคา", FV::Num(12.5)), ("จำนวน", FV::Num(2.0))]), Ok(FV::Num(26.0)));
        assert_eq!(run("concat(upper(\"ab\"), \"-\", 3)", &[]), Ok(FV::Text("AB-3".into())));
        assert_eq!(run("if(prop(\"Done\") and 2 > 1, \"✓\", \"…\")", &[("Done", FV::Bool(true))]), Ok(FV::Text("✓".into())));
        assert_eq!(run("round(10 / 3, 2)", &[]), Ok(FV::Num(3.33)));
        assert_eq!(run("\"a\" == \"a\" && !false", &[]), Ok(FV::Bool(true)));
    }

    #[test]
    fn dates() {
        let due = FV::Date(NaiveDate::from_ymd_opt(2026, 10, 31).unwrap());
        assert_eq!(run("dateBetween(prop(\"Due\"), today(), \"days\")", &[("Due", due.clone())]), Ok(FV::Num(10.0)));
        assert_eq!(run("formatDate(dateAdd(prop(\"Due\"), 1, \"months\"))", &[("Due", due)]), Ok(FV::Text("2026-11-30".into())));
    }

    #[test]
    fn errors_are_readable() {
        assert!(run("1 / 0", &[]).unwrap_err().contains("division"));
        assert!(run("prop(\"x\"", &[]).is_err());
        assert!(run("system(\"rm\")", &[]).unwrap_err().contains("unknown function"));
        assert!(run("prop(\"Missing\")", &[]).unwrap_err().contains("Missing"));
    }
}
