use pnet::packet::tcp::{self, MutableTcpPacket};
use pnet::transport::TransportSender;
use std::fmt::{self, Debug};
use std::net::{IpAddr, Ipv4Addr};

const TCP_SIZE: usize = 20;
const TCP_INIT_WINDOW: usize = 1460;

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

#[derive(Clone)]
pub struct SendParam {
	pub una: u32,  //未ACK送信
	pub next: u32, //次の送信
	pub window: u16,
	pub iss: u32, //初期送信シーケンス番号
}

#[derive(Clone)]
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

	pub fn initialize(my_ip: Ipv4Addr, dst_addr: Option<Ipv4Addr>, my_port: u16, dst_port: Option<u16>, status: TcpStatus) -> Self {
		let initial_seq = rand::random::<u32>();
		Socket {
			src_addr: my_ip,
			dst_addr,
			src_port: my_port,
			dst_port,
			send_param: SendParam {
				una: initial_seq,
				next: initial_seq,
				window: TCP_INIT_WINDOW as u16,
				iss: initial_seq,
			},
			recv_param: RecvParam {
				next: 0,
				window: 0,
				irs: 0,
			},
			status,
			buffer: Vec::new(),
		}
	}

	pub fn create_established(my_ip: Ipv4Addr, dst_ip: Ipv4Addr, my_port: u16, dst_port: u16, send_param: &SendParam, recv_param: &RecvParam) -> Self {
		Socket {
			src_addr: my_ip,
			dst_addr: Some(dst_ip),
			src_port: my_port,
			dst_port: Some(dst_port),
			send_param: send_param.clone(),
			recv_param: recv_param.clone(),
			status: TcpStatus::Established,
			buffer: Vec::new(),
		}
	}
}
