use failure;
use sheep_tcp::socket::Socket;
use sheep_tcp::tcp::TCPManager;
use std::sync::Arc;
use std::thread;
use std::{env, fs, io, str};
#[macro_use]
extern crate log;
use std::net::Ipv4Addr;

fn main() {
	env::set_var("RUST_LOG", "debug");
	env_logger::init();
	let mut tcp_manager = TCPManager::init().expect("initial error");
	communicate(tcp_manager, "127.0.0.1".parse().unwrap(), 33333);
	// tcp_manager.bind(3000).unwrap();
	// loop {
	//     let (stream, _) = tcp_manager.accept();
	//     // スレッドを立ち上げて接続に対処する。
	//     thread::spawn(move || {
	//         handler(stream).unwrap_or_else(|error| error!("{:?}", error));
	//     });
	// }
}

fn handler(mut stream: Socket) -> Result<(), failure::Error> {
	let mut buffer = [0u8; 1024];
	loop {
		let nbytes = stream.read(&mut buffer)?;
		if nbytes == 0 {
			debug!("Connection closed.");
			return Ok(());
		}
		print!("{}", str::from_utf8(&buffer[..nbytes])?);
		stream.write(&buffer[..nbytes])?;
	}
}

fn communicate(
	tcp_manager: Arc<TCPManager>,
	addr: Ipv4Addr,
	port: u16,
) -> Result<(), failure::Error> {
	let mut stream = tcp_manager.connect(addr, port)?;
	loop {
		// 入力データをソケットから送信。
		let mut input = String::new();
		io::stdin().read_line(&mut input)?;
		stream.write(input.as_bytes())?;

		// ソケットから受信したデータを表示。
		let mut buffer = Vec::new();
		let nbytes = stream.read(&mut buffer);
		// reader.read_until(b'\n', &mut buffer)?;
		print!("{:?} {}", nbytes, str::from_utf8(&buffer)?);
	}
}
