use std::collections::{HashMap, VecDeque};
use std::thread;
use std::net::{Ipv4Addr, IpAddr};
use std::sync::{RwLock};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::transport::{self, TransportChannelType, TransportProtocol, TransportReceiver, TransportSender};
use pnet::packet::tcp::{self, MutableTcpPacket, TcpFlags};
use rand::prelude::*;

use super::tcp::TcpStatus;
const TCP_SIZE: usize = 20;


pub struct Socket {
	src_addr: Ipv4Addr,
	dst_addr: Ipv4Addr,
	pub src_port: u16,
	dst_port: u16,
	send_param: SendParam,
	recv_param: RecvParam,
	status: TcpStatus,
}

pub struct SendParam {
	una: u32, //未ACK送信
	next: u32, //次の送信
	window: u16,
	iss: u32, //初期送信シーケンス番号
}

pub struct RecvParam {
	next: u32,
	window: u16,
	irs: u32, //初期受信シーケンスno
}

impl Socket {
	pub fn read(&self, buffer: &mut [u8]) -> Result<(), failure::Error> {
		// 届いたデータはソケットバッファに貯めて、この関数が呼ばれた時に読み出して返す
		// イテレータ回して受信、自分のポート以外のものは捨てる
		unimplemented!()
	}

	pub fn write(&self) -> Result<(), failure::Error> {
		unimplemented!()
	}

	pub fn handshake(&mut self) {

	}

	pub fn send_tcp_packet(&self, flag: u16, payload: Option<Vec<u8>>) -> Result<(), failure::Error> {
		let (mut ts, _) = transport::transport_channel(1024, TransportChannelType::Layer4(TransportProtocol::Ipv4(IpNextHeaderProtocols::Tcp)))?;
		let mut tcp_buffer = vec![0u8; TCP_SIZE];
		if let Some(payload) = payload {
			tcp_buffer.extend_from_slice(&payload)
		};
		let mut tcp_packet = MutableTcpPacket::new(&mut tcp_buffer).unwrap();
		tcp_packet.set_source(self.src_port);
		tcp_packet.set_destination(self.dst_port);
		tcp_packet.set_sequence(self.send_param.una); // TODO: reason
		tcp_packet.set_acknowledgement(self.recv_param.next);
		tcp_packet.set_data_offset(5);
		tcp_packet.set_flags(flag);
		tcp_packet.set_window(self.send_param.window);
		tcp_packet.set_checksum(tcp::ipv4_checksum(&tcp_packet.to_immutable(), &self.src_addr, &self.dst_addr));
		ts.send_to(tcp_packet, IpAddr::V4(self.dst_addr))?;
		Ok(())
	}

	// MSSとウィンドウで分割
	pub fn send_data(&self) {

	}
}
