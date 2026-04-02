use std::net::TcpStream;
use std::io::{self, Write, BufRead, BufReader};

fn main() {
    let host = "::1";
    let port = 20000;
    let address = format!("[{}]:{}", host, port);
    
    println!("Connecting to {}", address);

    let mut stream = match TcpStream::connect(&address) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error opening socket: {}", e);
            return;
        }
    };

    let mut reader = BufReader::new(stream.try_clone().unwrap());

    loop {
        print!("Enter a line to send: ");
        io::stdout()
            .flush()
            .unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input)
            .unwrap();

        stream.write_all(input.as_bytes())
            .unwrap();

        let mut response = String::new();
        reader.read_line(&mut response).unwrap();

        print!("Received: {}", response);
    }
}