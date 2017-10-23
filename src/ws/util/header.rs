//! Utility functions for reading and writing data frame headers.

use std::io::{Read, Write};
use result::{WebSocketResult, WebSocketError};
use byteorder::{BigEndian, ByteOrder, ReadBytesExt, WriteBytesExt};

#[allow(missing_docs)]
pub struct ReaderState {
	flags: Option<DataFrameFlags>,
	opcode: Option<u8>,
	has_mask: bool,
	mask: Vec<u8>,
	raw_len: Vec<u8>,
	len_byte: Option<u8>,
	len: Option<u64>,
}

impl ReaderState {
	pub(crate) fn new() -> ReaderState {
		ReaderState {
			flags: None,
			opcode: None,
			has_mask: false,
			mask: Vec::with_capacity(4),
			raw_len: Vec::with_capacity(8),
			len_byte: None,
			len: None,
		}
	}

	fn reset(&mut self) {
		self.flags = None;
		self.opcode = None;
		self.has_mask = false;
		self.mask = Vec::with_capacity(4);
		self.raw_len = Vec::with_capacity(8);
		self.len_byte = None;
		self.len = None;
	}
}

bitflags! {
	/// Flags relevant to a WebSocket data frame.
	pub struct DataFrameFlags: u8 {
		/// Marks this dataframe as the last dataframe
		const FIN = 0x80;
		/// First reserved bit
		const RSV1 = 0x40;
		/// Second reserved bit
		const RSV2 = 0x20;
		/// Third reserved bit
		const RSV3 = 0x10;
	}
}

/// Represents a data frame header.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataFrameHeader {
	/// The bit flags for the first byte of the header.
	pub flags: DataFrameFlags,
	/// The opcode of the header - must be <= 16.
	pub opcode: u8,
	/// The masking key, if any.
	pub mask: Option<[u8; 4]>,
	/// The length of the payload.
	pub len: u64,
}

/// Writes a data frame header.
pub fn write_header(writer: &mut Write, header: DataFrameHeader) -> WebSocketResult<()> {

	if header.opcode > 0xF {
		return Err(WebSocketError::DataFrameError("Invalid data frame opcode"));
	}
	if header.opcode >= 8 && header.len >= 126 {
		return Err(WebSocketError::DataFrameError("Control frame length too long"));
	}

	// Write 'FIN', 'RSV1', 'RSV2', 'RSV3' and 'opcode'
	writer.write_u8((header.flags.bits) | header.opcode)?;

	writer.write_u8(// Write the 'MASK'
	                if header.mask.is_some() { 0x80 } else { 0x00 } |
		// Write the 'Payload len'
		if header.len <= 125 { header.len as u8 }
		else if header.len <= 65535 { 126 }
		else { 127 })?;

	// Write 'Extended payload length'
	if header.len >= 126 && header.len <= 65535 {
		writer.write_u16::<BigEndian>(header.len as u16)?;
	} else if header.len > 65535 {
		writer.write_u64::<BigEndian>(header.len)?;
	}

	// Write 'Masking-key'
	if let Some(mask) = header.mask {
		writer.write_all(&mask)?
	}

	Ok(())
}

