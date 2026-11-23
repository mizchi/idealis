use crate::syntax::{
    Binary, Declaration, Expr, Field, Module, Mult, Quantifier, RawCommand, RawScope, Signature,
    Unary,
};
use crate::{Error, ErrorKind, Kind};

struct Parser {
    tokens: Vec<String>,
    position: usize,
    depth: usize,
}

pub(crate) fn parse(source: &str) -> Result<Module, Error> {
    if source.len() > 1_048_576 {
        return Err(Error::new(ErrorKind::Resource, "source exceeds 1 MiB"));
    }
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut position = 0;
    while position < bytes.len() {
        if bytes[position].is_ascii_whitespace() {
            position += 1;
            continue;
        }
        if source[position..].starts_with("//") || source[position..].starts_with("--") {
            while position < bytes.len() && bytes[position] != b'\n' {
                position += 1;
            }
            continue;
        }
        if source[position..].starts_with("/*") {
            let Some(end) = source[position + 2..].find("*/") else {
                return Err(Error::new(ErrorKind::Syntax, "unterminated comment"));
            };
            position += end + 4;
            continue;
        }
        let start = position;
        if bytes[position].is_ascii_alphanumeric() || bytes[position] == b'_' {
            while position < bytes.len()
                && (bytes[position].is_ascii_alphanumeric() || bytes[position] == b'_')
            {
                position += 1;
            }
        } else if let Some(operator) = ["<=>", "=>", "->", "<:", ":>", "&&", "||", "!=", "!in"]
            .into_iter()
            .find(|operator| source[position..].starts_with(operator))
        {
            position += operator.len();
        } else if b"{}()[],:|.+-&=~^*!;".contains(&bytes[position]) {
            position += 1;
        } else {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!("token at byte {position}"),
            ));
        }
        if position - start > 256 {
            return Err(Error::new(ErrorKind::Resource, "token exceeds 256 bytes"));
        }
        tokens.push(source[start..position].to_owned());
        if tokens.len() > 100_000 {
            return Err(Error::new(ErrorKind::Resource, "too many tokens"));
        }
    }
    Parser {
        tokens,
        position: 0,
        depth: 0,
    }
    .module()
}

