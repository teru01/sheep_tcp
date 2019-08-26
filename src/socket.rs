use std::collections::{HashMap, VecDeque};
use std::thread;
use std::net::{Ipv4Addr, IpAddr};
use std::sync::{RwLock};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::transport::{self, TransportChannelType, TransportProtocol, TransportReceiver, TransportSender};
use pnet::packet::tcp::{self, MutableTcpPacket, TcpFlags};
use rand::prelude::*;

use super::tcp::TcpStatus;

pub struct Socket {
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
	window: u32,
	iss: u32, //初期送信シーケンス番号
}

pub struct RecvParam {
	next: u32,
	window: u32,
	irs: u32, //初期受信シーケンスno
}

impl Socket {
	pub fn read(&self, buffer: &mut [u8]) -> Result<(), failure::Error> {
		// 届いたデータはソケットバッファに貯めて、この関数が呼ばれた時に読み出して返す
		// イテレータ回して受信、自分のポート以外のものは捨てる
	}

	pub fn write(&self) -> Result<(), failure::Error> {

	}

	pub fn handshake(&mut self) {

	}

	pub fn send_tcp_packet(&self, dst_addr: Ipv4Addr, dst_port: u16, my_port: u16, flag: u16, payload: Option<Vec[u8]>) -> Result<(), failure::Error> {
		let (mut ts, _) = transport::transport_channel(1024, TransportChannelType::Layer4(TransportProtocol::Ipv4(IpNextHeaderProtocols::Tcp)))?;
		// let data = build_packet(dst_addr, my_addr, src_port, dst_port, flag, payload)?;
		let mut tcp_buffer = [0u8; TCP_SIZE];
		let mut tcp_header = MutableTcpPacket::new(&mut tcp_buffer[..]).unwrap();
		tcp_header.set_destination(dst_port);
		tcp_header.set_source(my_port);

		// オプションを含まないので、20オクテットまでがTCPヘッダ。4オクテット単位で指定する
		tcp_header.set_data_offset(5);
		tcp_header.set_flags(flag);
		let checksum = tcp::ipv4_checksum(
			&tcp_header.to_immutable(),
			my_ipaddr,
			&packet_info.target_ipaddr,
		);
		tcp_header.set_checksum(checksum);

		tcp_buffer
		ts.send_to(MutableTcpPacket::new(&mut data), IpAddr::V4(dst_addr))?;
	}

	// MSSとウィンドウで分割
	pub fn send_data(&self) {

	}
}