/// Reads a data frame header.
pub fn read_header<R>(
	reader: &mut R,
	dataframe: &mut ReaderState,
) -> WebSocketResult<DataFrameHeader> where R: Read {
	let ret = {
		//	If the flag entry is None, then read the first byte of the header
		if dataframe.flags.is_none() {
			reader.read_u8().and_then(|byte| {
				dataframe.flags = Some(DataFrameFlags::from_bits_truncate(byte));
				dataframe.opcode = Some(byte & 0x0F);
				Ok(())
			})?;
		}

		//	Save the length byte separate since it is needed if getting the length fails
		dataframe.len_byte = match reader.read_u8() {
			Ok(byte) => Some(byte),
			Err(why) => {
				debug!("Could not read length: {:?}", why);
				return Err(WebSocketError::IoError(why))
			}
		};

		let byte = dataframe.len_byte.unwrap();
		dataframe.has_mask = byte & 0x80 == 0x80;

		//	Using the length byte, determine the length of the payload
		if dataframe.len.is_none() {
			let len = match byte & 0x7F {
				0...125 => (byte & 0x7F) as u64,
				126 => {
					//	Make sure 2 bytes are available for read_u16
					while dataframe.raw_len.len() < 2 {
						let byte = match reader.read_u8() {
							Ok(byte) => byte,
							Err(why) => {
								debug!("Could not read u16 length byte: {:?}", why);
								return Err(WebSocketError::IoError(why));
							}
						};
						dataframe.raw_len.push(byte);
					}

					let len = BigEndian::read_u16(&mut dataframe.raw_len) as u64;
					if len <= 125 {
						dataframe.reset();
						return Err(WebSocketError::DataFrameError("Invalid data frame length"));
					}
					len
				}
				127 => {
					//	Make sure 8 bytes are available for read_u64
					while dataframe.raw_len.len() < 8 {
						let byte = match reader.read_u8() {
							Ok(byte) => byte,
							Err(why) => {
								debug!("Could not read u64 length byte: {:?}", why);
								return Err(WebSocketError::IoError(why));
							}
						};
						dataframe.raw_len.push(byte);
					}

					let len = BigEndian::read_u64(&mut dataframe.raw_len);
					if len <= 65535 {
						dataframe.reset();
						return Err(WebSocketError::DataFrameError("Invalid data frame length"));
					}
					len
				}
				_ => unreachable!(),
			};

			dataframe.len = Some(len);
		}

		//	Check for invalid state
		if dataframe.opcode.unwrap() >= 8 {
			if dataframe.len.unwrap() >= 126 {
				dataframe.reset();
				return Err(WebSocketError::DataFrameError("Control frame length too long"));
			}
			if !dataframe.flags.unwrap().contains(FIN) {
				dataframe.reset();
				return Err(WebSocketError::ProtocolError("Illegal fragmented control frame"));
			}
		}

		//	Get the mask if one exists, making sure to have exactly 4 bytes
		if dataframe.has_mask {
			while dataframe.mask.len() < 4 {
				let byte = match reader.read_u8() {
					Ok(byte) => byte,
					Err(why) => {
						debug!("Could not read mask byte: {:?}", why);
						return Err(WebSocketError::IoError(why));
					}
				};
				dataframe.mask.push(byte);
			}
		}

		//	Return the final state
		Ok(DataFrameHeader {
			flags: dataframe.flags.unwrap(),
			opcode: dataframe.opcode.unwrap(),
			mask: if dataframe.has_mask {
				let mut mask = [0; 4];
				mask.clone_from_slice(&dataframe.mask[0..4]);
				Some(mask)
			} else {
				None
				},
			len: dataframe.len.unwrap(),
		})
	};

	dataframe.reset();

	ret
}

#[cfg(all(feature = "nightly", test))]
mod tests {
	use super::*;
	use test;
	#[test]
	fn test_read_header_simple() {
		let header = [0x81, 0x2B];
		let obtained = read_header(&mut &header[..]).unwrap();
		let expected = DataFrameHeader {
			flags: FIN,
			opcode: 1,
			mask: None,
			len: 43,
		};
		assert_eq!(obtained, expected);
	}
	#[test]
	fn test_write_header_simple() {
		let header = DataFrameHeader {
			flags: FIN,
			opcode: 1,
			mask: None,
			len: 43,
		};
		let expected = [0x81, 0x2B];
		let mut obtained = Vec::with_capacity(2);
		write_header(&mut obtained, header).unwrap();

		assert_eq!(&obtained[..], &expected[..]);
	}
	#[test]
	fn test_read_header_complex() {
		let header = [0x42, 0xFE, 0x02, 0x00, 0x02, 0x04, 0x08, 0x10];
		let obtained = read_header(&mut &header[..]).unwrap();
		let expected = DataFrameHeader {
			flags: RSV1,
			opcode: 2,
			mask: Some([2, 4, 8, 16]),
			len: 512,
		};
		assert_eq!(obtained, expected);
	}
	#[test]
	fn test_write_header_complex() {
		let header = DataFrameHeader {
			flags: RSV1,
			opcode: 2,
			mask: Some([2, 4, 8, 16]),
			len: 512,
		};
		let expected = [0x42, 0xFE, 0x02, 0x00, 0x02, 0x04, 0x08, 0x10];
		let mut obtained = Vec::with_capacity(8);
		write_header(&mut obtained, header).unwrap();

		assert_eq!(&obtained[..], &expected[..]);
	}
	#[bench]
	fn bench_read_header(b: &mut test::Bencher) {
		let header = vec![0x42u8, 0xFE, 0x02, 0x00, 0x02, 0x04, 0x08, 0x10];
		b.iter(|| { read_header(&mut &header[..]).unwrap(); });
	}
	#[bench]
	fn bench_write_header(b: &mut test::Bencher) {
		let header = DataFrameHeader {
			flags: RSV1,
			opcode: 2,
			mask: Some([2, 4, 8, 16]),
			len: 512,
		};
		let mut writer = Vec::with_capacity(8);
		b.iter(|| { write_header(&mut writer, header).unwrap(); });
	}
}
