use libdplyr::lexer::Lexer;

fn main() {
    let input = "inner_join(df1, df2, by = \"id\")";
    println!("Input: {}", input);

    let mut lexer = Lexer::new(input.to_string());
    let mut tokens = Vec::new();

    loop {
        match lexer.next_token() {
            Ok(token) => {
                println!("Token: {:?}", token);
                if token == libdplyr::lexer::Token::EOF {
                    break;
                }
                tokens.push(token);
            }
            Err(e) => {
                println!("Error: {:?}", e);
                break;
            }
        }
    }
}
