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
use std::time::Duration;

use super::socket::{RecvParam, SendParam, Socket, TcpStatus};
use super::util;

const TCP_INIT_WINDOW: usize = 1460;
const HS_RETRY_LIMIT: i32 = 3;
const CLIENT_PORT: u16 = 55555;

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

	pub fn connect(&self, addr: Ipv4Addr, port: u16) -> Result<u16, failure::Error> {
		let mut table_lock = self.connections.write().unwrap();
		let client_port = CLIENT_PORT;
		let initial_seq = rand::random::<u32>();
		debug!("init_rand:{}", initial_seq);
		let socket = Socket {
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

		let (mut ts, _) = transport::transport_channel(
			1024,
			TransportChannelType::Layer4(TransportProtocol::Ipv4(IpNextHeaderProtocols::Tcp)),
		)?;
		let socket = table_lock.get_mut(&client_port).unwrap();
		socket.send_tcp_packet(&mut ts, TcpFlags::SYN, None)?;
		socket.status = TcpStatus::SynSent;

		drop(table_lock);

		let mut retry_count = 0;
		loop {
			thread::sleep(Duration::from_millis(1000));
			let table_lock = self.connections.write().unwrap();
			let mut socket = table_lock[&client_port];
			if socket.status == TcpStatus::Established {
				break;
			}
			if retry_count > HS_RETRY_LIMIT {
				return Err(failure::err_msg("tcp syn retry count exceeded"));
			}
			socket.send_tcp_packet(&mut ts, TcpFlags::SYN, None)?;
			retry_count += 1;
		}
		Ok(client_port)
	}

	// pub fn disconnect(&self, stream_id: u16) -> Result<(), failure::Error> {

	// }

	pub fn send(&self, stream_id: u16, data: &[u8]) -> Result<(), failure::Error> {
		let (mut ts, _) = transport::transport_channel(
			1024,
			TransportChannelType::Layer4(TransportProtocol::Ipv4(IpNextHeaderProtocols::Tcp)),
		)?;
		let mut table_lock = self.connections.write().unwrap();

		match table_lock.get_mut(&stream_id) {
			Some(socket) => {
				if socket.status != TcpStatus::Established {
					Err(failure::err_msg("connection have not been established."))?
				}
				socket.send_tcp_packet(&mut ts, TcpFlags::ACK, Some(data))?;
				Ok(())
			}
			None => Err(failure::err_msg("stream was not found")),
		}
	}

	pub fn recv_handler(&self) -> Result<(), failure::Error> {
		let (mut ts, mut tr) = transport::transport_channel(
			1024,
			TransportChannelType::Layer4(TransportProtocol::Ipv4(IpNextHeaderProtocols::Tcp)),
		)?;

		let mut packet_iter = transport::tcp_packet_iter(&mut tr);
		debug!("begin recv thread");
		loop {
			match packet_iter.next() {
				Ok((tcp_packet, src_addr)) => {
					let dport = tcp_packet.get_destination();
					if dport != CLIENT_PORT {
						continue;
					}
					debug!("{}", src_addr);
					debug!("{}", tcp_packet.get_destination());
					debug!("{}", tcp_packet.get_flags());

					let mut table_lock = self.connections.write().unwrap();
					if let Some(socket) = table_lock.get_mut(&tcp_packet.get_destination()) {
						debug!("socket status: {:?}", socket.status);
						let recv_tcp_flag = tcp_packet.get_flags();
						match socket.status {
							TcpStatus::SynSent => {
								if recv_tcp_flag & TcpFlags::SYN > 0 {
									socket.status = TcpStatus::SynRecv;
									if recv_tcp_flag & TcpFlags::ACK > 0 {
										debug!(
											"connection established: {}:{}",
											src_addr,
											tcp_packet.get_source()
										);
										socket.status = TcpStatus::Established;
									}
								}
								socket.recv_param.irs = tcp_packet.get_sequence();
								socket.recv_param.next = tcp_packet.get_sequence() + 1;
								socket.send_param.una = tcp_packet.get_acknowledgement();
								debug!("*1");
								socket.send_tcp_packet(&mut ts, TcpFlags::ACK, None)?;
							}
							TcpStatus::Established => {}
							_ => {}
						}
					} else {
						//send_rst_packet();
					}
				}
				Err(_) => {
					warn!("packet received error");
					continue;
				}
			}
		}
	}
}
