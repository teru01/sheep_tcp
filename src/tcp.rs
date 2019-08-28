use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::tcp::{self, TcpFlags, TcpPacket};
use pnet::packet::Packet;
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
const FIN_RETRY_LIMIT: i32 = 3;
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

		let (mut ts, _) = util::create_tcp_channel()?;
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

	pub fn disconnect(&self, stream_id: u16) -> Result<u16, failure::Error> {
		let (mut ts, _) = util::create_tcp_channel()?;
		let mut table_lock = self.connections.write().unwrap();
		match table_lock.get_mut(&stream_id) {
			None => Err(failure::err_msg("stream was not found.")),
			Some(socket) => {
				if socket.status != TcpStatus::Established {
					Err(failure::err_msg("connection have not been established."))?
				}
				socket.send_tcp_packet(&mut ts, TcpFlags::FIN, None)?;
				socket.status = TcpStatus::FinWait1;
				drop(socket);
				let mut retry_count = 0;
				loop {
					thread::sleep(Duration::from_millis(1000));
					let table_lock = self.connections.write().unwrap();
					let mut socket = table_lock[&stream_id];
					if socket.status == TcpStatus::Closed {
						break;
					}
					if retry_count > FIN_RETRY_LIMIT {
						return Err(failure::err_msg("fin retry limit exceeded"));
					}
					socket.send_tcp_packet(&mut ts, TcpFlags::FIN, None)?;
					retry_count += 1;
				}
				let mut table_lock = self.connections.write().unwrap();
				table_lock.remove(&stream_id).unwrap();
				Ok(stream_id)
			}
		}
	}

	pub fn send(&self, stream_id: u16, data: &[u8]) -> Result<(), failure::Error> {
		let (mut ts, _) = util::create_tcp_channel()?;
		let mut table_lock = self.connections.write().unwrap();

		match table_lock.get_mut(&stream_id) {
			Some(socket) => {
				if socket.status != TcpStatus::Established {
					Err(failure::err_msg("connection have not been established."))?
				}
				socket.send_tcp_packet(&mut ts, TcpFlags::ACK, Some(data))?;
				Ok(())
			}
			None => Err(failure::err_msg("stream was not found.")),
		}
	}

	pub fn recv_handler(&self) -> Result<(), failure::Error> {
		let (mut ts, mut tr) = util::create_tcp_channel()?;
		let mut packet_iter = transport::tcp_packet_iter(&mut tr);
		debug!("begin recv thread");
		loop {
			match packet_iter.next() {
				Ok((tcp_packet, src_addr)) => {
					let src_addr = match src_addr {
						IpAddr::V4(addr) => addr,
						IpAddr::V6(_) => continue,
					};
					let mut table_lock = self.connections.write().unwrap();
					if let Some(socket) = table_lock.get_mut(&tcp_packet.get_destination()) {
						if !util::is_correct_checksum(&tcp_packet, &src_addr, &self.my_ip) {
							continue;
						}
						util::print_info(&tcp_packet, &src_addr, socket.status);
						match socket.status {
							TcpStatus::SynSent => {
								self.syn_send_state_handler(&tcp_packet, socket, &mut ts)?;
							}
							TcpStatus::Established => {
								self.established_state_handler(&tcp_packet, socket, &mut ts)?;
							}
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

	pub fn syn_send_state_handler(
		&self,
		recv_packet: &TcpPacket,
		socket: &mut Socket,
		ts: &mut TransportSender,
	) -> Result<(), failure::Error> {
		let recv_tcp_flag = recv_packet.get_flags();
		if recv_tcp_flag & TcpFlags::SYN > 0 {
			socket.status = TcpStatus::SynRecv;
			if recv_tcp_flag & TcpFlags::ACK > 0 {
				debug!("connection established",);
				socket.status = TcpStatus::Established;
			}
		}
		socket.recv_param.irs = recv_packet.get_sequence();
		socket.recv_param.next = recv_packet.get_sequence() + 1;
		socket.send_param.una = recv_packet.get_acknowledgement();
		socket.send_tcp_packet(ts, TcpFlags::ACK, None)?;
		Ok(())
	}

	pub fn established_state_handler(
		&self,
		recv_packet: &TcpPacket,
		socket: &mut Socket,
		ts: &mut TransportSender,
	) -> Result<(), failure::Error> {
		let payload = recv_packet.payload();
		// TODO payload processing
		socket.recv_param.next = recv_packet.get_sequence() + payload.len() as u32;
		socket.send_param.una = recv_packet.get_acknowledgement();
		if payload.len() > 0 {
			socket.send_tcp_packet(ts, TcpFlags::ACK, None)?;
		}
		Ok(())
	}
}
