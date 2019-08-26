use tcp;
use std::thread;
use std::{env, str};
use failure;
#[macro_use]
extern crate log;

fn main() {
	env::set_var("RUST_LOG", "debug");
    env_logger::init();
	let tcp_manager = tcp::TCPManager::bind(3000).unwrap();
	loop {
        let (stream, _) = tcp_manager.accept().unwrap();
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
        stream.write_all(&buffer[..nbytes])?;
    }
}


