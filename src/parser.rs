//! The only parsing adapter: no interpreter, VM, or subprocess.
use crate::{diagnostic::OperationError, source::Source};
use rustpython_parser::{Mode, Parse, Tok, ast, lexer};
pub fn parse(source: &Source) -> Result<ast::Suite, OperationError> {
    let error = |code, msg: String| OperationError::new(code, msg, Some(source.path.clone()));
    let mut nesting = 0usize;
    let mut tokens = 0usize;
    let mut expression = 0usize;
    for token in lexer::lex(source.parsed_text(), Mode::Module) {
        let (token, _) = token.map_err(|e| error("GSU-E006", format!("{e:?}")))?;
        tokens += 1;
        expression += 1;
        match token {
            Tok::Lpar | Tok::Lsqb | Tok::Lbrace | Tok::Indent => nesting += 1,
            Tok::Rpar | Tok::Rsqb | Tok::Rbrace | Tok::Dedent => {
                nesting = nesting.saturating_sub(1)
            }
            Tok::Newline | Tok::Semi => expression = 0,
            _ => {}
        }
        // An additional logical-statement bound protects left-deep ASTs (a+a+...).
        if nesting > 256 || tokens > 500_000 || expression > 4096 {
            return Err(error("GSU-E008","Source exceeds structural depth (256), token (500000), or logical statement (4096 tokens) limit".into()));
        }
    }
    ast::Suite::parse(source.parsed_text(), &source.path)
        .map_err(|e| error("GSU-E006", e.to_string()))
}
