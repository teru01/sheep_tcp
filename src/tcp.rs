use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::tcp::{self, MutableTcpPacket, TcpFlags};
use pnet::transport::{
	self, TransportChannelType, TransportProtocol, TransportReceiver, TransportSender,
};
use rand::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, RwLock};
use std::thread;

use super::socket::{RecvParam, SendParam, Socket, TcpStatus};
use super::util;

const TCP_INIT_WINDOW: usize = 1460;

pub struct TCPManager {
	my_ip: Ipv4Addr,
	//srcPortがキー(1ポートでしか受けられない) (相手のaddr, portのタプルをキーにしたら？)
	connections: RwLock<HashMap<u16, Socket>>,
	waiting_queue: RwLock<VecDeque<Socket>>, // acceptに拾われるのを待ってるソケット
}

impl TCPManager {
	pub fn init() -> Result<Arc<Self>, failure::Error> {
		let config = util::load_env();
		let manager = Arc::new(TCPManager {
			my_ip: config.get("IP_ADDR").expect("missing IP_ADDR").parse()?,
			connections: RwLock::new(HashMap::new()),
			waiting_queue: RwLock::new(VecDeque::new()),
		});
		let cloned = manager.clone();
		thread::spawn(move || cloned.recv_handler());
		Ok(manager)
	}

	pub fn bind(&self, port: u16) -> Result<(), failure::Error> {
		// ソケットの初期化
		// //
		// Ok(listener)]
		unimplemented!()
	}

	pub fn accept(&self) -> (Socket, Ipv4Addr) {
		unimplemented!()
		// loop {
		// 	let lock = self.waiting_queue.read().unwrap();
		// 	if !lock.is_empty() {
		// 		break;
		// 	}
		// }
		// let mut lock = self.waiting_queue.write().unwrap();
		// let socket = lock.pop_front().unwrap();
		// let mut con_lock = self.connections.write().unwrap();
		// con_lock[&socket.src_port] = socket;
		// socket
		// ブロックする
		// 受信したのが待ち受けポートだったら3whs
		// TCPstreamを生成、アクティブ接続として保持する
		// 適切なソケットを返す
	}

	pub fn connect(&self, addr: Ipv4Addr, port: u16) -> Result<Socket, failure::Error> {
		let mut table_lock = self.connections.write().unwrap();
		let client_port = 55555;
		let initial_seq = rand::random::<u32>();
		let mut socket = Socket {
			src_addr: self.my_ip,
			dst_addr: addr,
			src_port: client_port,
			dst_port: port,
			send_param: SendParam {
				una: initial_seq,
				next: initial_seq, //同じでいいの？=>OK 送信していないので
				window: TCP_INIT_WINDOW as u16,
				iss: initial_seq,
			},
			recv_param: RecvParam {
				next: 0,
				window: 0,
				irs: 0,
			},
			status: TcpStatus::Closed,
		};
		table_lock.insert(client_port, socket);

		socket.handshake()?;
		Ok(socket)
	}

	pub fn recv_handler(&self) {
		// 受信したのが待ち受けポートだったら3whs, rst, finなどもこれが受ける
		// ポーリングして受信、受け取ったもので分岐 hsまたはデータ
		let (mut ts, mut tr) = transport::transport_channel(
			1024,
			TransportChannelType::Layer4(TransportProtocol::Ipv4(IpNextHeaderProtocols::Tcp)),
		)
		.expect("failed to create channel");
		let mut packet_iter = transport::tcp_packet_iter(&mut tr);
		loop {
			match packet_iter.next() {
				Ok((_tcp_packet, src_addr)) => {
					println!("{}", src_addr);
					// if tcp_packet.get_flags() == TcpFlag.SYN {
					// 	send_flag_only_packet();
					// 	continue;
					// }
					// if let Some(socket) = self.get_socket(tcp_packet.get_destination()) {

					// }
					// match connection.status {
					// 	TcpStatus::Established => {

					// 	},
					// 	TcpStatus::SynSent => {
					// 		//
					// 	}
					// }
				}
				Err(_) => continue,
			}
		}
	}

	pub fn get_socket(&self, port: u16) -> Option<&Socket> {
		unimplemented!()
		// let lock = self.connections.read().unwrap();
		// return lock.get(&port);
	}
}
