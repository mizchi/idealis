use crate::ast::{
    Assumption, Bound, CaseArm, Declaration, Definition, DefinitionKind, ExceptPath, ExceptUpdate,
    Expression, ExpressionKind, Instance, InstanceSubstitution, Location, Module, Pattern,
    RecordField, Selector, Theorem,
};
use crate::lexer::{Kind, Token, tokenize};
use crate::{ErrorKind, ParseError, SyntaxVersion};
mod proof;

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
        "$" => (9, 13, true),
        "??" => (9, 13, true),
        "##" => (9, 13, true),
        "!!" => (9, 13, false),
        "|-" => (5, 5, false),
        "|=" => (5, 5, false),
        "-|" => (5, 5, false),
        "=|" => (5, 5, false),
        "::=" => (5, 5, false),
        "\\oplus" => (10, 10, true),
        "\\ominus" => (11, 11, true),
        "\\odot" => (13, 13, true),
        "\\oslash" => (13, 13, false),
        "\\otimes" => (13, 13, true),
        "\\uplus" => (9, 13, true),
        "\\sqcap" => (9, 13, true),
        "\\sqcup" => (9, 13, true),
        "\\wr" => (9, 14, false),
        "\\star" => (13, 13, true),
        "\\bigcirc" => (13, 13, true),
        "\\bullet" => (13, 13, true),
        "\\prec" => (5, 5, false),
        "\\succ" => (5, 5, false),
        "\\preceq" => (5, 5, false),
        "\\succeq" => (5, 5, false),
        "\\sim" => (5, 5, false),
        "\\simeq" => (5, 5, false),
        "\\ll" => (5, 5, false),
        "\\gg" => (5, 5, false),
        "\\asymp" => (5, 5, false),
        "\\subset" => (5, 5, false),
        "\\supset" => (5, 5, false),
        "\\supseteq" => (5, 5, false),
        "\\approx" => (5, 5, false),
        "\\cong" => (5, 5, false),
        "\\sqsubset" => (5, 5, false),
        "\\sqsubseteq" => (5, 5, false),
        "\\sqsupset" => (5, 5, false),
        "\\sqsupseteq" => (5, 5, false),
        "\\doteq" => (5, 5, false),
        "\\propto" => (5, 5, false),
        "$$" => (9, 13, true),
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
            | ExpressionKind::Label { body, .. }
            | ExpressionKind::Lambda { body, .. } => vec![body],
            ExpressionKind::Selection {
                expression,
                selectors,
            } => std::iter::once(expression.as_ref())
                .chain(selectors.iter().flat_map(|selector| match selector {
                    Selector::Name { args, .. } | Selector::Arguments { args } => args.as_slice(),
                    _ => &[],
                }))
                .collect(),
            ExpressionKind::AssumeProve {
                assumptions, prove, ..
            } => assumptions
                .iter()
                .filter_map(|assumption| match assumption {
                    crate::ast::ProofAssumption::Expression { expression } => Some(expression),
                    crate::ast::ProofAssumption::New { set, .. } => set.as_deref(),
                })
                .chain(std::iter::once(prove.as_ref()))
                .collect(),
            ExpressionKind::Let { definitions, body }
            | ExpressionKind::LetModule {
                definitions, body, ..
            } => {
                let mut children = vec![body.as_ref()];
                for definition in definitions {
                    children.push(&definition.body);
                    if let DefinitionKind::Function { bounds } = &definition.head {
                        children.extend(bounds.iter().map(|bound| &bound.set));
                    }
                }
                if let ExpressionKind::LetModule { instances, .. } = &expression.node {
                    children.extend(instances.iter().flat_map(|instance| {
                        instance.substitutions.iter().map(|entry| &entry.expression)
                    }));
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
            | ExpressionKind::Integer { .. }
            | ExpressionKind::Decimal { .. }
            | ExpressionKind::String { .. }
            | ExpressionKind::Identifier { .. }
            | ExpressionKind::ExceptAt => vec![],
        };
        pending.extend(children.into_iter().map(|child| (child, depth + 1)));
    }
    true
}

fn prefix_declaration(operator: &str) -> bool {
    matches!(
        operator,
        "~" | "-." | "[]" | "<>" | "ENABLED" | "UNCHANGED" | "SUBSET" | "UNION" | "DOMAIN"
    )
}

