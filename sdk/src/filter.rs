use anyhow::{anyhow, Result};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Token {
    ColRef(usize),           // $4
    Number(f64),             // 100.5
    String(String),          // "Gripe"
    And,                     // AND, &&
    Or,                      // OR, ||
    Eq,                      // ==, =
    Neq,                     // !=
    Gt,                      // >
    Gte,                     // >=
    Lt,                      // <
    Lte,                     // <=
    LParen,                  // (
    RParen,                  // )
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Op {
    Eq, Neq, Gt, Gte, Lt, Lte
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Expr {
    // Boolean logic
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    
    // Comparisons
    Compare(usize, Op, Value),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Value {
    Number(f64),
    String(String),
}

pub fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        if c == '$' {
            i += 1;
            let mut num_str = String::new();
            while i < chars.len() && chars[i].is_ascii_digit() {
                num_str.push(chars[i]);
                i += 1;
            }
            if num_str.is_empty() {
                return Err(anyhow!("Expected column index after $"));
            }
            tokens.push(Token::ColRef(num_str.parse()?));
            continue;
        }

        if c == '"' || c == '\'' {
            let quote = c;
            i += 1;
            let mut s = String::new();
            while i < chars.len() && chars[i] != quote {
                s.push(chars[i]);
                i += 1;
            }
            if i >= chars.len() {
                return Err(anyhow!("Unterminated string literal"));
            }
            i += 1; // skip closing quote
            tokens.push(Token::String(s));
            continue;
        }

        if c.is_ascii_digit() || (c == '-' && i + 1 < chars.len() && chars[i+1].is_ascii_digit()) {
            let mut num_str = String::new();
            if c == '-' {
                num_str.push('-');
                i += 1;
            }
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                num_str.push(chars[i]);
                i += 1;
            }
            tokens.push(Token::Number(num_str.parse()?));
            continue;
        }

        match c {
            '(' => { tokens.push(Token::LParen); i += 1; },
            ')' => { tokens.push(Token::RParen); i += 1; },
            '=' => {
                if i + 1 < chars.len() && chars[i+1] == '=' { i += 2; } else { i += 1; }
                tokens.push(Token::Eq);
            },
            '!' => {
                if i + 1 < chars.len() && chars[i+1] == '=' {
                    tokens.push(Token::Neq);
                    i += 2;
                } else {
                    return Err(anyhow!("Unexpected character '!'"));
                }
            },
            '>' => {
                if i + 1 < chars.len() && chars[i+1] == '=' {
                    tokens.push(Token::Gte);
                    i += 2;
                } else {
                    tokens.push(Token::Gt);
                    i += 1;
                }
            },
            '<' => {
                if i + 1 < chars.len() && chars[i+1] == '=' {
                    tokens.push(Token::Lte);
                    i += 2;
                } else {
                    tokens.push(Token::Lt);
                    i += 1;
                }
            },
            '&' => {
                if i + 1 < chars.len() && chars[i+1] == '&' {
                    tokens.push(Token::And);
                    i += 2;
                } else {
                    return Err(anyhow!("Expected &&"));
                }
            },
            '|' => {
                if i + 1 < chars.len() && chars[i+1] == '|' {
                    tokens.push(Token::Or);
                    i += 2;
                } else {
                    return Err(anyhow!("Expected ||"));
                }
            },
            _ => {
                // Try reading words like AND, OR
                let mut word = String::new();
                while i < chars.len() && chars[i].is_alphabetic() {
                    word.push(chars[i]);
                    i += 1;
                }
                match word.to_uppercase().as_str() {
                    "AND" => tokens.push(Token::And),
                    "OR" => tokens.push(Token::Or),
                    _ if !word.is_empty() => return Err(anyhow!("Unexpected keyword: {}", word)),
                    _ => return Err(anyhow!("Unexpected character: {}", c)),
                }
            }
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos);
        self.pos += 1;
        t
    }

    fn parse_or(&mut self) -> Result<Expr> {
        let mut node = self.parse_and()?;
        while let Some(Token::Or) = self.peek() {
            self.advance();
            let right = self.parse_and()?;
            node = Expr::Or(Box::new(node), Box::new(right));
        }
        Ok(node)
    }

    fn parse_and(&mut self) -> Result<Expr> {
        let mut node = self.parse_primary()?;
        while let Some(Token::And) = self.peek() {
            self.advance();
            let right = self.parse_primary()?;
            node = Expr::And(Box::new(node), Box::new(right));
        }
        Ok(node)
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        if let Some(Token::LParen) = self.peek() {
            self.advance();
            let expr = self.parse_or()?;
            if let Some(Token::RParen) = self.peek() {
                self.advance();
                return Ok(expr);
            } else {
                return Err(anyhow!("Expected closing parenthesis"));
            }
        }

        // Must be a comparison: ColRef Op Value
        let col = match self.advance() {
            Some(Token::ColRef(c)) => *c,
            other => return Err(anyhow!("Expected column reference (e.g., $0), found {:?}", other)),
        };

        let op = match self.advance() {
            Some(Token::Eq) => Op::Eq,
            Some(Token::Neq) => Op::Neq,
            Some(Token::Gt) => Op::Gt,
            Some(Token::Gte) => Op::Gte,
            Some(Token::Lt) => Op::Lt,
            Some(Token::Lte) => Op::Lte,
            other => return Err(anyhow!("Expected comparison operator, found {:?}", other)),
        };

        let val = match self.advance() {
            Some(Token::Number(n)) => Value::Number(*n),
            Some(Token::String(s)) => Value::String(s.clone()),
            other => return Err(anyhow!("Expected value (number or string), found {:?}", other)),
        };

        Ok(Expr::Compare(col, op, val))
    }
}

pub fn parse_filter(input: &str) -> Result<Expr> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_or()?;
    if parser.pos < parser.tokens.len() {
        return Err(anyhow!("Unexpected tokens at end of expression"));
    }
    Ok(ast)
}

/// Evaluates a parsed filter expression against a single row's raw bytes.
/// `extract_fn` should take a column index and return the field's string slice.
pub fn evaluate_row<'a, F>(expr: &Expr, extract_fn: &F) -> bool
where
    F: Fn(usize) -> Option<&'a str>,
{
    match expr {
        Expr::And(left, right) => evaluate_row(left, extract_fn) && evaluate_row(right, extract_fn),
        Expr::Or(left, right) => evaluate_row(left, extract_fn) || evaluate_row(right, extract_fn),
        Expr::Compare(col, op, target_val) => {
            let field_str = extract_fn(*col);
            if field_str.is_none() {
                return false;
            }
            let field_str = field_str.unwrap().trim();
            // remove surrounding quotes if present for comparison
            let field_str = if field_str.starts_with('"') && field_str.ends_with('"') && field_str.len() >= 2 {
                &field_str[1..field_str.len()-1]
            } else { field_str };

            match target_val {
                Value::Number(n) => {
                    if let Ok(field_num) = field_str.parse::<f64>() {
                        match op {
                            Op::Eq => (field_num - n).abs() < 1e-9, // Float eq handling
                            Op::Neq => (field_num - n).abs() >= 1e-9,
                            Op::Gt => field_num > *n,
                            Op::Gte => field_num >= *n,
                            Op::Lt => field_num < *n,
                            Op::Lte => field_num <= *n,
                        }
                    } else {
                        false
                    }
                }
                Value::String(s) => {
                    let cmp = field_str.eq_ignore_ascii_case(s);
                    match op {
                        Op::Eq => cmp,
                        Op::Neq => !cmp,
                        _ => false, // Cannot do > < on strings for now
                    }
                }
            }
        }
    }
}
