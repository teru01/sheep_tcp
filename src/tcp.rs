use std::collections::{HashMap, VecDeque};
use std::thread;
use std::net::Ipv4Addr;
use std::sync::{RwLock};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::transport::{self, TransportChannelType, TransportProtocol, TransportReceiver, TransportSender};
use pnet::packet::tcp::{self, MutableTcpPacket, TcpFlags};
use rand::prelude::*;

const TCP_INIT_WINDOW: usize = 1460;
const TCP_SIZE: usize = 20;

enum TcpStatus {
	Established = 1,
	SynSent = 2,
	Closed = 3,
}

pub struct Socket {
	dst_addr: Ipv4Addr,
	dst_port: u16,
	send_param: SendParam,
	recv_param: RecvParam,
	status: TcpStatus,
}

pub struct SendParam {
	una: u32, //未ACK送信
	next: u32, //次の送信
	window: u32,
	iss: u32, //初期送信シーケンス番号
}

pub struct RecvParam {
	next: u32,
	window: u32,
	irs: u32, //初期受信シーケンスno
}

pub struct TCPManager {
	//srcPortがキー
	connections: RwLock<HashMap<u16, Socket>>,
	waiting_queue: RwLock<VecDeque<Socket>> // acceptに拾われるのを待ってるソケット
}

impl TCPManager {
	pub fn init() -> Self {
		let manager = TCPManager {
			connections: RwLock::new(HashMap::new()),
			waiting_queue: RwLock::new(VecDeque::new())
		};
		thread::spawn(move || manager.recv_handler());
		manager
	}

	pub fn bind(&self, port: u16) -> Result<Self, failure::Error> {
		Ok(listener)
	}

	pub fn accept(&self) -> Result<Socket, failure::Error> {
		loop {
			let lock = self.waiting_queue.read().unwrap();
			if lock.is_empty() {
				break;
			}
		}
		let mut lock = self.waiting_queue.write().unwrap();
		if let Some(socket) = lock.pop_front() {
			let mut con_lock = self.connections.write().unwrap();
			con_lock[(socket.dst_addr, socket.dst_port)] = socket;
			Ok(socket)
		}
		// ブロックする
		// 受信したのが待ち受けポートだったら3whs
		// TCPstreamを生成、アクティブ接続として保持する
		// 適切なソケットを返す
	}

	pub fn connect(&self, addr: Ipv4Addr, port: u16) -> Result<Socket, failure::Error> {
		let mut table_lock = self.connections.write().unwrap();
		let client_port = 55555;
		let initial_seq = rand::random::<u32>();
		let socket = Socket {
			dst_addr: addr,
			dst_port: port,
			send_param: SendParam {
				una: initial_seq,
				next: initial_seq, //同じでいいの？
				window: TCP_INIT_WINDOW as u32,
				iss: initial_seq
			},
			recv_param: RecvParam {
				next: 0,
				window: 0,
				irs: 0
			},
			status: TcpStatus::Closed,
		};
		table_lock[&client_port] = socket;

		Ok(socket)
	}

	pub fn send(&self, buffer: &[u8]) {

	}

	pub fn recv_handler(&self) {
		// 受信したのが待ち受けポートだったら3whs, rst, finなどもこれが受ける
		// ポーリングして受信、受け取ったもので分岐 hsまたはデータ
		let (mut ts, mut tr) = transport::transport_channel(1024,
			TransportChannelType::Layer4(TransportProtocol::Ipv4(IpNextHeaderProtocols::Tcp))).expect("failed to create channel");
		let mut packet_iter = transport::tcp_packet_iter(&mut tr);
		loop {
			let tcp_packet = match packet_iter.next() {
				Ok((tcp_packet, src_addr)) => {
					if tcp_packet.get_flags() == TcpFlag.SYN {
						send_flag_only_packet();
						continue;
					}
					let lock = self.connections.read().unwrap();
					let connection = lock[(src_addr, tcp_packet.src_port)];
					match connection.status {
						TcpStatus::Established => {

						},
						TcpStatus::SynSent => {
							//
						}
					}
				},
				Err(_) => continue,
			}
		}
	}
}

pub fn send_flag_only_packet(dst_addr: Ipv4Addr, dst_port: u16, flag: u16) {

}

impl Socket {
	pub fn read(&self, buffer: &mut [u8]) -> Result<(), failure::Error> {
		// 届いたデータはソケットバッファに貯めて、この関数が呼ばれた時に読み出して返す
		// イテレータ回して受信、自分のポート以外のものは捨てる
	}

	pub fn write(&self) -> Result<(), failure::Error> {

	}
}
