use pnet::packet::tcp::{self, MutableTcpPacket};
use pnet::transport::TransportSender;
use std::fmt::{self, Debug};
use std::net::{IpAddr, Ipv4Addr};

const TCP_SIZE: usize = 20;

pub struct Socket {
	pub src_addr: Ipv4Addr,
	pub dst_addr: Option<Ipv4Addr>,
	pub src_port: u16,
	pub dst_port: Option<u16>,
	pub send_param: SendParam,
	pub recv_param: RecvParam,
	pub status: TcpStatus,
	pub buffer: Vec<u8>,
}

pub struct SendParam {
	pub una: u32,  //未ACK送信
	pub next: u32, //次の送信
	pub window: u16,
	pub iss: u32, //初期送信シーケンス番号
}

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
			TcpStatus::Listen => write!(f, "LISTEN"),
			TcpStatus::SynSent => write!(f, "SYNSENT"),
			TcpStatus::SynRecv => write!(f, "SYNRECV"),
			TcpStatus::Established => write!(f, "ESTABLISHED"),
			TcpStatus::FinWait1 => write!(f, "FINWAIT1"),
			TcpStatus::FinWait2 => write!(f, "FINWAIT2"),
			TcpStatus::Closing => write!(f, "CLOSING"),
			TcpStatus::LastAck => write!(f, "LASTACK"),
			TcpStatus::TimeWait => write!(f, "TIMEWAIT"),
			TcpStatus::Closed => write!(f, "CLOSED"),
		}
	}
}

impl Socket {
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
		if let Some(port) = self.dst_port {
			tcp_packet.set_destination(port);
		} else {
			return Err(failure::err_msg("missing dest port"));
		}
		tcp_packet.set_sequence(self.send_param.una); // TODO: reason
		tcp_packet.set_acknowledgement(self.recv_param.next);
		tcp_packet.set_data_offset(5);
		tcp_packet.set_flags(flag);
		tcp_packet.set_window(self.send_param.window);

		if let Some(dst_addr) = self.dst_addr {
			tcp_packet.set_checksum(tcp::ipv4_checksum(
				&tcp_packet.to_immutable(),
				&self.src_addr,
				&dst_addr,
			));
			ts.send_to(tcp_packet, IpAddr::V4(dst_addr))?;
		}
		self.send_param.next = self.send_param.una + payload_len as u32;
		Ok(())
	}
}
