use std::collections::{HashMap, VecDeque};
use std::thread;
use std::net::Ipv4Addr;
use std::sync::{RwLock};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::transport::{self, TransportChannelType, TransportProtocol, TransportReceiver, TransportSender};
use pnet::packet::tcp::{self, MutableTcpPacket, TcpFlags};

const TCP_SIZE: usize = 20;

enum TcpStatus {
	Established = 1,
	SynSent = 2,
}

pub struct Socket {
	dst_addr: Ipv4Addr,
	dst_port: u16,
	src_port: u16,
	status: TcpStatus
}

pub struct TCPListener {
	connections: RwLock<HashMap<(Ipv4Addr, u16), Socket>>, //(ip, port)がキー
	waiting_queue: RwLock<VecDeque<Socket>> // acceptに拾われるのを待ってるソケット
}

// listenerを分けると、1つのポートで複数のクライアントを管理できる
impl TCPListener {
	pub fn bind(port: u16) -> Result<Self, failure::Error> {
		let mut listener = TCPListener {
			connections: RwLock::new(HashMap::new()),
			waiting_queue: RwLock::new(VecDeque::new())
		};
		thread::spawn(move || listener.recv_handler());
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
			self.connections[(socket.dst_addr, socket.dst_port)] = socket;
			Ok(socket)
		}
		// ブロックする
		// 受信したのが待ち受けポートだったら3whs
		// TCPstreamを生成、アクティブ接続として保持する
		// 適切なソケットを返す
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
						send_sin_ack();
						continue;
					}
					let lock = self.connections.read().unwrap();
					let connection = lock[(src_addr, tcp_packet.src_port)];
					match connection.status {
						TcpStatus::Established => {

						}
					}
				},
				Err(_) => continue,
			}
		}
	}
}

pub fn send_syn() {

}

pub fn send_ack() {

}

impl Socket {
	pub fn read(&self, buffer: &mut [u8]) -> Result<(), failure::Error> {
		// 届いたデータはソケットバッファに貯めて、この関数が呼ばれた時に読み出して返す
		// イテレータ回して受信、自分のポート以外のものは捨てる
	}

	pub fn write(&self) -> Result<(), failure::Error> {

	}
}
