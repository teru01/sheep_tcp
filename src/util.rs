use super::socket::{Socket, TcpStatus};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::tcp::{self, TcpFlags, TcpPacket};
use pnet::transport::{
	self, TransportChannelType, TransportProtocol, TransportReceiver, TransportSender,
};
use std::collections::HashMap;
use std::fs;
use std::net::Ipv4Addr;

pub fn load_env() -> HashMap<String, String> {
	let contents = fs::read_to_string(".env").expect("Failed to read env file");
	let lines: Vec<_> = contents.split('\n').collect();
	let mut map = HashMap::new();
	for line in lines {
		let elm: Vec<_> = line.split('=').map(str::trim).collect();
		if elm.len() == 2 {
			map.insert(elm[0].to_string(), elm[1].to_string());
		}
	}
	map
}

pub fn create_tcp_channel() -> Result<(TransportSender, TransportReceiver), failure::Error> {
	Ok(transport::transport_channel(
		1024,
		TransportChannelType::Layer4(TransportProtocol::Ipv4(IpNextHeaderProtocols::Tcp)),
	)?)
}

pub fn is_correct_checksum(tcp_packet: &TcpPacket, src_addr: &Ipv4Addr, my_ip: &Ipv4Addr) -> bool {
	if !tcp::ipv4_checksum(tcp_packet, src_addr, my_ip) == tcp_packet.get_checksum() {
		warn!("checksum was not matched");
		false
	} else {
		true
	}
}

pub fn print_info(packet: &TcpPacket, src_addr: &Ipv4Addr, status: TcpStatus) {
	debug!("=================================");
	debug!("From Addr: {}", src_addr);
	debug!("From Port: {}", packet.get_source());
	debug!("To   Port: {}", packet.get_destination());
	debug!("Flag     : {}", flag_to_string(packet.get_flags()));
	debug!("status   : {:?}", status);
}

pub fn flag_to_string(flag: u16) -> String {
	let mut flag_str = "".to_string();
	if flag & TcpFlags::SYN > 0 {
		flag_str.push_str("SYN ");
	}
	if flag & TcpFlags::ACK > 0 {
		flag_str.push_str("ACK ");
	}
	if flag & TcpFlags::FIN > 0 {
		flag_str.push_str("FIN ");
	}
	if flag & TcpFlags::RST > 0 {
		flag_str.push_str("RST ");
	}
	if flag & TcpFlags::CWR > 0 {
		flag_str.push_str("CWR ");
	}
	if flag & TcpFlags::ECE > 0 {
		flag_str.push_str("ECE ");
	}
	if flag & TcpFlags::NS > 0 {
		flag_str.push_str("NS ");
	}
	if flag & TcpFlags::PSH > 0 {
		flag_str.push_str("PSH ");
	}
	if flag & TcpFlags::URG > 0 {
		flag_str.push_str("URG ");
	}
	flag_str
}

pub fn is_valid_seq_num(socket: &Socket, recv_packet: &TcpPacket) -> bool {
	return socket.recv_param.next != 0 && socket.recv_param.next != recv_packet.get_sequence();
}