impl Parser {
    fn peek(&self) -> &str {
        self.tokens
            .get(self.position)
            .map(String::as_str)
            .unwrap_or("")
    }
    fn take(&mut self, token: &str) -> bool {
        if self.peek() != token {
            return false;
        }
        self.position += 1;
        true
    }
    fn expect(&mut self, token: &str) -> Result<(), Error> {
        if self.take(token) {
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::Syntax,
                format!(
                    "expected {token}, got {} at token {}",
                    self.peek(),
                    self.position
                ),
            ))
        }
    }
    fn name(&mut self) -> Result<String, Error> {
        let token = self.peek();
        let keywords = [
            "abstract",
            "after",
            "all",
            "always",
            "and",
            "as",
            "assert",
            "before",
            "but",
            "check",
            "disj",
            "else",
            "eventually",
            "exactly",
            "extends",
            "fact",
            "for",
            "fun",
            "historically",
            "iden",
            "iff",
            "implies",
            "in",
            "Int",
            "let",
            "lone",
            "module",
            "no",
            "none",
            "not",
            "once",
            "one",
            "open",
            "or",
            "pred",
            "private",
            "seq",
            "set",
            "sig",
            "some",
            "String",
            "sum",
            "univ",
            "var",
        ];
        if token.is_empty()
            || !token.as_bytes()[0].is_ascii_alphabetic() && !token.starts_with('_')
            || keywords.contains(&token)
        {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!("expected supported identifier, got {token}"),
            ));
        }
        let name = token.to_owned();
        self.position += 1;
        Ok(name)
    }
    fn number(&mut self) -> Result<usize, Error> {
        let result = self
            .peek()
            .parse()
            .map_err(|_| Error::new(ErrorKind::Syntax, "expected nonnegative scope"))?;
        self.position += 1;
        Ok(result)
    }
    /// A signature name in a field type, including the builtin Int and String.
    fn column(&mut self) -> Result<String, Error> {
        if ["Int", "String"].contains(&self.peek()) {
            let name = self.peek().to_owned();
            self.position += 1;
            Ok(name)
        } else {
            self.name()
        }
    }
    fn mult(&mut self, default: Mult) -> Mult {
        if self.take("one") {
            Mult::One
        } else if self.take("lone") {
            Mult::Lone
        } else if self.take("some") {
            Mult::Some
        } else if self.take("set") {
            Mult::Set
        } else {
            default
        }
    }
    fn module(mut self) -> Result<Module, Error> {
        let mut module = Module::default();
        if self.take("module") {
            self.name()?;
        }
        while !self.peek().is_empty() {
            if self.take(";") {
                continue;
            }
            if ["sig", "one", "lone", "some"].contains(&self.peek()) {
                let mult = self.mult(Mult::Set);
                self.expect("sig")?;
                let owner = self.name()?;
                self.expect("{")?;
                while !self.take("}") {
                    let mut names = vec![self.name()?];
                    while self.take(",") {
                        names.push(self.name()?);
                    }
                    self.expect(":")?;
                    let explicit = ["one", "lone", "some", "set"].contains(&self.peek());
                    let field_mult = self.mult(Mult::One);
                    let mut columns = vec![self.column()?];
                    let mut arrows = Vec::new();
                    loop {
                        let left = ["one", "lone", "some", "set"].contains(&self.peek());
                        let start = self.position;
                        let left_mult = self.mult(Mult::Set);
                        if !self.take("->") {
                            if left {
                                self.position = start;
                            }
                            break;
                        }
                        let right_mult = self.mult(Mult::Set);
                        arrows.push((left_mult, right_mult));
                        columns.push(self.column()?);
                    }
                    if !arrows.is_empty() && explicit {
                        return Err(Error::new(
                            ErrorKind::Unsupported,
                            "multiplicity before an arrow field type",
                        ));
                    }
                    if arrows.len() > 2
                        || arrows.len() == 2
                            && arrows.iter().any(|arrow| *arrow != (Mult::Set, Mult::Set))
                    {
                        return Err(Error::new(
                            ErrorKind::Unsupported,
                            "arrow multiplicities beyond one arrow, or fields above arity four",
                        ));
                    }
                    let arrow = (arrows.len() == 1).then(|| arrows[0]);
                    for name in names {
                        module.fields.push(Field {
                            name,
                            owner: owner.clone(),
                            columns: columns.clone(),
                            mult: if arrows.is_empty() {
                                field_mult
                            } else {
                                Mult::Set
                            },
                            arrow,
                        });
                    }
                    if self.take(",") {
                        continue;
                    }
                    if self.peek() != "}" {
                        return Err(Error::new(
                            ErrorKind::Unsupported,
                            "only named unary field ranges are supported",
                        ));
                    }
                }
                module.signatures.push(Signature { name: owner, mult });
            } else if self.take("fact") {
                if self.peek() != "{" {
                    self.name()?;
                }
                module.facts.push(self.block()?);
            } else if self.take("pred") {
                let name = self.name()?;
                if self.take("[") {
                    self.expect("]")?;
                }
                let body = self.block()?;
                if module.predicates.insert(name, body).is_some() {
                    return Err(Error::new(ErrorKind::Type, "duplicate predicate"));
                }
            } else if self.take("assert") {
                let name = self.name()?;
                let body = self.block()?;
                if module.assertions.insert(name, body).is_some() {
                    return Err(Error::new(ErrorKind::Type, "duplicate assertion"));
                }
            } else {
                let label = if self
                    .tokens
                    .get(self.position + 1)
                    .is_some_and(|token| token == ":")
                {
                    let label = self.name()?;
                    self.expect(":")?;
                    label
                } else {
                    format!("command-{}", module.commands.len())
                };
                let kind = if self.take("run") {
                    Kind::Run
                } else if self.take("check") {
                    Kind::Check
                } else {
                    return Err(Error::new(
                        ErrorKind::Unsupported,
                        format!("paragraph {}", self.peek()),
                    ));
                };
                let body = if self.peek() == "{" {
                    self.block()?
                } else {
                    Expr::Name(self.name()?)
                };
                let scope = self.scope()?;
                module.commands.push(RawCommand {
                    label,
                    kind,
                    body,
                    scope,
                });
            }
            if module.signatures.len() > 8
                || module.fields.len() > 8
                || module.predicates.len() + module.assertions.len() > 64
                || module.commands.len() > 64
            {
                return Err(Error::new(ErrorKind::Resource, "declaration limit"));
            }
        }
        if module.commands.is_empty() {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "at least one run/check is required",
            ));
        }
        Ok(module)
    }
    fn scope(&mut self) -> Result<RawScope, Error> {
        let mut scope = RawScope {
            overall: Some(3),
            entries: Vec::new(),
            bitwidth: 4,
        };
        if !self.take("for") {
            return Ok(scope);
        }
        scope.overall = None;
        if !self.take("exactly") {
            let number = self.number()?;
            if self.take("but") {
                scope.overall = Some(number);
            } else if self.peek().is_empty()
                || [
                    "sig", "fact", "pred", "assert", "run", "check", "one", "lone", "some", ";",
                ]
                .contains(&self.peek())
                || self
                    .tokens
                    .get(self.position + 1)
                    .is_some_and(|token| token == ":")
            {
                scope.overall = Some(number);
                return Ok(scope);
            } else {
                self.position -= 1;
            }
        } else {
            self.position -= 1;
        }
        let mut seen_int = false;
        loop {
            let exact = self.take("exactly");
            let number = self.number()?;
            if self.take("Int") {
                if seen_int || exact {
                    return Err(Error::new(
                        ErrorKind::Scope,
                        "duplicate or exact integer bitwidth",
                    ));
                }
                scope.bitwidth = number;
                seen_int = true;
            } else {
                scope.entries.push((self.name()?, number, exact));
            }
            if !self.take(",") {
                break;
            }
        }
        Ok(scope)
    }
    fn block(&mut self) -> Result<Expr, Error> {
        self.expect("{")?;
        let mut expressions = Vec::new();
        while !self.take("}") {
            if self.peek().is_empty() {
                return Err(Error::new(ErrorKind::Syntax, "unterminated block"));
            }
            expressions.push(self.expression(0)?);
        }
        Ok(Expr::Block(expressions))
    }
    fn declaration_ahead(&self) -> bool {
        let mut position = self.position;
        if self
            .tokens
            .get(position)
            .is_some_and(|token| token == "disj")
        {
            position += 1;
        }
        loop {
            let Some(name) = self.tokens.get(position) else {
                return false;
            };
            if !name.as_bytes()[0].is_ascii_alphabetic() && !name.starts_with('_') {
                return false;
            }
            position += 1;
            match self.tokens.get(position).map(String::as_str) {
                Some(":") => return true,
                Some(",") => position += 1,
                _ => return false,
            }
        }
    }
    fn expression(&mut self, minimum: u8) -> Result<Expr, Error> {
        let start = self.position;
        self.depth += 1;
        if self.depth > 48 {
            return Err(Error::new(
                ErrorKind::Resource,
                "expression nesting exceeds 48",
            ));
        }
        let mut left = if self.take("(") {
            let inner = self.expression(0)?;
            self.expect(")")?;
            inner
        } else if self.peek() == "{" {
            self.block()?
        } else if ["all", "no", "some", "one", "lone"].contains(&self.peek()) {
            let word = self.peek().to_owned();
            self.position += 1;
            if self.declaration_ahead() {
                let quantifier = match word.as_str() {
                    "all" => Quantifier::All,
                    "no" => Quantifier::No,
                    "some" => Quantifier::Some,
                    "one" => Quantifier::One,
                    _ => Quantifier::Lone,
                };
                let mut declarations = Vec::new();
                loop {
                    let disjoint = self.take("disj");
                    let mut names = vec![self.name()?];
                    while self.take(",") {
                        names.push(self.name()?);
                    }
                    self.expect(":")?;
                    let domain = self.expression(0)?;
                    declarations.push(Declaration {
                        names,
                        domain,
                        disjoint,
                    });
                    if !self.take(",") {
                        break;
                    }
                }
                self.expect("|")?;
                Expr::Quantified(quantifier, declarations, Box::new(self.expression(0)?))
            } else {
                let operator = match word.as_str() {
                    "no" => Unary::No,
                    "some" => Unary::Some,
                    "one" => Unary::One,
                    "lone" => Unary::Lone,
                    _ => return Err(Error::new(ErrorKind::Syntax, "all needs a declaration")),
                };
                Expr::Unary(operator, Box::new(self.expression(7)?))
            }
        } else if self.take("not") || self.take("!") {
            Expr::Unary(Unary::Not, Box::new(self.expression(5)?))
        } else if ["~", "^", "*"].contains(&self.peek()) {
            let operator = match self.peek() {
                "~" => Unary::Transpose,
                "^" => Unary::Closure,
                _ => Unary::Reflexive,
            };
            self.position += 1;
            Expr::Unary(operator, Box::new(self.expression(15)?))
        } else if ["none", "univ", "iden", "Int", "String"].contains(&self.peek()) {
            let name = self.peek().to_owned();
            self.position += 1;
            Expr::Name(name)
        } else {
            Expr::Name(self.name()?)
        };
        loop {
            if self.position - start > 512 {
                return Err(Error::new(
                    ErrorKind::Resource,
                    "expression exceeds 512 tokens",
                ));
            }
            if self.take("[") {
                self.expect("]")?;
                let Expr::Name(name) = left else {
                    return Err(Error::new(
                        ErrorKind::Unsupported,
                        "only zero-argument predicate calls",
                    ));
                };
                left = Expr::Call(name);
                continue;
            }
            let (operator, precedence, right_associative) = match self.peek() {
                "or" | "||" => (Binary::Or, 1, false),
                "iff" | "<=>" => (Binary::Iff, 2, false),
                "implies" | "=>" => (Binary::Implies, 3, true),
                "and" | "&&" => (Binary::And, 4, false),
                "=" => (Binary::Equal, 6, false),
                "!=" => (Binary::NotEqual, 6, false),
                "in" => (Binary::In, 6, false),
                "!in" => (Binary::NotIn, 6, false),
                "not"
                    if self
                        .tokens
                        .get(self.position + 1)
                        .is_some_and(|token| token == "in") =>
                {
                    (Binary::NotIn, 6, false)
                }
                "+" => (Binary::Union, 8, false),
                "-" => (Binary::Difference, 8, false),
                "&" => (Binary::Intersection, 9, false),
                "->" => (Binary::Product, 10, true),
                "<:" => (Binary::Domain, 11, false),
                ":>" => (Binary::Range, 12, false),
                "." => (Binary::Join, 14, false),
                _ => break,
            };
            if precedence < minimum {
                break;
            }
            if self.take("not") {
                self.expect("in")?;
            } else {
                self.position += 1;
            }
            let right = self.expression(if right_associative {
                precedence
            } else {
                precedence + 1
            })?;
            left = Expr::Binary(operator, Box::new(left), Box::new(right));
        }
        self.depth -= 1;
        Ok(left)
    }
}
