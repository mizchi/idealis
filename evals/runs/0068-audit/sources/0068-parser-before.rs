use crate::ast::{
    Assumption, Bound, CaseArm, Declaration, Definition, DefinitionKind, ExceptPath, ExceptUpdate,
    Expression, ExpressionKind, Instance, InstanceSubstitution, Location, Module, Pattern,
    RecordField, Theorem,
};
use crate::lexer::{Kind, Token, tokenize};
use crate::{ErrorKind, ParseError};

#[derive(Clone, Copy)]
struct Precedence {
    low: u8,
    high: u8,
    associative: bool,
}
fn infix(operator: &str) -> Option<Precedence> {
    let (low, high, associative) = match operator {
        "=>" => (1, 1, false),
        "<=>" | "~>" | "-+->" => (2, 2, false),
        "/\\" | "\\/" => (3, 3, true),
        "=" | ":=" | "#" | "<" | ">" | "<=" | ">=" | "\\in" | "\\notin" | "\\subseteq" => {
            (5, 5, false)
        }
        "\\cdot" => (5, 14, true),
        "@@" => (6, 6, true),
        ":>" | "<:" => (7, 7, false),
        "\\cup" | "\\cap" => (8, 8, true),
        "\\" => (8, 8, false),
        ".." | "..." => (9, 9, false),
        "+" | "++" => (10, 10, true),
        "%" => (10, 11, false),
        "%%" | "|" | "||" => (10, 11, true),
        "\\X" => (10, 13, false),
        "-" | "--" => (11, 11, true),
        "*" | "**" | "\\o" | "&" | "&&" => (13, 13, true),
        "/" | "//" | "\\div" => (13, 13, false),
        "^" | "^^" => (14, 14, false),
        _ => return None,
    };
    Some(Precedence {
        low,
        high,
        associative,
    })
}

fn within_ast_limit(expression: &Expression) -> bool {
    let mut pending = vec![(expression, 1)];
    while let Some((expression, depth)) = pending.pop() {
        if depth > crate::MAX_NESTING {
            return false;
        }
        let children: Vec<&Expression> = match &expression.node {
            ExpressionKind::Prime { expression }
            | ExpressionKind::Unchanged { expression }
            | ExpressionKind::Enabled { expression }
            | ExpressionKind::Always { expression }
            | ExpressionKind::Eventually { expression } => vec![expression],
            ExpressionKind::Apply { args, .. } | ExpressionKind::Bracket { args } => {
                args.iter().collect()
            }
            ExpressionKind::And { operands, .. } | ExpressionKind::Or { operands, .. } => {
                operands.iter().collect()
            }
            ExpressionKind::SetEnumeration { elements } | ExpressionKind::Tuple { elements } => {
                elements.iter().collect()
            }
            ExpressionKind::Product { factors } => factors.iter().collect(),
            ExpressionKind::If {
                condition,
                consequent,
                alternative,
            } => vec![condition, consequent, alternative],
            ExpressionKind::Case { arms, other } => arms
                .iter()
                .flat_map(|arm| [&arm.condition, &arm.body])
                .chain(other.iter().map(Box::as_ref))
                .collect(),
            ExpressionKind::Forall { bounds, body }
            | ExpressionKind::Exists { bounds, body }
            | ExpressionKind::FunctionConstruction { bounds, body }
            | ExpressionKind::SetMap {
                expression: body,
                bounds,
            } => bounds
                .iter()
                .map(|bound| &bound.set)
                .chain(std::iter::once(body.as_ref()))
                .collect(),
            ExpressionKind::UnboundedQuantifier { body, .. }
            | ExpressionKind::Label { body, .. } => vec![body],
            ExpressionKind::Let { definitions, body } => {
                let mut children = vec![body.as_ref()];
                for definition in definitions {
                    children.push(&definition.body);
                    if let DefinitionKind::Function { bounds } = &definition.head {
                        children.extend(bounds.iter().map(|bound| &bound.set));
                    }
                }
                children
            }
            ExpressionKind::Choose { set, body, .. } => set
                .iter()
                .map(Box::as_ref)
                .chain(std::iter::once(body.as_ref()))
                .collect(),
            ExpressionKind::SetFilter { bound, predicate } => vec![&bound.set, predicate],
            ExpressionKind::FunctionSet { domain, range } => vec![domain, range],
            ExpressionKind::Record { fields } | ExpressionKind::RecordSet { fields } => {
                fields.iter().map(|field| &field.value).collect()
            }
            ExpressionKind::Except { function, updates } => {
                let mut children = vec![function.as_ref()];
                for update in updates {
                    children.push(&update.value);
                    for part in &update.path {
                        if let ExceptPath::Index { args } = part {
                            children.extend(args);
                        }
                    }
                }
                children
            }
            ExpressionKind::FunctionApplication { function, args } => {
                std::iter::once(function.as_ref()).chain(args).collect()
            }
            ExpressionKind::FieldAccess { record, .. } => vec![record],
            ExpressionKind::BoxAction { action, subscript }
            | ExpressionKind::AngleAction { action, subscript }
            | ExpressionKind::WeakFairness { subscript, action }
            | ExpressionKind::StrongFairness { subscript, action } => vec![action, subscript],
            ExpressionKind::Number { .. }
            | ExpressionKind::Decimal { .. }
            | ExpressionKind::String { .. }
            | ExpressionKind::Identifier { .. }
            | ExpressionKind::ExceptAt => vec![],
        };
        pending.extend(children.into_iter().map(|child| (child, depth + 1)));
    }
    true
}

