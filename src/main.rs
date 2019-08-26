use tcp;
use std::thread;
use std::{env, str, io};
use failure;
#[macro_use]
extern crate log;
use std::net::Ipv4Addr;

fn main() {
	env::set_var("RUST_LOG", "debug");
    env_logger::init();
	let mut tcp_manager = tcp::TCPManager::init();
	let listener = tcp_manager.bind(3000).unwrap();
	loop {
        let (stream, _) = listener.accept().unwrap();
        // スレッドを立ち上げて接続に対処する。
        thread::spawn(move || {
            handler(stream).unwrap_or_else(|error| error!("{:?}", error));
        });
    }
}

fn handler(mut stream: tcp::Socket) -> Result<(), failure::Error> {
    let mut buffer = [0u8; 1024];
    loop {
        let nbytes = stream.read(&mut buffer)?;
        if nbytes == 0 {
            debug!("Connection closed.");
            return Ok(());
        }
        print!("{}", str::from_utf8(&buffer[..nbytes])?);
        stream.send(&buffer[..nbytes])?;
    }
}

fn connect_to(addr: Ipv4Addr, port: u16) -> Result<(), failure::Error> {
	let mut tcp_manager = tcp::TCPManager::init();
	let mut stream = tcp_manager.connect(addr, port)?;
	loop {
        // 入力データをソケットから送信。
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        stream.send(input.as_bytes())?;

        // ソケットから受信したデータを表示。
        let mut buffer = Vec::new();
		let mut reader = stream.read();
        reader.read_until(b'\n', &mut buffer)?;
        print!("{}", str::from_utf8(&buffer)?);
    }
}


