use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::tcp::{self, MutableTcpPacket, TcpFlags};
use pnet::transport::{
	self, TransportChannelType, TransportProtocol, TransportReceiver, TransportSender,
};
use rand::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::fmt::{self, Debug};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::RwLock;
use std::thread;
use std::time::Duration;

const TCP_SIZE: usize = 20;

#[derive(Copy, Clone)]
pub struct Socket {
	pub src_addr: Ipv4Addr,
	pub dst_addr: Ipv4Addr,
	pub src_port: u16,
	pub dst_port: u16,
	pub send_param: SendParam,
	pub recv_param: RecvParam,
	pub status: TcpStatus,
}

#[derive(Copy, Clone)]
pub struct SendParam {
	pub una: u32,  //未ACK送信
	pub next: u32, //次の送信
	pub window: u16,
	pub iss: u32, //初期送信シーケンス番号
}

#[derive(Copy, Clone)]
pub struct RecvParam {
	pub next: u32,
	pub window: u16,
	pub irs: u32, //初期受信シーケンスno
}

#[derive(Copy, Clone, PartialEq)]
pub enum TcpStatus {
	Listen,
	SynSent,
	SynRecv,
	Established,
	FinWait1,
	FinWait2,
	Closing,
	LastAck,
	TimeWait,
	Closed,
}

impl Debug for TcpStatus {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			Listen => write!(f, "LISTEN"),
			SynSent => write!(f, "SYNSENT"),
			SynRecv => write!(f, "SYNRECV"),
			Established => write!(f, "ESTABLISHED"),
			FinWait1 => write!(f, "FINWAIT1"),
			FinWait2 => write!(f, "FINWAIT2"),
			Closing => write!(f, "CLOSING"),
			LastAck => write!(f, "LASTACK"),
			TimeWait => write!(f, "TIMEWAIT"),
			Closed => write!(f, "CLOSED"),
		}
	}
}

impl Socket {
	pub fn read(&self, buffer: &mut [u8]) -> Result<usize, failure::Error> {
		// 届いたデータはソケットバッファに貯めて、この関数が呼ばれた時に読み出して返す
		// イテレータ回して受信、自分のポート以外のものは捨てる
		unimplemented!()
	}

	pub fn write(&self, buffer: &[u8]) -> Result<(), failure::Error> {
		unimplemented!()
	}

	// pub fn handshake(&mut self, sender: &mut TransportSender) -> Result<(), failure::Error>{
	// 	debug!("send tcp packet");
	// 	self.send_tcp_packet(sender, TcpFlags::SYN, None)?;
	// 	self.status = TcpStatus::SynSent;
	// 	let mut retry_count = 0;
	// 	debug!("socket status in hs: {:}", self.status as u16);
	// 	loop {
	// 		thread::sleep(Duration::from_millis(1000));

	// 		if self.status == TcpStatus::Established {
	// 			break;
	// 		}
	// 		if retry_count > HS_RETRY_LIMIT {
	// 			return Err(failure::err_msg("tcp syn retry count exceeded"));
	// 		}
	// 		self.send_tcp_packet(sender, TcpFlags::SYN, None)?;
	// 		retry_count += 1;
	// 	}
	// 	Ok(())
	// }

	pub fn send_tcp_packet(
		&mut self,
		ts: &mut TransportSender,
		flag: u16,
		payload: Option<&[u8]>,
	) -> Result<(), failure::Error> {
		let mut payload_len = 0;
		let mut tcp_buffer = vec![0u8; TCP_SIZE];
		if let Some(payload) = payload {
			tcp_buffer.extend_from_slice(payload);
			payload_len = payload.len();
		};
		let mut tcp_packet = MutableTcpPacket::new(&mut tcp_buffer).unwrap();
		tcp_packet.set_source(self.src_port);
		tcp_packet.set_destination(self.dst_port);
		debug!("send una(packet seq):{}", self.send_param.una);
		tcp_packet.set_sequence(self.send_param.una); // TODO: reason
		tcp_packet.set_acknowledgement(self.recv_param.next);
		tcp_packet.set_data_offset(5);
		tcp_packet.set_flags(flag);
		tcp_packet.set_window(self.send_param.window);
		tcp_packet.set_checksum(tcp::ipv4_checksum(
			&tcp_packet.to_immutable(),
			&self.src_addr,
			&self.dst_addr,
		));
		ts.send_to(tcp_packet, IpAddr::V4(self.dst_addr))?;
		self.send_param.next = self.send_param.una + payload_len as u32;
		Ok(())
	}

	// MSSとウィンドウで分割
	pub fn send_data(&self) {
		unimplemented!()
	}
}
