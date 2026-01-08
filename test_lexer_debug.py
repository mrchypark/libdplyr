from libdplyr_ffi import libdplyr

input_str = 'inner_join(df1, df2, by = "id")'

# Initialize lexer
lexer = libdplyr.Lexer(input_str)

# Collect tokens
tokens = []
while True:
    token = lexer.next_token()
    if token.is_err():
        print(f"Error: {token.error()}")
        break
    tok = token.unwrap()
    tokens.append((tok, lexer.position))
    print(f"Token: {tok}, Position: {lexer.position}")
    if tok == libdplyr.Token.EOF:
        break

print("\nDone")
