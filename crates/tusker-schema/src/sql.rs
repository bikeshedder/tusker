use std::fmt;

#[derive(Debug, Default)]
pub struct StatementBuilder {
    parts: Vec<String>,
}

impl StatementBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn part(&mut self, s: impl ToString) {
        self.parts.push(s.to_string());
    }
    pub fn ident(&mut self, s: impl ToString) {
        self.part(quote_ident(&s.to_string()));
    }
}

impl fmt::Display for StatementBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.parts.join(" "))
    }
}

pub fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::quote_ident;

    #[test]
    fn quote_ident_doubles_quotes_but_leaves_backslashes() {
        assert_eq!(quote_ident("plain"), "\"plain\"");
        assert_eq!(quote_ident("a\"b"), "\"a\"\"b\"");
        assert_eq!(quote_ident(r"a\b"), r#""a\b""#);
    }
}
