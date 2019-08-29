use pnet::packet::tcp::{TcpFlags, TcpPacket};
use pnet::packet::Packet;
use pnet::transport::{self, TransportSender};
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
extern crate rand;
use rand::Rng;
use std::cmp::min;

use super::socket::{Socket, TcpStatus};
use super::util;

const HS_RETRY_LIMIT: i32 = 3;
const FIN_RETRY_LIMIT: i32 = 3;
const MSS: usize = 1460;
const UNDEFINED_ADDR: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);
const UNDEFINED_PORT: u16 = 0;

type SockId = (Ipv4Addr, u16);

pub struct TCPManager {
	my_ip: Ipv4Addr,
	//srcPortがキー(1ポートでしか受けられない) (相手のaddr, portのタプルをキーにしたら？)
	connections: RwLock<HashMap<SockId, Socket>>,
	backlog: RwLock<VecDeque<SockId>>
}

impl TCPManager {
	pub fn init() -> Result<Arc<Self>, failure::Error> {
		let config = util::load_env();

		let manager = Arc::new(TCPManager {
			my_ip: config.get("IP_ADDR").expect("missing IP_ADDR").parse()?,
			connections: RwLock::new(HashMap::new()),
			backlog: RwLock::new(VecDeque::new()),
		});
		let cloned = manager.clone();
		thread::spawn(move || cloned.recv_handler());
		Ok(manager)
	}

	pub fn listen(&self, client_port: u16) -> Result<SockId, failure::Error> {
		let socket = Socket::initialize(self.my_ip, None, client_port, None, TcpStatus::Listen);
		let mut table_lock = self.connections.write().unwrap();
		table_lock.insert((UNDEFINED_ADDR, UNDEFINED_PORT), socket);
		Ok((UNDEFINED_ADDR, UNDEFINED_PORT))
	}

	pub fn accept(&self) -> SockId {
		loop {
			let mut que_lock = self.backlog.write().unwrap();
			if !que_lock.is_empty() {
				return que_lock.pop_front().unwrap();
			}
			drop(que_lock);
			thread::sleep(Duration::from_millis(20));
		}
	}

	pub fn connect(&self, addr: Ipv4Addr, port: u16) -> Result<SockId, failure::Error> {
		let mut rng = rand::thread_rng();
		let my_port = rng.gen_range(50000, 65000);

		let socket = Socket::initialize(self.my_ip, Some(addr), my_port, Some(port), TcpStatus::Closed);
		let mut table_lock = self.connections.write().unwrap();
		table_lock.insert((addr, port), socket);

		let (mut ts, _) = util::create_tcp_channel()?;
		let socket = table_lock.get_mut(&(addr, port)).unwrap();
		socket.send_tcp_packet(&mut ts, TcpFlags::SYN, None)?;
		socket.status = TcpStatus::SynSent;

		drop(table_lock);

		let mut retry_count = 0;
		loop {
			thread::sleep(Duration::from_millis(1000));
			let mut table_lock = self.connections.write().unwrap();
			let socket = table_lock.get_mut(&(addr, port)).unwrap();
			if socket.status == TcpStatus::Established {
				break;
			}
			if retry_count > HS_RETRY_LIMIT {
				return Err(failure::err_msg("tcp syn retry count exceeded"));
			}
			socket.send_tcp_packet(&mut ts, TcpFlags::SYN, None)?;
			retry_count += 1;
		}
		Ok((addr, port))
	}

	pub fn disconnect(&self, stream_id: SockId) -> Result<(), failure::Error> {
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
				debug!("stream_id: {:?} closed", stream_id);
				Ok(())
			}
		}
	}

	pub fn send(&self, stream_id: SockId, payload: &[u8]) -> Result<(), failure::Error> {
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
					let mut socket = {
						// recv SYN while listening
						let socket = if tcp_packet.get_flags() == TcpFlags::SYN {
							table_lock.get_mut(&(UNDEFINED_ADDR, UNDEFINED_PORT))
						} else {
							table_lock.get_mut(&(src_addr, tcp_packet.get_destination()))
						};

						match socket {
							Some(sock) => sock,
							None => return Err(failure::err_msg("unavailable socket"))
						}
					};
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
							self.syn_recv_state_handler(&tcp_packet, socket, src_addr)?;
						}
						TcpStatus::LastAck => {
							self.lastack_state_handler(&tcp_packet, socket)?;
						}

						_ => {
							warn!("unimplemented state: {:?}", socket.status);
						}
					}
					socket.recv_param.window = tcp_packet.get_window();
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
		src_addr: Ipv4Addr,
	) -> Result<(), failure::Error> {
		let recv_tcp_flag = recv_packet.get_flags();
		if recv_tcp_flag & TcpFlags::ACK > 0 {
			// 接続済みソケットの生成
			let mut new_socket = Socket::create_established(self.my_ip, src_addr, recv_packet.get_source(), recv_packet.get_destination(), &socket.send_param, &socket.recv_param);
			new_socket.recv_param.next = recv_packet.get_sequence();
			new_socket.send_param.una = recv_packet.get_acknowledgement();
			let mut table_lock = self.connections.write().unwrap();
			let stream_id = (src_addr, recv_packet.get_source());
			table_lock.insert(stream_id, new_socket);

			let mut que_lock = self.backlog.write().unwrap();
			que_lock.push_back(stream_id);

			// リスニングソケットはリッスン状態に戻る
			*socket = Socket::initialize(self.my_ip, Some(src_addr), recv_packet.get_destination(), Some(recv_packet.get_source()), TcpStatus::Listen);
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
		stream_id: SockId,
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
