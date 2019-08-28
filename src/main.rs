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


	if let Err(e) = communicate(tcp_manager, addr, port_num) {
		error!("{}", e);
	}

	// tcp_manager.disconnect(stream_id);
	// 	.unwrap_or_else(|e| error!("{}", e));
	// tcp_manager.bind(3000).unwrap();
	// loop {
	//     let (stream, _) = tcp_manager.accept();
	//     // スレッドを立ち上げて接続に対処する。
	//     thread::spawn(move || {
	//         handler(stream).unwrap_or_else(|error| error!("{:?}", error));
	//     });
	// }
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

		// // ソケットから受信したデータを表示。
		// let mut buffer = Vec::new();
		// let nbytes = tcp_manager.read(stream_id, &mut buffer);
		// // reader.read_until(b'\n', &mut buffer)?;
		// print!("{:?} {}", nbytes, str::from_utf8(&buffer)?);
	}
	Ok(())
}



// fn handler(mut stream: Socket) -> Result<(), failure::Error> {
// 	let mut buffer = [0u8; 1024];
// 	loop {
// 		let nbytes = stream.read(&mut buffer)?;
// 		if nbytes == 0 {
// 			debug!("Connection closed.");
// 			return Ok(());
// 		}
// 		print!("{}", str::from_utf8(&buffer[..nbytes])?);
// 		stream.write(&buffer[..nbytes])?;
// 	}
// }
