use failure;
use sheep_tcp::tcp::TCPManager;
use std::sync::Arc;
use std::process;
use std::{env, io, str};
#[macro_use]
extern crate log;
extern crate ctrlc;
use std::net::Ipv4Addr;

fn main() {
	env::set_var("RUST_LOG", "debug");
	env_logger::init();
	let args: Vec<String> = env::args().collect();
	if args.len() != 4 {
		error!("usage: ./sheep_tcp [server|client] [addr] [port]");
		std::process::exit(1);
	}
	let tcp_manager = TCPManager::init().expect("initial error");

	let role: &str = &args[1];
	let addr: Ipv4Addr = args[2].parse().unwrap();
	let port_num: u16 = args[3].parse().unwrap();
	if role == "server" {
		if let Err(e) = serve(tcp_manager) {
			error!("{}", e);
		}
	} else if role == "client" {
		if let Err(e) = communicate(tcp_manager, addr, port_num) {
			error!("{}", e);
		}
	}
}

fn serve(tcp_manager: Arc<TCPManager>) -> Result<(), failure::Error> {
	let stream_id = tcp_manager.listen(60000).unwrap();
	loop {
		let mut buffer = [0u8; 100];
		let read_size = 10;
		let nbytes = tcp_manager.read(stream_id, &mut buffer, read_size)?;
		// reader.read_until(b'\n', &mut buffer)?;
		print!("{}", str::from_utf8(&buffer[..nbytes])?);

		tcp_manager.send(stream_id, &buffer[..nbytes])?;
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
	})?;
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
		info!("{}", str::from_utf8(&buffer[..nbytes])?);
	}
}



