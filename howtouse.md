# Participle v2 API Documentation

## `participle` Package

This package contains the core functionality for building and using parsers.

### Functions

| Function Signature | Description |
| :--- | :--- |
| `Build[G any](options ...Option) (*Parser[G], error)` | Constructs a parser for the given grammar `G`. |
| `MustBuild[G any](options ...Option) *Parser[G]` | Similar to `Build`, but panics if an error occurs. |
| `ParserForProduction[P, G any](parser *Parser[G]) (*Parser[P], error)` | Returns a new parser for a specific production `P` within a given grammar `G`. |
| `Errorf(pos lexer.Position, format string, args ...interface{}) Error` | Creates a new error at a specific position. |
| `Wrapf(pos lexer.Position, err error, format string, args ...interface{}) Error` | Wraps an existing error with a new message and position. |

---

### Interfaces

| Interface | Description |
| :--- | :--- |
| `Capture` | Implemented by fields to transform captured tokens into field values. It has one method: `Capture(values []string) error`. |
| `Parseable` | Implemented by grammar elements for custom parsing logic. It has one method: `Parse(lex *lexer.PeekingLexer) error`. |
| `Error` | Represents a parsing error, providing methods for accessing the error message and position. |

---

### Types

#### `Parser[G any]`

A parser for a specific grammar.

| Method | Description |
| :--- | :--- |
| `(p *Parser[G]) Lexer() lexer.Definition` | Returns the parser's lexer definition. |
| `(p *Parser[G]) Lex(filename string, r io.Reader) ([]lexer.Token, error)` | Tokenizes input using the parser's lexer. |
| `(p *Parser[G]) Parse(filename string, r io.Reader, options ...ParseOption) (*G, error)` | Parses input from an `io.Reader`. |
| `(p *Parser[G]) ParseFromLexer(lex *lexer.PeekingLexer, options ...ParseOption) (*G, error)` | Parses input from a `PeekingLexer`. |
| `(p *Parser[G]) ParseString(filename string, s string, options ...ParseOption) (*G, error)` | Parses input from a string. |
| `(p *Parser[G]) ParseBytes(filename string, b []byte, options ...ParseOption) (*G, error)` | Parses input from a byte slice. |
| `(p *Parser[G]) String() string` | Returns the EBNF representation of the grammar. |

---

#### `Option`

A function for configuring a `Parser`.

| Function | Description |
| :--- | :--- |
| `Lexer(def lexer.Definition) Option` | Sets the lexer definition for the parser. |
| `UseLookahead(n int) Option` | Sets the number of tokens to look ahead for disambiguation. |
| `CaseInsensitive(tokens ...string) Option` | Specifies token types to be matched case-insensitively. |
| `ParseTypeWith[T any](parseFn func(*lexer.PeekingLexer) (T, error)) Option` | Associates a custom parsing function with a type. |
| `Union[T any](members ...T) Option` | Defines a union type where members are tried in sequence. |
| `Elide(types ...string) Option` | Specifies token types to be elided from the token stream. |
| `Map(mapper Mapper, symbols ...string) Option` | Applies a mapping function to each token. |
| `Unquote(types ...string) Option` | Unquotes string tokens of the given types. |
| `Upper(types ...string) Option` | Converts tokens of the given types to uppercase. |

---

#### `ParseOption`

A function for modifying an individual parse.

| Function | Description |
| :--- | :--- |
| `Trace(w io.Writer) ParseOption` | Traces the parse to the given writer. |
| `AllowTrailing(ok bool) ParseOption` | Allows trailing tokens without causing an error. |

---

## `lexer` Package

This package provides interfaces and implementations for lexing.

### Functions

| Function Signature | Description |
| :--- | :--- |
| `New(rules Rules) (*StatefulDefinition, error)` | Creates a new stateful lexer from a set of rules. |
| `MustStateful(rules Rules) *StatefulDefinition` | Similar to `New`, but panics on error. |
| `NewSimple(rules []SimpleRule) (*StatefulDefinition, error)` | Creates a new stateful lexer with a single root state. |
| `MustSimple(rules []SimpleRule) *StatefulDefinition` | Similar to `NewSimple`, but panics on error. |
| `Upgrade(lex Lexer, elide ...TokenType) (*PeekingLexer, error)` | Upgrades a `Lexer` to a `PeekingLexer` with lookahead capabilities. |

---

### Interfaces

| Interface | Description |
| :--- | :--- |
| `Definition` | The main entry point for lexing, providing methods for creating a `Lexer` and retrieving symbol information. |
| `Lexer` | Returns tokens from a source via the `Next()` method. |
| `Action` | An action to be applied when a lexer rule matches. |

---

### Types

#### `StatefulDefinition`

A `Definition` for a stateful lexer.

| Method | Description |
| :--- | :--- |
| `(d *StatefulDefinition) Lex(filename string, r io.Reader) (Lexer, error)` | Creates a new `Lexer` for the given input. |
| `(d *StatefulDefinition) LexString(filename string, s string) (Lexer, error)` | A fast-path for lexing strings. |
| `(d *StatefulDefinition) Symbols() map[string]TokenType` | Returns the symbol map for the lexer. |
| `(d *StatefulDefinition) Rules() Rules` | Returns the rules used to construct the lexer. |

---

#### `PeekingLexer`

A `Lexer` with lookahead capabilities.

| Method | Description |
| :--- | :--- |
| `(p *PeekingLexer) Next() *Token` | Consumes and returns the next token. |
| `(p *PeekingLexer) Peek() *Token` | Peeks at the next non-elided token. |
| `(p *PeekingLexer) Range(rawStart, rawEnd RawCursor) []Token` | Returns the slice of tokens between two cursor points. |