use pnet::packet::tcp::{TcpFlags, TcpPacket};
use pnet::packet::Packet;
use pnet::transport::{self, TransportSender};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
extern crate rand;
use rand::Rng;
use std::cmp::min;

use super::socket::{RecvParam, SendParam, Socket, TcpStatus};
use super::util;

const TCP_INIT_WINDOW: usize = 1460;
const HS_RETRY_LIMIT: i32 = 3;
const FIN_RETRY_LIMIT: i32 = 3;
const MSS: usize = 1460;

pub struct TCPManager {
	my_ip: Ipv4Addr,
	//srcPortがキー(1ポートでしか受けられない) (相手のaddr, portのタプルをキーにしたら？)
	connections: RwLock<HashMap<u16, Socket>>,
}

impl TCPManager {
	pub fn init() -> Result<Arc<Self>, failure::Error> {
		let config = util::load_env();

		let manager = Arc::new(TCPManager {
			my_ip: config.get("IP_ADDR").expect("missing IP_ADDR").parse()?,
			connections: RwLock::new(HashMap::new()),
		});
		let cloned = manager.clone();
		thread::spawn(move || cloned.recv_handler());
		Ok(manager)
	}

	pub fn listen(&self, client_port: u16) -> Result<u16, failure::Error> {
		let initial_seq = rand::random::<u32>();
		let socket = Socket {
			src_addr: self.my_ip,
			dst_addr: None,
			src_port: client_port,
			dst_port: None,
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
			status: TcpStatus::Listen,
			buffer: Vec::new(),
		};
		let mut table_lock = self.connections.write().unwrap();
		table_lock.insert(client_port, socket);
		Ok(client_port)
	}

	// pub fn accept(&self) -> (Socket, Ipv4Addr) {
	// 	loop {
	// 		let lock = self.waiting_queue.read().unwrap();
	// 		if !lock.is_empty() {
	// 			break;
	// 		}
	// 		drop(lock);
	// 		thread::sleep(Duration::from_millis(10));
	// 	}
	// 	let mut lock = self.waiting_queue.write().unwrap();
	// 	let socket = lock.pop_front().unwrap();
	// 	let mut con_lock = self.connections.write().unwrap();
	// 	con_lock[&socket.src_port] = socket;
	// 	socket
	// ブロックする
	// 受信したのが待ち受けポートだったら3whs
	// TCPstreamを生成、アクティブ接続として保持する
	// 適切なソケットを返す
	// }

