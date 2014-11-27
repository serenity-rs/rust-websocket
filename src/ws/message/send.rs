use super::dataframe::{WebSocketOpcode, WebSocketDataFrame, WebSocketDataFrameLength, WriteWebSocketDataFrame};
use super::mask::mask_data;
use super::WebSocketMessage;
use std::io::net::tcp::TcpStream;
use std::io::IoResult;
use std::rand;

pub struct WebSocketSender {
	stream: TcpStream,
	mask: bool,
}

fn message_to_dataframe(message: &WebSocketMessage, mask: bool, finished: bool) -> WebSocketDataFrame {
	let mut opcode: WebSocketOpcode;
	let masking_key = if mask { Some(gen_key()) } else { None };
	let mut data: Vec<u8> = Vec::new();
	match *message {
		WebSocketMessage::Text(ref payload) => {				
			opcode = WebSocketOpcode::Text;
			data = payload.clone().into_bytes();
		}
		WebSocketMessage::Binary(ref payload) => {
			opcode = WebSocketOpcode::Binary;
			data = payload.clone();
		}
		WebSocketMessage::Close => { opcode = WebSocketOpcode::Close; }
		WebSocketMessage::Ping => { opcode = WebSocketOpcode::Ping; }
		WebSocketMessage::Pong => { opcode = WebSocketOpcode::Pong; }
	}
	
	if mask {
		data = mask_data(masking_key.unwrap(), data.as_slice());
	}
	
	let length = WebSocketDataFrameLength::new(data.len());
	
	WebSocketDataFrame {
		finished: finished,
		reserved: [false, ..3],
		opcode: opcode,
		mask: masking_key,
		length: length,
		data: data,
	}
}

impl WebSocketSender {
	pub fn send_message(&mut self, message: &WebSocketMessage) -> IoResult<()> {
		let dataframe = message_to_dataframe(message, self.mask, true);
		try!(self.stream.write_websocket_dataframe(dataframe));
		Ok(())
	}
	
	pub fn fragment(&mut self) -> WebSocketFragmentSerializer {
		WebSocketFragmentSerializer {
			inc: self,
			started: false,
		}
	}
}

pub struct WebSocketFragmentSerializer<'a> {
	inc: &'a mut WebSocketSender,
	started: bool,
}

impl<'a> WebSocketFragmentSerializer<'a> {
	pub fn send_fragment(&mut self, message: &WebSocketMessage) -> IoResult<()> {
		let mut dataframe = message_to_dataframe(message, self.inc.mask, false);
		
		if self.started {
			dataframe.opcode = WebSocketOpcode::Continuation;
		}
		else {
			self.started = true;
		}
		
		try!(self.inc.stream.write_websocket_dataframe(dataframe));
		Ok(())
	}
	
	pub fn finish(&mut self, message: &WebSocketMessage) -> IoResult<()> {
		let mut dataframe = message_to_dataframe(message, self.inc.mask, true);
		
		if self.started {
			dataframe.opcode = WebSocketOpcode::Continuation;
			self.started = false;
		}
		
		try!(self.inc.stream.write_websocket_dataframe(dataframe));
		Ok(())
	}
}

pub fn new_sender(stream: TcpStream, mask: bool) -> WebSocketSender {
	WebSocketSender {
		stream: stream,
		mask: mask,
	}
}

fn gen_key() -> [u8, ..4] {
	[rand::random::<u8>(), rand::random::<u8>(), rand::random::<u8>(), rand::random::<u8>()]
}