fn postfix(operator: &str) -> bool {
    matches!(operator, "'" | "^+" | "^*" | "^#")
}

fn symbolic(token: &Token) -> bool {
    matches!(token.kind, Kind::Symbol | Kind::Keyword)
        && (prefix_declaration(&token.text) || postfix(&token.text) || infix(&token.text).is_some())
}

fn parameter(token: Token) -> Declaration {
    Declaration {
        name: token.text,
        arity: 0,
        location: token.location,
    }
}

pub(crate) struct Parser {
    pub tokens: Vec<Token>,
    pub cursor: usize,
    offside: Vec<usize>,
    depth: usize,
    syntax: SyntaxVersion,
}
impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            cursor: 0,
            offside: vec![],
            depth: 0,
            syntax: SyntaxVersion::default(),
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
        let start = self.cursor;
        self.depth += 1;
        let result = self.expression_inner(minimum);
        self.depth -= 1;
        if let Ok(expression) = &result
            && self.label_changes_precedence(expression, minimum, start)
        {
            return Err(ParseError {
                kind: ErrorKind::Syntax,
                message: "Label changes operator precedence. Add parentheses".into(),
                location: self.tokens[start].location,
            });
        }
        result
    }
    fn label_changes_precedence(&self, expression: &Expression, minimum: u8, start: usize) -> bool {
        if minimum == 0 || self.tokens[start].kind != Kind::Name {
            return false;
        }
        let ExpressionKind::Label { body, .. } = &expression.node else {
            return false;
        };
        let precedence = match &body.node {
            ExpressionKind::Apply { operator, args } if args.len() == 2 => {
                infix(operator).map(|precedence| precedence.low)
            }
            ExpressionKind::And {
                is_bulleted: false, ..
            }
            | ExpressionKind::Or {
                is_bulleted: false, ..
            } => Some(3),
            ExpressionKind::Prime { .. } => Some(15),
            ExpressionKind::Apply { operator, args }
                if args.len() == 1 && matches!(operator.as_str(), "^+" | "^*" | "^#") =>
            {
                Some(15)
            }
            _ => None,
        };
        if precedence.is_none_or(|precedence| precedence >= minimum) {
            return false;
        }
        let body_start = start
            + self.tokens[start..self.cursor]
                .iter()
                .position(|token| token.kind == Kind::Symbol && token.text == "::")
                .expect("label separator")
            + 1;
        if self.tokens[body_start].text != "(" {
            return true;
        }
        let mut nesting = 0;
        self.tokens[body_start..self.cursor]
            .iter()
            .position(|token| {
                if token.kind == Kind::Symbol {
                    if token.text == "(" {
                        nesting += 1;
                    } else if token.text == ")" {
                        nesting -= 1;
                    }
                }
                nesting == 0
            })
            != Some(self.cursor - body_start - 1)
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
            if self.accept("!") {
                if !matches!(
                    left.node,
                    ExpressionKind::Identifier { .. }
                        | ExpressionKind::Apply { .. }
                        | ExpressionKind::Selection { .. }
                ) {
                    return Err(self.error("Expected operator or subexpression name before !"));
                }
                let selector = self.selector()?;
                if let ExpressionKind::Selection { selectors, .. } = &mut left.node {
                    selectors.push(selector);
                } else {
                    left = Expression {
                        location: left.location,
                        node: ExpressionKind::Selection {
                            expression: Box::new(left),
                            selectors: vec![selector],
                        },
                    };
                }
                continue;
            }
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
            "SUBSET" | "UNION" if self.syntax == SyntaxVersion::Tlc180 => (8, 8),
            "DOMAIN" if self.syntax == SyntaxVersion::Tlc180 => (9, 9),
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
        if self.accept("(") {
            let expression = self.expression(0)?;
            self.expect(")")?;
            return Ok(expression);
        }
        if matches!(self.raw().kind, Kind::Name | Kind::Step)
            || symbolic(self.raw())
                && (!prefix_declaration(&self.raw().text) || self.raw().text == "-.")
        {
            let token = self.advance()?;
            return self.named_expression(token);
        }
        if self.at("<<") {
            let location = self.advance()?.location;
            return self.tuple(location);
        }
        if self.at("{") {
            let location = self.advance()?.location;
            return self.brace(location);
        }
        if self.at("[") {
            let location = self.advance()?.location;
            return self.bracket(location);
        }
        self.primary_inner()
    }

    fn primary_inner(&mut self) -> Result<Expression, ParseError> {
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
                .filter(|value| *value <= 9_007_199_254_740_991);
                match value {
                    Some(value) => ExpressionKind::Number { value },
                    None => ExpressionKind::Integer { text: token.text },
                }
            }
            Kind::Decimal => ExpressionKind::Decimal { text: token.text },
            Kind::String => ExpressionKind::String { value: token.text },
            _ => match token.text.as_str() {
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
                    return self.let_expression(location);
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
                    return self.fairness(token);
                }
                "LAMBDA" => {
                    return Err(ParseError {
                        kind: ErrorKind::Syntax,
                        message:
                            "LAMBDA is allowed only as an operator argument or WITH replacement"
                                .into(),
                        location,
                    });
                }
                "INSTANCE" | "PROOF" | "ASSUME" => {
                    return Err(ParseError {
                        kind: ErrorKind::Syntax,
                        message: format!("{} is not an expression", token.text),
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
    fn tuple(&mut self, location: Location) -> Result<Expression, ParseError> {
        let node = {
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
        };
        Ok(Expression { node, location })
    }

    fn subscript(&mut self) -> Result<Expression, ParseError> {
        if !matches!(self.raw().kind, Kind::Name | Kind::Step)
            && !["<<", "(", "{", "["].iter().any(|text| self.at(text))
        {
            return Err(self.error("Expected action subscript"));
        }
        self.expression(16)
    }

    fn fairness(&mut self, token: Token) -> Result<Expression, ParseError> {
        let mut subscript = self.subscript()?;
        let action = if self.accept("(") {
            let expression = self.expression(0)?;
            self.expect(")")?;
            expression
        } else {
            match &mut subscript.node {
                ExpressionKind::Apply { operator, args } if args.len() == 1 => {
                    let action = args.remove(0);
                    subscript.node = ExpressionKind::Identifier {
                        name: operator.clone(),
                    };
                    action
                }
                ExpressionKind::Selection { selectors, .. } => {
                    let Some(Selector::Name { args, .. }) = selectors.last_mut() else {
                        return Err(self.error("Expected fairness action"));
                    };
                    if args.len() != 1 {
                        return Err(self.error("Fairness requires one action argument"));
                    }
                    args.remove(0)
                }
                _ => return Err(self.error("Expected fairness action")),
            }
        };
        let subscript = Box::new(subscript);
        let action = Box::new(action);
        Ok(Expression {
            location: token.location,
            node: if token.text == "WF_" {
                ExpressionKind::WeakFairness { subscript, action }
            } else {
                ExpressionKind::StrongFairness { subscript, action }
            },
        })
    }
    fn expression_list(&mut self, closing: &str) -> Result<Vec<Expression>, ParseError> {
        let mut expressions = vec![self.expression(0)?];
        while self.accept(",") {
            expressions.push(self.expression(0)?);
        }
        self.expect(closing)?;
        Ok(expressions)
    }
    fn definition_start(&self, offset: usize) -> bool {
        let start = self.cursor + offset;
        let Some(first) = self.tokens.get(start) else {
            return false;
        };
        if first.kind != Kind::Name && !symbolic(first) {
            return false;
        }
        let Some(next) = self.tokens.get(start + 1) else {
            return false;
        };
        if next.text == "==" {
            return first.kind == Kind::Name;
        }
        if matches!(next.text.as_str(), "(" | "[") && first.kind == Kind::Name {
            let mut depth = 0;
            for (index, token) in self.tokens.iter().enumerate().skip(start + 1) {
                if matches!(token.text.as_str(), "(" | "[") {
                    depth += 1;
                }
                if matches!(token.text.as_str(), ")" | "]") {
                    depth -= 1;
                }
                if depth == 0 {
                    return self
                        .tokens
                        .get(index + 1)
                        .is_some_and(|token| token.text == "==");
                }
            }
            return false;
        }
        let offset = if first.kind == Kind::Name && infix(&next.text).is_some() {
            3
        } else {
            2
        };
        self.tokens
            .get(start + offset)
            .is_some_and(|token| token.text == "==")
    }

    fn operator_value(&mut self) -> Result<Expression, ParseError> {
        let token = self.advance()?;
        if matches!(token.raw.as_str(), "\\X" | "\\times") {
            return Err(self.error("Cartesian product cannot be an operator argument"));
        }
        Ok(Expression {
            location: token.location,
            node: ExpressionKind::Identifier { name: token.text },
        })
    }

    fn operator_argument(&mut self) -> Result<Expression, ParseError> {
        if self.at("LAMBDA") {
            return self.lambda();
        }
        if symbolic(self.raw()) && matches!(self.peek(1).text.as_str(), "," | ")") {
            return self.operator_value();
        }
        self.expression(0)
    }

    fn lambda(&mut self) -> Result<Expression, ParseError> {
        let location = self.expect("LAMBDA")?.location;
        let mut params = vec![parameter(self.name()?)];
        while self.accept(",") {
            params.push(parameter(self.name()?));
        }
        self.expect(":")?;
        Ok(Expression {
            location,
            node: ExpressionKind::Lambda {
                params,
                body: Box::new(self.expression(0)?),
            },
        })
    }
    fn operator_arguments(&mut self) -> Result<Vec<Expression>, ParseError> {
        let mut expressions = vec![self.operator_argument()?];
        while self.accept(",") {
            expressions.push(self.operator_argument()?);
        }
        self.expect(")")?;
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
        while self.at("!") && (self.peek(1).kind == Kind::Name || symbolic(self.peek(1))) {
            self.advance()?;
            name.push('!');
            let part = if symbolic(self.raw()) {
                self.advance()?
            } else {
                self.name()?
            };
            name.push_str(&part.text);
        }
        let node = if self.accept("::") {
            ExpressionKind::Label {
                name,
                params: vec![],
                body: Box::new(self.expression(0)?),
            }
        } else if self.accept("(") {
            let start = self.cursor;
            let args = self.operator_arguments()?;
            if self.accept("::") {
                let tokens = &self.tokens[start..self.cursor - 2];
                if tokens.iter().enumerate().any(|(index, token)| {
                    if index % 2 == 0 {
                        token.kind != Kind::Name
                    } else {
                        token.text != ","
                    }
                }) || tokens.len() != args.len() * 2 - 1
                {
                    return Err(self.error("Label parameters must be identifiers"));
                }
                let params = tokens
                    .iter()
                    .step_by(2)
                    .map(|token| token.text.clone())
                    .collect();
                ExpressionKind::Label {
                    name,
                    params,
                    body: Box::new(self.expression(0)?),
                }
            } else {
                ExpressionKind::Apply {
                    operator: name,
                    args,
                }
            }
        } else {
            ExpressionKind::Identifier { name }
        };
        Ok(Expression {
            node,
            location: token.location,
        })
    }

    fn selector(&mut self) -> Result<Selector, ParseError> {
        let token = self.advance()?;
        Ok(match token.text.as_str() {
            "<<" => Selector::Left,
            ">>" => Selector::Right,
            ":" => Selector::Body,
            "@" => Selector::Bound,
            "(" => Selector::Arguments {
                args: self.operator_arguments()?,
            },
            _ if token.kind == Kind::Number => Selector::Index {
                index: token
                    .text
                    .parse()
                    .map_err(|_| self.error("Expected decimal selector index"))?,
            },
            _ if token.kind == Kind::Name || symbolic(&token) => Selector::Name {
                name: token.text,
                args: if self.accept("(") {
                    self.operator_arguments()?
                } else {
                    vec![]
                },
            },
            _ => return Err(self.error("Expected subexpression selector")),
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
        if symbolic(self.raw()) && prefix_declaration(&self.raw().text) {
            let token = self.advance()?;
            self.expect("_")?;
            return Ok(Declaration {
                name: token.text,
                arity: 1,
                location: token.location,
            });
        }
        if self.accept("_") {
            if !symbolic(self.raw()) {
                return Err(self.error("Expected infix or postfix operator"));
            }
            let token = self.advance()?;
            let arity = if postfix(&token.text) {
                1
            } else if infix(&token.text).is_some() {
                self.expect("_")?;
                2
            } else {
                return Err(self.error("Expected infix or postfix operator"));
            };
            return Ok(Declaration {
                name: token.text,
                arity,
                location: token.location,
            });
        }
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
        let name = if symbolic(self.raw()) && prefix_declaration(&self.raw().text) {
            self.advance()?
        } else {
            self.name()?
        };
        let (name, head) = if symbolic(&name) {
            (
                name,
                DefinitionKind::Operator {
                    params: vec![parameter(self.name()?)],
                },
            )
        } else if symbolic(self.raw())
            && (postfix(&self.raw().text) || infix(&self.raw().text).is_some())
        {
            let operator = self.advance()?;
            let mut params = vec![parameter(name)];
            if !postfix(&operator.text) {
                params.push(parameter(self.name()?));
            }
            (operator, DefinitionKind::Operator { params })
        } else if self.accept("[") {
            let bounds = self.bounds()?;
            self.expect("]")?;
            (name, DefinitionKind::Function { bounds })
        } else {
            let mut params = vec![];
            if self.accept("(") {
                params.push(self.declaration()?);
                while self.accept(",") {
                    params.push(self.declaration()?);
                }
                self.expect(")")?;
            }
            (name, DefinitionKind::Operator { params })
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
        let (name, params) = if self.at("INSTANCE") {
            (None, vec![])
        } else {
            let name = self.name()?.text;
            let mut params = vec![];
            if self.accept("(") {
                params.push(self.declaration()?);
                while self.accept(",") {
                    params.push(self.declaration()?);
                }
                self.expect(")")?;
            }
            self.expect("==")?;
            (Some(name), params)
        };
        self.expect("INSTANCE")?;
        let module = self.name()?.text;
        let mut substitutions: Vec<InstanceSubstitution> = vec![];
        if self.accept("WITH") {
            loop {
                let name = if symbolic(self.raw()) {
                    self.advance()?.text
                } else {
                    self.name()?.text
                };
                if substitutions.iter().any(|entry| entry.name == name) {
                    return Err(self.error("Duplicate INSTANCE substitution"));
                }
                self.expect("<-")?;
                let expression = if self.at("LAMBDA") {
                    self.lambda()?
                } else if symbolic(self.raw())
                    && (matches!(
                        self.peek(1).text.as_str(),
                        "," | "===="
                            | "----"
                            | "LOCAL"
                            | "INSTANCE"
                            | "THEOREM"
                            | "LEMMA"
                            | "COROLLARY"
                            | "PROPOSITION"
                            | "ASSUME"
                            | "ASSUMPTION"
                            | "AXIOM"
                            | "CONSTANT"
                            | "CONSTANTS"
                            | "VARIABLE"
                            | "VARIABLES"
                            | "RECURSIVE"
                    ) || self.definition_start(1))
                {
                    self.operator_value()?
                } else {
                    self.expression(0)?
                };
                substitutions.push(InstanceSubstitution { name, expression });
                if !self.accept(",") {
                    break;
                }
            }
        }
        Ok(Instance {
            name,
            params,
            module,
            substitutions,
            is_local,
            location,
        })
    }

    fn instance_ahead(&self) -> bool {
        let mut index = self.cursor + usize::from(self.at("LOCAL"));
        if self
            .tokens
            .get(index)
            .is_some_and(|token| token.text == "INSTANCE")
        {
            return true;
        }
        if !self
            .tokens
            .get(index)
            .is_some_and(|token| token.kind == Kind::Name)
        {
            return false;
        }
        index += 1;
        if self
            .tokens
            .get(index)
            .is_some_and(|token| token.text == "(")
        {
            let mut depth = 0usize;
            while let Some(token) = self.tokens.get(index) {
                if token.text == "(" {
                    depth += 1;
                }
                if token.text == ")" {
                    depth -= 1;
                }
                index += 1;
                if depth == 0 {
                    break;
                }
            }
        }
        self.tokens
            .get(index)
            .is_some_and(|token| token.text == "==")
            && self
                .tokens
                .get(index + 1)
                .is_some_and(|token| token.text == "INSTANCE")
    }

    fn recursive(&mut self) -> Result<Vec<Declaration>, ParseError> {
        self.expect("RECURSIVE")?;
        let mut declarations = vec![self.declaration()?];
        while self.accept(",") {
            declarations.push(self.declaration()?);
        }
        Ok(declarations)
    }

    fn let_expression(&mut self, location: Location) -> Result<Expression, ParseError> {
        let mut definitions = vec![];
        let mut instances = vec![];
        let mut recursive = vec![];
        while !self.accept("IN") {
            if self.at("RECURSIVE") {
                recursive.extend(self.recursive()?);
            } else if self.instance_ahead() {
                if self.at("INSTANCE") || self.at("LOCAL") {
                    return Err(self.error("LET requires a named module definition"));
                }
                instances.push(self.instance()?);
            } else {
                definitions.push(self.definition()?);
            }
        }
        if definitions.is_empty() && instances.is_empty() && recursive.is_empty() {
            return Err(self.error("LET requires a definition"));
        }
        for definition in &mut definitions {
            definition.is_recursive = recursive
                .iter()
                .any(|declaration| declaration.name == definition.name);
        }
        let body = Box::new(self.expression(0)?);
        let node = if instances.is_empty() && recursive.is_empty() {
            ExpressionKind::Let { definitions, body }
        } else {
            ExpressionKind::LetModule {
                definitions,
                instances,
                recursive,
                body,
            }
        };
        Ok(Expression { node, location })
    }

    fn module(&mut self) -> Result<Module, ParseError> {
        if self.depth >= crate::MAX_NESTING {
            return Err(self.unsupported("Module nesting exceeds 64"));
        }
        self.depth += 1;
        let result = self.module_inner();
        self.depth -= 1;
        result
    }

    fn module_inner(&mut self) -> Result<Module, ParseError> {
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
            recursive: vec![],
            modules: vec![],
            directives: vec![],
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
            if self.at("----") && self.peek(1).text == "MODULE" {
                module.modules.push(self.module()?);
                continue;
            }
            if self.accept("----") {
                continue;
            }
            if self.accept("CONSTANT") || self.accept("CONSTANTS") {
                module.constants.push(self.declaration()?);
                while self.accept(",") {
                    module.constants.push(self.declaration()?);
                }
            } else if self.accept("VARIABLE") || self.accept("VARIABLES") {
                module.variables.push(parameter(self.name()?));
                while self.accept(",") {
                    module.variables.push(parameter(self.name()?));
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
                    statement: self.statement()?,
                    proof: if self.begins_proof(None) {
                        Some(self.proof(None)?)
                    } else {
                        None
                    },
                    location: keyword.location,
                });
            } else if self.instance_ahead() {
                module.instances.push(self.instance()?);
            } else if self.at("RECURSIVE") {
                module.recursive.extend(self.recursive()?);
            } else if self.at("USE") || self.at("HIDE") {
                let token = self.advance()?;
                module.directives.push(crate::ast::ProofDirective {
                    is_hide: token.text == "HIDE",
                    usage: self.use_body(token.text != "HIDE")?,
                    location: token.location,
                });
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
                return Err(self.error("Unexpected module declaration or proof outside a theorem"));
            } else {
                module.definitions.push(self.definition()?);
            }
        }
        for definition in &mut module.definitions {
            definition.is_recursive = module
                .recursive
                .iter()
                .any(|declaration| declaration.name == definition.name);
        }
        Ok(module)
    }
}

pub fn parse_expression(source: &str) -> Result<Expression, ParseError> {
    parse_expression_with_version(source, SyntaxVersion::default())
}
pub fn parse_expression_with_version(
    source: &str,
    syntax: SyntaxVersion,
) -> Result<Expression, ParseError> {
    let mut parser = Parser::new(tokenize(source, 0, false)?);
    parser.syntax = syntax;
    let expression = parser.expression(0)?;
    if parser.raw().kind != Kind::End {
        return Err(parser.error("Unexpected token after expression"));
    }
    Ok(expression)
}
pub fn parse_module(source: &str) -> Result<Module, ParseError> {
    parse_module_with_version(source, SyntaxVersion::default())
}
pub fn parse_module_with_version(
    source: &str,
    syntax: SyntaxVersion,
) -> Result<Module, ParseError> {
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
    let mut parser = Parser::new(tokenize(source, start, true)?);
    parser.syntax = syntax;
    parser.module()
}