	pub fn connect(&self, addr: Ipv4Addr, port: u16) -> Result<u16, failure::Error> {
		let mut rng = rand::thread_rng();
		let client_port = rng.gen_range(50000, 65000);
		let initial_seq = rand::random::<u32>();
		let socket = Socket {
			src_addr: self.my_ip,
			dst_addr: Some(addr),
			src_port: client_port,
			dst_port: Some(port),
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
			buffer: Vec::new(),
		};
		let mut table_lock = self.connections.write().unwrap();
		table_lock.insert(client_port, socket);

		let (mut ts, _) = util::create_tcp_channel()?;
		let socket = table_lock.get_mut(&client_port).unwrap();
		socket.send_tcp_packet(&mut ts, TcpFlags::SYN, None)?;
		socket.status = TcpStatus::SynSent;

		drop(table_lock);

		let mut retry_count = 0;
		loop {
			thread::sleep(Duration::from_millis(1000));
			let mut table_lock = self.connections.write().unwrap();
			let socket = table_lock.get_mut(&client_port).unwrap();
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
				socket.send_tcp_packet(&mut ts, TcpFlags::FIN | TcpFlags::ACK, None)?;
				socket.status = TcpStatus::FinWait1;
				drop(table_lock);
				let mut retry_count = 0;
				let mut timewait_count = 0;
				loop {
					thread::sleep(Duration::from_millis(1000));
					let mut table_lock = self.connections.write().unwrap();
					let mut socket = table_lock.get_mut(&stream_id).unwrap();

					if socket.status == TcpStatus::TimeWait {
						if timewait_count > 1 {
							socket.status = TcpStatus::Closed;
							break;
						}
						timewait_count += 1;
						continue;
					}
					if retry_count > FIN_RETRY_LIMIT {
						return Err(failure::err_msg("fin retry limit exceeded"));
					}
					socket.send_tcp_packet(&mut ts, TcpFlags::FIN, None)?;
					retry_count += 1;
				}
				let mut table_lock = self.connections.write().unwrap();
				table_lock.remove(&stream_id).unwrap();
				debug!("stream_id: {} closed", stream_id);
				Ok(stream_id)
			}
		}
	}

	pub fn send(&self, stream_id: u16, payload: &[u8]) -> Result<(), failure::Error> {
		let (mut ts, _) = util::create_tcp_channel()?;
		let table_lock = self.connections.read().unwrap();
		if !table_lock.contains_key(&stream_id) {
			return Err(failure::err_msg("connection have not been established."));
		}

		let socket = table_lock.get(&stream_id).unwrap();
		if socket.status != TcpStatus::Established {
			Err(failure::err_msg("connection have not been established."))?
		}
		let mut unsent_len = payload.len();
		let mut len_to_be_sent = unsent_len;
		let mut left = 0;
		let max_len = min(socket.recv_param.window as usize, MSS);

		drop(table_lock);

		while unsent_len > 0 {
			if unsent_len > max_len {
				len_to_be_sent = max_len;
			}
			let mut retry_count = 0;
			loop {
				if retry_count > 3 {
					return Err(failure::err_msg("senddata retry limit exceeded."));
				}
				let mut table_lock = self.connections.write().unwrap();
				let socket = table_lock.get_mut(&stream_id).unwrap();
				socket.send_tcp_packet(
					&mut ts,
					TcpFlags::ACK,
					Some(&payload[left..left + len_to_be_sent]),
				)?;
				drop(table_lock);
				thread::sleep(Duration::from_millis(200));

				let table_lock = self.connections.read().unwrap();
				let socket = table_lock.get(&stream_id).unwrap();

				if socket.send_param.una == socket.send_param.next {
					break;
				}
				retry_count += 1;
			}
			unsent_len -= len_to_be_sent;
			left += len_to_be_sent;
		}
		Ok(())
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
						if !util::is_correct_checksum(&tcp_packet, &src_addr, &self.my_ip)
							|| !util::is_valid_seq_num(socket, &tcp_packet)
						{
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
							TcpStatus::FinWait1 => {
								self.finwait_state_handler(&tcp_packet, socket, &mut ts)?;
							}
							TcpStatus::FinWait2 => {
								self.finwait_state_handler(&tcp_packet, socket, &mut ts)?;
							}
							TcpStatus::Listen => {
								self.listen_state_handler(&tcp_packet, socket, &mut ts, src_addr)?;
							}
							TcpStatus::SynRecv => {
								self.syn_recv_state_handler(&tcp_packet, socket, &mut ts)?;
							}
							TcpStatus::LastAck => {
								self.lastack_state_handler(&tcp_packet, socket)?;
							}

							_ => {
								warn!("unimplemented state: {:?}", socket.status);
							}
						}
						socket.recv_param.window = tcp_packet.get_window();
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

	pub fn lastack_state_handler(
		&self,
		recv_packet: &TcpPacket,
		socket: &mut Socket,
	) -> Result<(), failure::Error> {
		if recv_packet.get_flags() & TcpFlags::ACK > 0 {
			socket.status = TcpStatus::Closed;
			socket.recv_param.next = recv_packet.get_sequence();
			socket.send_param.una = recv_packet.get_acknowledgement();
		}
		Ok(())
	}

	pub fn listen_state_handler(
		&self,
		recv_packet: &TcpPacket,
		socket: &mut Socket,
		ts: &mut TransportSender,
		src_addr: Ipv4Addr,
	) -> Result<(), failure::Error> {
		let recv_tcp_flag = recv_packet.get_flags();
		if recv_tcp_flag & TcpFlags::SYN > 0 {
			socket.status = TcpStatus::SynRecv;
			socket.dst_port = Some(recv_packet.get_source());
			socket.dst_addr = Some(src_addr);
			socket.recv_param.irs = recv_packet.get_sequence() + 1;
			socket.recv_param.next = recv_packet.get_sequence() + 1;
			socket.send_tcp_packet(ts, TcpFlags::SYN | TcpFlags::ACK, None)?;
		}
		Ok(())
	}

	pub fn syn_recv_state_handler(
		&self,
		recv_packet: &TcpPacket,
		socket: &mut Socket,
		ts: &mut TransportSender,
	) -> Result<(), failure::Error> {
		let recv_tcp_flag = recv_packet.get_flags();
		if recv_tcp_flag & TcpFlags::ACK > 0 {
			socket.status = TcpStatus::Established;
			socket.recv_param.next = recv_packet.get_sequence();
			socket.send_param.una = recv_packet.get_acknowledgement();
			socket.send_tcp_packet(ts, TcpFlags::SYN | TcpFlags::ACK, None)?;
		}
		Ok(())
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
		let recv_tcp_flag = recv_packet.get_flags();
		let payload = recv_packet.payload();

		if recv_tcp_flag & TcpFlags::FIN > 0 {
			socket.recv_param.next = payload.len() as u32 + recv_packet.get_sequence() + 1;
			socket.send_param.una = recv_packet.get_acknowledgement();
			socket.send_tcp_packet(ts, TcpFlags::ACK, None)?;
			socket.send_tcp_packet(ts, TcpFlags::FIN | TcpFlags::ACK, None)?;
			socket.status = TcpStatus::LastAck;
			return Ok(());
		}

		debug!("recv payload len: {}", payload.len());
		socket.buffer.extend_from_slice(payload);
		socket.recv_param.next = recv_packet.get_sequence() + payload.len() as u32;
		socket.send_param.una = recv_packet.get_acknowledgement();
		if payload.len() > 0 {
			socket.send_tcp_packet(ts, TcpFlags::ACK, None)?;
		}
		Ok(())
	}

	pub fn finwait_state_handler(
		&self,
		recv_packet: &TcpPacket,
		socket: &mut Socket,
		ts: &mut TransportSender,
	) -> Result<(), failure::Error> {
		let recv_tcp_flag = recv_packet.get_flags();
		if recv_tcp_flag & TcpFlags::FIN > 0 {
			socket.status = TcpStatus::Closing;
			socket.recv_param.next = recv_packet.get_sequence() + 1;
			socket.send_param.una = recv_packet.get_acknowledgement();
			socket.send_tcp_packet(ts, TcpFlags::ACK, None)?;
			if recv_tcp_flag & TcpFlags::ACK > 0 {
				socket.status = TcpStatus::TimeWait;
			}
		} else if recv_tcp_flag & TcpFlags::ACK > 0 {
			socket.status = TcpStatus::FinWait2;
			socket.recv_param.next = recv_packet.get_sequence();
			socket.send_param.una = recv_packet.get_acknowledgement();
		}
		Ok(())
	}

	pub fn read(
		&self,
		stream_id: u16,
		buffer: &mut [u8],
		read_size: usize,
	) -> Result<usize, failure::Error> {
		// バッファにデータが溜まるまでブロック
		loop {
			let table_lock = self.connections.read().unwrap();
			if let Some(socket) = table_lock.get(&stream_id) {
				if socket.buffer.len() != 0 {
					break;
				}
			}
			drop(table_lock);
			thread::sleep(Duration::from_millis(10));
		}
		let mut table_lock = self.connections.write().unwrap();
		match table_lock.get_mut(&stream_id) {
			Some(socket) => {
				let mut actual_read_size = read_size;
				if socket.buffer.len() <= read_size {
					actual_read_size = socket.buffer.len();
				}
				buffer[..actual_read_size].copy_from_slice(&socket.buffer[..actual_read_size]);
				socket.buffer = socket.buffer[actual_read_size..].to_vec();
				debug!("sock buf: {}", socket.buffer.len());
				Ok(actual_read_size)
			}
			None => Err(failure::err_msg("stream was not found.")),
		}
	}
}
