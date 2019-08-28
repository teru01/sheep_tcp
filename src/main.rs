use failure;
use sheep_tcp::socket::Socket;
use sheep_tcp::tcp::TCPManager;
use std::sync::Arc;
use std::{thread, process};
use std::time::Duration;
use std::{env, fs, io, str};
#[macro_use]
extern crate log;
extern crate ctrlc;
use std::net::Ipv4Addr;

fn main() {
	env::set_var("RUST_LOG", "debug");
	env_logger::init();
	let args: Vec<String> = env::args().collect();
	if args.len() != 3 {
		error!("missing addr port num");
		std::process::exit(1);
	}
	let addr: Ipv4Addr = args[1].parse().unwrap();
	let port_num: u16 = args[2].parse().unwrap();

	let tcp_manager = TCPManager::init().expect("initial error");

	// if let Err(e) = communicate(tcp_manager, addr, port_num) {
	// 	error!("{}", e);
	// }
	let listening_socket_id = tcp_manager.bind(60000).unwrap();
	loop {
	    let (socket_id, _) = tcp_manager.accept(listening_socket_id);
	    // スレッドを立ち上げて接続に対処する。
		cloned_manager = tcp_manager.clone();
	    thread::spawn(move || {
	        handler(cloned_manager).unwrap_or_else(|error| error!("{:?}", error));
	    });
	}
}

fn handler(mut tcp_manager: &TCPManager) -> Result<(), failure::Error> {
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
	let stream_id = tcp_manager.connect(addr, port)?;
	let cloned = tcp_manager.clone();

	ctrlc::set_handler(move || {
		if let Err(e) = cloned.disconnect(stream_id) {
			error!("{}", e);
		}
		process::exit(0);
	});
	loop {
		// 入力データをソケットから送信。
		let mut input = String::new();
		io::stdin().read_line(&mut input)?;
		tcp_manager.send(stream_id, input.as_bytes())?;

		// ソケットから受信したデータを表示。
		let mut buffer = [0u8; 100];
		let read_size = 10;
		let nbytes = tcp_manager.read(stream_id, &mut buffer, read_size)?;
		// reader.read_until(b'\n', &mut buffer)?;
		print!("{}", str::from_utf8(&buffer[..nbytes])?);
	}
	Ok(())
}