pub(crate) struct Parser {
    pub tokens: Vec<Token>,
    pub cursor: usize,
    offside: Vec<usize>,
    depth: usize,
}
impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            cursor: 0,
            offside: vec![],
            depth: 0,
        }
    }
    pub fn raw(&self) -> &Token {
        &self.tokens[self.cursor]
    }
    fn peek(&self, offset: usize) -> &Token {
        &self.tokens[(self.cursor + offset).min(self.tokens.len() - 1)]
    }
    fn blocked(&self) -> bool {
        self.offside
            .last()
            .is_some_and(|column| self.raw().location.column <= *column)
    }
    pub fn at(&self, text: &str) -> bool {
        !self.blocked() && self.raw().kind != Kind::String && self.raw().text == text
    }
    pub fn accept(&mut self, text: &str) -> bool {
        if self.at(text) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }
    pub fn advance(&mut self) -> Result<Token, ParseError> {
        if self.blocked() || self.raw().kind == Kind::End {
            return Err(self.error("Expected expression before end of input or offside token"));
        }
        let token = self.raw().clone();
        self.cursor += 1;
        Ok(token)
    }
    pub fn expect(&mut self, text: &str) -> Result<Token, ParseError> {
        if !self.at(text) {
            return Err(self.error(&format!("Expected {text}, found {:?}", self.raw().raw)));
        }
        self.advance()
    }
    pub fn name(&mut self) -> Result<Token, ParseError> {
        if self.raw().kind != Kind::Name {
            return Err(self.error("Expected identifier"));
        }
        self.advance()
    }
    pub fn error(&self, message: &str) -> ParseError {
        ParseError {
            kind: ErrorKind::Syntax,
            message: message.into(),
            location: self.raw().location,
        }
    }
    fn unsupported(&self, message: &str) -> ParseError {
        ParseError {
            kind: ErrorKind::Unsupported,
            message: message.into(),
            location: self.raw().location,
        }
    }
    fn expression(&mut self, minimum: u8) -> Result<Expression, ParseError> {
        if self.depth >= crate::MAX_NESTING {
            return Err(self.unsupported("Expression nesting exceeds 64"));
        }
        self.depth += 1;
        let result = self.expression_inner(minimum);
        self.depth -= 1;
        result
    }
    fn expression_inner(&mut self, minimum: u8) -> Result<Expression, ParseError> {
        let mut left = self.prefix()?;
        let mut previous: Option<(String, Precedence)> = None;
        loop {
            if !within_ast_limit(&left) {
                return Err(self.unsupported("AST nesting exceeds 64"));
            }
            if self.blocked() || self.raw().kind != Kind::Symbol {
                break;
            }
            let token = self.raw().clone();
            if token.text == "'" && minimum <= 15 {
                self.advance()?;
                left = Expression {
                    location: token.location,
                    node: ExpressionKind::Prime {
                        expression: Box::new(left),
                    },
                };
                continue;
            }
            if matches!(token.text.as_str(), "^+" | "^*" | "^#") && minimum <= 15 {
                self.advance()?;
                left = Expression {
                    location: token.location,
                    node: ExpressionKind::Apply {
                        operator: token.text,
                        args: vec![left],
                    },
                };
                continue;
            }
            if self.accept("[") {
                let args = self.expression_list("]")?;
                left = Expression {
                    location: token.location,
                    node: ExpressionKind::FunctionApplication {
                        function: Box::new(left),
                        args,
                    },
                };
                continue;
            }
            if self.accept(".") {
                let field = self.advance()?;
                if !matches!(field.kind, Kind::Name | Kind::Keyword) && field.text != "_" {
                    return Err(self.error("Expected record field"));
                }
                left = Expression {
                    location: token.location,
                    node: ExpressionKind::FieldAccess {
                        record: Box::new(left),
                        field: field.text,
                    },
                };
                continue;
            }
            let Some(precedence) = infix(&token.text) else {
                break;
            };
            if precedence.low < minimum {
                break;
            }
            if let Some((name, preceding)) = &previous {
                let overlapping =
                    preceding.low <= precedence.high && preceding.high >= precedence.low;
                if overlapping
                    && !(name == &token.text && (precedence.associative || name == "\\X"))
                {
                    return Err(self.error("Precedence conflict. Add parentheses"));
                }
            }
            self.advance()?;
            if precedence.low > 12 && self.at("-") {
                return Err(self.error("Precedence conflict before unary minus"));
            }
            let right = self.expression(precedence.high + 1)?;
            let is_chain = previous
                .as_ref()
                .is_some_and(|(name, _)| *name == token.text);
            let node = match token.text.as_str() {
                "/\\" | "\\/" => {
                    let mut operands = if is_chain {
                        match left.node {
                            ExpressionKind::And { operands, .. }
                            | ExpressionKind::Or { operands, .. } => operands,
                            _ => unreachable!(),
                        }
                    } else {
                        vec![left]
                    };
                    operands.push(right);
                    if token.text == "/\\" {
                        ExpressionKind::And {
                            operands,
                            is_bulleted: false,
                        }
                    } else {
                        ExpressionKind::Or {
                            operands,
                            is_bulleted: false,
                        }
                    }
                }
                "\\X" => {
                    let mut factors = if is_chain {
                        match left.node {
                            ExpressionKind::Product { factors } => factors,
                            _ => unreachable!(),
                        }
                    } else {
                        vec![left]
                    };
                    factors.push(right);
                    ExpressionKind::Product { factors }
                }
                _ => ExpressionKind::Apply {
                    operator: token.text.clone(),
                    args: vec![left, right],
                },
            };
            left = Expression {
                node,
                location: token.location,
            };
            previous = Some((token.text, precedence));
        }
        Ok(left)
    }
    fn prefix(&mut self) -> Result<Expression, ParseError> {
        let token = self.raw().clone();
        if !matches!(token.kind, Kind::Symbol | Kind::Keyword) {
            return self.primary();
        }
        let (low, high) = match token.text.as_str() {
            "~" => (4, 4),
            "-" => (12, 12),
            "[]" | "<>" | "ENABLED" | "UNCHANGED" => (4, 15),
            "SUBSET" | "UNION" | "DOMAIN" => (10, 13),
            "/\\" | "\\/" if token.raw != "\\land" && token.raw != "\\lor" => {
                return self.junction();
            }
            _ => return self.primary(),
        };
        self.advance()?;
        let expression = Box::new(self.expression(high + 1)?);
        let next_high = if self.at("'") {
            Some(15)
        } else {
            infix(&self.raw().text).map(|next| next.high)
        };
        if !self.blocked()
            && self.raw().kind == Kind::Symbol
            && next_high.is_some_and(|next| next >= low)
        {
            return Err(self.error("Precedence conflict after prefix operator"));
        }
        let node = match token.text.as_str() {
            "[]" => ExpressionKind::Always { expression },
            "<>" => ExpressionKind::Eventually { expression },
            "ENABLED" => ExpressionKind::Enabled { expression },
            "UNCHANGED" => ExpressionKind::Unchanged { expression },
            _ => ExpressionKind::Apply {
                operator: if token.text == "-" {
                    "-.".into()
                } else {
                    token.text
                },
                args: vec![*expression],
            },
        };
        Ok(Expression {
            node,
            location: token.location,
        })
    }
    fn junction(&mut self) -> Result<Expression, ParseError> {
        let bullet = self.advance()?;
        self.offside.push(bullet.location.column);
        let mut operands = vec![self.expression(0)?];
        while self.raw().kind == Kind::Symbol
            && self.raw().text == bullet.text
            && self.raw().location.column == bullet.location.column
        {
            self.cursor += 1;
            operands.push(self.expression(0)?);
        }
        self.offside.pop();
        let node = if bullet.text == "/\\" {
            ExpressionKind::And {
                operands,
                is_bulleted: true,
            }
        } else {
            ExpressionKind::Or {
                operands,
                is_bulleted: true,
            }
        };
        Ok(Expression {
            node,
            location: bullet.location,
        })
    }
    fn primary(&mut self) -> Result<Expression, ParseError> {
        let token = self.advance()?;
        let location = token.location;
        let node = match token.kind {
            Kind::Number => {
                let value = if token.text.starts_with('\\') {
                    let radix = match token.text.as_bytes()[1].to_ascii_lowercase() {
                        b'b' => 2,
                        b'o' => 8,
                        _ => 16,
                    };
                    u64::from_str_radix(&token.text[2..], radix)
                } else {
                    token.text.parse()
                }
                .ok()
                .filter(|value| *value <= 9_007_199_254_740_991)
                .ok_or(ParseError {
                    kind: ErrorKind::Unsupported,
                    message: "Numeral exceeds exact AST integer range".into(),
                    location,
                })?;
                ExpressionKind::Number { value }
            }
            Kind::Decimal => ExpressionKind::Decimal { text: token.text },
            Kind::String => ExpressionKind::String { value: token.text },
            Kind::Name => return self.named_expression(token),
            _ => match token.text.as_str() {
                "(" => {
                    let expression = self.expression(0)?;
                    self.expect(")")?;
                    return Ok(expression);
                }
                "[" => return self.bracket(location),
                "{" => return self.brace(location),
                "<<" => {
                    let mut elements = vec![];
                    if !self.at(">>") && !self.at(">>_") {
                        elements.push(self.expression(0)?);
                        while self.accept(",") {
                            elements.push(self.expression(0)?);
                        }
                    }
                    if self.accept(">>_") {
                        if elements.len() != 1 {
                            return Err(self.error("Angle action needs one expression"));
                        }
                        ExpressionKind::AngleAction {
                            action: Box::new(elements.remove(0)),
                            subscript: Box::new(self.subscript()?),
                        }
                    } else {
                        self.expect(">>")?;
                        ExpressionKind::Tuple { elements }
                    }
                }
                "@" => ExpressionKind::ExceptAt,
                "\\A" | "\\E" | "\\AA" | "\\EE" => {
                    let forall = token.text == "\\A" || token.text == "\\AA";
                    if self.bound_ahead() && token.text.len() == 2 {
                        let bounds = self.bounds()?;
                        self.expect(":")?;
                        let body = Box::new(self.expression(0)?);
                        if forall {
                            ExpressionKind::Forall { bounds, body }
                        } else {
                            ExpressionKind::Exists { bounds, body }
                        }
                    } else {
                        let mut names = vec![self.name()?.text];
                        while self.accept(",") {
                            names.push(self.name()?.text);
                        }
                        self.expect(":")?;
                        ExpressionKind::UnboundedQuantifier {
                            quantifier: if forall {
                                "Forall".into()
                            } else {
                                "Exists".into()
                            },
                            names,
                            is_temporal: token.text.len() == 3,
                            body: Box::new(self.expression(0)?),
                        }
                    }
                }
                "IF" => {
                    let condition = Box::new(self.expression(0)?);
                    self.expect("THEN")?;
                    let consequent = Box::new(self.expression(0)?);
                    self.expect("ELSE")?;
                    ExpressionKind::If {
                        condition,
                        consequent,
                        alternative: Box::new(self.expression(0)?),
                    }
                }
                "CASE" => {
                    let mut arms = vec![];
                    let mut other = None;
                    loop {
                        if self.accept("OTHER") {
                            self.expect("->")?;
                            other = Some(Box::new(self.expression(0)?));
                            break;
                        }
                        let condition = self.expression(0)?;
                        self.expect("->")?;
                        arms.push(CaseArm {
                            condition,
                            body: self.expression(0)?,
                        });
                        if !self.accept("[]") {
                            break;
                        }
                    }
                    ExpressionKind::Case { arms, other }
                }
                "LET" => {
                    let mut definitions = vec![];
                    while !self.accept("IN") {
                        definitions.push(self.definition()?);
                    }
                    ExpressionKind::Let {
                        definitions,
                        body: Box::new(self.expression(0)?),
                    }
                }
                "CHOOSE" => {
                    let pattern = self.pattern()?;
                    let set = if self.accept("\\in") {
                        Some(Box::new(self.expression(0)?))
                    } else {
                        None
                    };
                    self.expect(":")?;
                    ExpressionKind::Choose {
                        pattern,
                        set,
                        body: Box::new(self.expression(0)?),
                    }
                }
                "WF_" | "SF_" => {
                    let subscript = Box::new(self.subscript()?);
                    self.expect("(")?;
                    let action = Box::new(self.expression(0)?);
                    self.expect(")")?;
                    if token.text == "WF_" {
                        ExpressionKind::WeakFairness { subscript, action }
                    } else {
                        ExpressionKind::StrongFairness { subscript, action }
                    }
                }
                "LAMBDA" | "INSTANCE" | "PROOF" | "ASSUME" => {
                    return Err(ParseError {
                        kind: ErrorKind::Unsupported,
                        message: format!("Unsupported syntax: {}", token.text),
                        location,
                    });
                }
                _ => {
                    return Err(ParseError {
                        kind: ErrorKind::Syntax,
                        message: format!("Unexpected token {:?}", token.raw),
                        location,
                    });
                }
            },
        };
        Ok(Expression { node, location })
    }
    fn subscript(&mut self) -> Result<Expression, ParseError> {
        if self.raw().kind == Kind::Name {
            let token = self.advance()?;
            return Ok(Expression {
                node: ExpressionKind::Identifier { name: token.text },
                location: token.location,
            });
        }
        if !["<<", "(", "{", "["].iter().any(|text| self.at(text)) {
            return Err(self.error("Expected action subscript"));
        }
        self.primary()
    }
    fn expression_list(&mut self, closing: &str) -> Result<Vec<Expression>, ParseError> {
        let mut expressions = vec![self.expression(0)?];
        while self.accept(",") {
            expressions.push(self.expression(0)?);
        }
        self.expect(closing)?;
        Ok(expressions)
    }
    fn bound_ahead(&self) -> bool {
        let mut offset = 0;
        if self.peek(offset).text == "<<" {
            offset += 1;
            while self.peek(offset).kind == Kind::Name || self.peek(offset).text == "," {
                offset += 1;
            }
            return self.peek(offset).text == ">>" && self.peek(offset + 1).text == "\\in";
        }
        while self.peek(offset).kind == Kind::Name {
            if self.peek(offset + 1).text == "\\in" {
                return true;
            }
            if self.peek(offset + 1).text != "," {
                return false;
            }
            offset += 2;
        }
        false
    }
    fn pattern(&mut self) -> Result<Pattern, ParseError> {
        if !self.accept("<<") {
            return Ok(Pattern::Name {
                name: self.name()?.text,
            });
        }
        let mut names = vec![];
        if !self.at(">>") {
            names.push(self.name()?.text);
            while self.accept(",") {
                names.push(self.name()?.text);
            }
        }
        self.expect(">>")?;
        Ok(Pattern::Tuple { names })
    }
    fn bounds(&mut self) -> Result<Vec<Bound>, ParseError> {
        let mut bounds = vec![];
        loop {
            let mut patterns = vec![self.pattern()?];
            while self.accept(",") {
                patterns.push(self.pattern()?);
            }
            self.expect("\\in")?;
            let set = self.expression(0)?;
            bounds.extend(patterns.into_iter().map(|pattern| Bound {
                pattern,
                set: set.clone(),
            }));
            if !self.accept(",") {
                break;
            }
        }
        Ok(bounds)
    }
    fn bracket(&mut self, location: Location) -> Result<Expression, ParseError> {
        let node = if matches!(self.raw().kind, Kind::Name | Kind::Keyword)
            && matches!(self.peek(1).text.as_str(), "|->" | ":")
        {
            let record = self.peek(1).text == "|->";
            let mut fields = vec![];
            loop {
                let name = self.advance()?;
                if !matches!(name.kind, Kind::Name | Kind::Keyword) {
                    return Err(self.error("Expected field name"));
                }
                self.expect(if record { "|->" } else { ":" })?;
                fields.push(RecordField {
                    name: name.text,
                    value: self.expression(0)?,
                    location: name.location,
                });
                if !self.accept(",") {
                    break;
                }
            }
            self.expect("]")?;
            if record {
                ExpressionKind::Record { fields }
            } else {
                ExpressionKind::RecordSet { fields }
            }
        } else if self.bound_ahead() {
            let bounds = self.bounds()?;
            self.expect("|->")?;
            let body = Box::new(self.expression(0)?);
            self.expect("]")?;
            ExpressionKind::FunctionConstruction { bounds, body }
        } else {
            let expression = Box::new(self.expression(0)?);
            if self.accept("->") {
                let range = Box::new(self.expression(0)?);
                self.expect("]")?;
                ExpressionKind::FunctionSet {
                    domain: expression,
                    range,
                }
            } else if self.accept("EXCEPT") {
                let mut updates = vec![];
                loop {
                    self.expect("!")?;
                    let mut path = vec![];
                    while self.at(".") || self.at("[") {
                        if self.accept(".") {
                            let field = self.advance()?;
                            if !matches!(field.kind, Kind::Name | Kind::Keyword)
                                && field.text != "_"
                            {
                                return Err(self.error("Expected EXCEPT field"));
                            }
                            path.push(ExceptPath::Field { name: field.text });
                        } else {
                            self.expect("[")?;
                            path.push(ExceptPath::Index {
                                args: self.expression_list("]")?,
                            });
                        }
                    }
                    if path.is_empty() {
                        return Err(self.error("EXCEPT needs an index or field path"));
                    }
                    self.expect("=")?;
                    updates.push(ExceptUpdate {
                        path,
                        value: self.expression(0)?,
                    });
                    if !self.accept(",") {
                        break;
                    }
                }
                self.expect("]")?;
                ExpressionKind::Except {
                    function: expression,
                    updates,
                }
            } else if self.accept("]_") {
                ExpressionKind::BoxAction {
                    action: expression,
                    subscript: Box::new(self.subscript()?),
                }
            } else {
                let mut args = vec![*expression];
                while self.accept(",") {
                    args.push(self.expression(0)?);
                }
                self.expect("]")?;
                ExpressionKind::Bracket { args }
            }
        };
        Ok(Expression { node, location })
    }
    fn named_expression(&mut self, token: Token) -> Result<Expression, ParseError> {
        let mut name = token.text;
        while self.accept("!") {
            name.push('!');
            name.push_str(&self.name()?.text);
        }
        let node = if self.accept("::") {
            ExpressionKind::Label {
                name,
                params: vec![],
                body: Box::new(self.expression(0)?),
            }
        } else if self.accept("(") {
            ExpressionKind::Apply {
                operator: name,
                args: self.expression_list(")")?,
            }
        } else {
            ExpressionKind::Identifier { name }
        };
        Ok(Expression {
            node,
            location: token.location,
        })
    }

    fn brace(&mut self, location: Location) -> Result<Expression, ParseError> {
        if self.accept("}") {
            return Ok(Expression {
                node: ExpressionKind::SetEnumeration { elements: vec![] },
                location,
            });
        }
        if self.bound_ahead() && (self.peek(1).text == "\\in" || self.at("<<")) {
            let checkpoint = self.cursor;
            let pattern = self.pattern()?;
            self.expect("\\in")?;
            let set = self.expression(0)?;
            if self.accept(":") {
                let predicate = Box::new(self.expression(0)?);
                self.expect("}")?;
                return Ok(Expression {
                    node: ExpressionKind::SetFilter {
                        bound: Box::new(Bound { pattern, set }),
                        predicate,
                    },
                    location,
                });
            }
            self.cursor = checkpoint;
        }
        let first = self.expression(0)?;
        let node = if self.accept(":") {
            let bounds = self.bounds()?;
            self.expect("}")?;
            ExpressionKind::SetMap {
                expression: Box::new(first),
                bounds,
            }
        } else {
            let mut elements = vec![first];
            while self.accept(",") {
                elements.push(self.expression(0)?);
            }
            self.expect("}")?;
            ExpressionKind::SetEnumeration { elements }
        };
        Ok(Expression { node, location })
    }
    fn declaration(&mut self) -> Result<Declaration, ParseError> {
        let token = self.name()?;
        let mut arity = 0;
        if self.accept("(") {
            self.expect("_")?;
            arity += 1;
            while self.accept(",") {
                self.expect("_")?;
                arity += 1;
            }
            self.expect(")")?;
        }
        Ok(Declaration {
            name: token.text,
            arity,
            location: token.location,
        })
    }
    fn definition(&mut self) -> Result<Definition, ParseError> {
        let is_local = self.accept("LOCAL");
        if self.at("INSTANCE") || self.at("RECURSIVE") {
            return Err(self.unsupported("INSTANCE and RECURSIVE declarations are not supported"));
        }
        let name = self.name()?;
        let head = if self.accept("[") {
            let bounds = self.bounds()?;
            self.expect("]")?;
            DefinitionKind::Function { bounds }
        } else {
            let mut params = vec![];
            if self.accept("(") {
                params.push(self.declaration()?);
                while self.accept(",") {
                    params.push(self.declaration()?);
                }
                self.expect(")")?;
            }
            DefinitionKind::Operator { params }
        };
        self.expect("==")?;
        let body = self.expression(0)?;
        Ok(Definition {
            name: name.text,
            head,
            body,
            is_local,
            is_recursive: false,
            location: name.location,
        })
    }
    fn instance(&mut self) -> Result<Instance, ParseError> {
        let location = self.raw().location;
        let is_local = self.accept("LOCAL");
        let name = if self.at("INSTANCE") {
            None
        } else {
            let name = self.name()?.text;
            self.expect("==")?;
            Some(name)
        };
        self.expect("INSTANCE")?;
        let module = self.name()?.text;
        let mut substitutions: Vec<InstanceSubstitution> = vec![];
        if self.accept("WITH") {
            loop {
                let name = self.name()?.text;
                if substitutions.iter().any(|entry| entry.name == name) {
                    return Err(self.error("Duplicate INSTANCE substitution"));
                }
                self.expect("<-")?;
                substitutions.push(InstanceSubstitution {
                    name,
                    expression: self.expression(0)?,
                });
                if !self.accept(",") {
                    break;
                }
            }
        }
        Ok(Instance {
            name,
            module,
            substitutions,
            is_local,
            location,
        })
    }
    fn module(&mut self) -> Result<Module, ParseError> {
        let location = self.expect("----")?.location;
        self.expect("MODULE")?;
        let name = self.name()?.text;
        self.expect("----")?;
        let mut module = Module {
            name,
            extends: vec![],
            constants: vec![],
            variables: vec![],
            definitions: vec![],
            instances: vec![],
            assumptions: vec![],
            theorems: vec![],
            location,
        };
        if self.accept("EXTENDS") {
            module.extends.push(self.name()?.text);
            while self.accept(",") {
                module.extends.push(self.name()?.text);
            }
        }
        while !self.accept("====") {
            if self.accept("----") {
                continue;
            }
            if self.at("CONSTANT")
                || self.at("CONSTANTS")
                || self.at("VARIABLE")
                || self.at("VARIABLES")
            {
                let constants = self.advance()?.text.starts_with("CONSTANT");
                let declarations = if constants {
                    &mut module.constants
                } else {
                    &mut module.variables
                };
                declarations.push(self.declaration()?);
                while self.accept(",") {
                    declarations.push(self.declaration()?);
                }
            } else if self.at("ASSUME") || self.at("ASSUMPTION") || self.at("AXIOM") {
                let keyword = self.advance()?;
                let name = if self.peek(1).text == "==" {
                    let name = self.name()?.text;
                    self.expect("==")?;
                    Some(name)
                } else {
                    None
                };
                module.assumptions.push(Assumption {
                    name,
                    expression: self.expression(0)?,
                    is_axiom: keyword.text == "AXIOM",
                    location: keyword.location,
                });
            } else if ["THEOREM", "LEMMA", "COROLLARY", "PROPOSITION"]
                .iter()
                .any(|text| self.at(text))
            {
                let keyword = self.advance()?;
                let name = if self.peek(1).text == "==" {
                    let name = self.name()?.text;
                    self.expect("==")?;
                    Some(name)
                } else {
                    None
                };
                module.theorems.push(Theorem {
                    name,
                    statement: self.expression(0)?,
                    location: keyword.location,
                });
            } else if self.at("INSTANCE")
                || (self.at("LOCAL") && self.peek(1).text == "INSTANCE")
                || (self.raw().kind == Kind::Name
                    && self.peek(1).text == "=="
                    && self.peek(2).text == "INSTANCE")
                || (self.at("LOCAL")
                    && self.peek(2).text == "=="
                    && self.peek(3).text == "INSTANCE")
            {
                module.instances.push(self.instance()?);
            } else if [
                "MODULE",
                "INSTANCE",
                "RECURSIVE",
                "PROOF",
                "BY",
                "OBVIOUS",
                "OMITTED",
                "USE",
                "HIDE",
            ]
            .iter()
            .any(|text| self.at(text))
            {
                return Err(self.unsupported("Nested modules, instances, recursive declarations and proofs are not supported"));
            } else {
                module.definitions.push(self.definition()?);
            }
        }
        Ok(module)
    }
}

pub fn parse_expression(source: &str) -> Result<Expression, ParseError> {
    let mut parser = Parser::new(tokenize(source, 0, false)?);
    let expression = parser.expression(0)?;
    if parser.raw().kind != Kind::End {
        return Err(parser.error("Unexpected token after expression"));
    }
    Ok(expression)
}
pub fn parse_module(source: &str) -> Result<Module, ParseError> {
    let start = source
        .match_indices("----")
        .find_map(|(index, _)| {
            let rest = source[index..].trim_start_matches('-').trim_start();
            rest.strip_prefix("MODULE")
                .filter(|suffix| suffix.starts_with(char::is_whitespace))
                .map(|_| index)
        })
        .ok_or(ParseError {
            kind: ErrorKind::Syntax,
            message: "Missing MODULE header".into(),
            location: Location {
                start: 0,
                end: 0,
                line: 1,
                column: 1,
            },
        })?;
    Parser::new(tokenize(source, start, true)?).module()
}
