use wetc_buyback_backend::{
    shell_response_from_items,
    shell_response_from_hash,
    ParsedInput,
    Response,
    Error,
};

use std::io::{self, Read};

use tokio;

#[tokio::main]
async fn main() {
    let mut buf: String = String::new();
    read_stdin(&mut buf).unwrap();

    let parsed_input: ParsedInput = ParsedInput::from_str(&buf).unwrap();
    let response: Response = match parsed_input {
        ParsedInput::Items((v, l)) => shell_response_from_items(v, l)
            .await
            .unwrap(),
        ParsedInput::Hash(s) => shell_response_from_hash(s)
            .await
            .unwrap(),
    };

    response.to_stdout().unwrap();
}

fn read_stdin(buf: &mut String) -> Result<(), Error> {
    io::stdin()
        .read_to_string(buf)
        .map(|_| ())
        .map_err(|e| Error::StdinError(e))
}
