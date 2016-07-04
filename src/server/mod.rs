//! Provides an implementation of a WebSocket server
use std::net::{
	SocketAddr,
	ToSocketAddrs,
	TcpListener,
	TcpStream,
	Shutdown,
};
use std::io::{
	self,
	Read,
	Write,
};
use std::borrow::Cow;
use std::ops::Deref;
use std::convert::Into;
use openssl::ssl::{
	SslContext,
	SslMethod,
	SslStream,
};
use stream::{
	Stream,
	MaybeSslContext,
	NoSslContext,
};
use self::upgrade::{
	WsUpgrade,
	IntoWs,
};
pub use self::upgrade::hyper::{
	Request,
	HyperIntoWsError,
};

pub mod request;
pub mod response;
pub mod upgrade;

pub struct InvalidConnection<S>
where S: Stream,
{
	pub stream: Option<S>,
	pub parsed: Option<Request>,
	pub error: HyperIntoWsError,
}

pub type AcceptResult<S> = Result<WsUpgrade<S>, InvalidConnection<S>>;

/// Represents a WebSocket server which can work with either normal (non-secure) connections, or secure WebSocket connections.
///
/// This is a convenient way to implement WebSocket servers, however it is possible to use any sendable Reader and Writer to obtain
/// a WebSocketClient, so if needed, an alternative server implementation can be used.
///#Non-secure Servers
///
/// ```no_run
///extern crate websocket;
///# fn main() {
///use std::thread;
///use websocket::{Server, Message};
///
///let server = Server::bind("127.0.0.1:1234").unwrap();
///
///for connection in server {
///    // Spawn a new thread for each connection.
///    thread::spawn(move || {
///		   let request = connection.unwrap().read_request().unwrap(); // Get the request
///		   let response = request.accept(); // Form a response
///		   let mut client = response.send().unwrap(); // Send the response
///
///		   let message = Message::text("Hello, client!");
///		   let _ = client.send_message(&message);
///
///		   // ...
///    });
///}
/// # }
/// ```
///
///#Secure Servers
/// ```no_run
///extern crate websocket;
///extern crate openssl;
///# fn main() {
///use std::thread;
///use std::path::Path;
///use websocket::{Server, Message};
///use openssl::ssl::{SslContext, SslMethod};
///use openssl::x509::X509FileType;
///
///let mut context = SslContext::new(SslMethod::Tlsv1).unwrap();
///let _ = context.set_certificate_file(&(Path::new("cert.pem")), X509FileType::PEM);
///let _ = context.set_private_key_file(&(Path::new("key.pem")), X509FileType::PEM);
///let server = Server::bind_secure("127.0.0.1:1234", &context).unwrap();
///
///for connection in server {
///    // Spawn a new thread for each connection.
///    thread::spawn(move || {
///		   let request = connection.unwrap().read_request().unwrap(); // Get the request
///		   let response = request.accept(); // Form a response
///		   let mut client = response.send().unwrap(); // Send the response
///
///		   let message = Message::text("Hello, client!");
///		   let _ = client.send_message(&message);
///
///		   // ...
///    });
///}
/// # }
/// ```
pub struct Server<'s, S>
where S: MaybeSslContext + 's,
{
	inner: TcpListener,
	ssl_context: Cow<'s, S>,
}

impl<'s, S> Server<'s, S>
where S: MaybeSslContext + 's,
{
	/// Get the socket address of this server
	pub fn local_addr(&self) -> io::Result<SocketAddr> {
		self.inner.local_addr()
	}

	/// Create a new independently owned handle to the underlying socket.
	pub fn try_clone(&'s self) -> io::Result<Server<'s, S>> {
		let inner = try!(self.inner.try_clone());
		Ok(Server {
			inner: inner,
			ssl_context: Cow::Borrowed(&*self.ssl_context),
		})
	}

	pub fn into_owned<'o>(self) -> io::Result<Server<'o, S>> {
		Ok(Server {
			inner: self.inner,
			ssl_context: Cow::Owned(self.ssl_context.into_owned()),
		})
	}
}

impl<'s> Server<'s, SslContext> {
	/// Bind this Server to this socket, utilising the given SslContext
	pub fn bind_secure<A>(addr: A, context: &'s SslContext) -> io::Result<Self>
	where A: ToSocketAddrs,
	{
		Ok(Server {
			inner: try!(TcpListener::bind(&addr)),
			ssl_context: Cow::Borrowed(context),
		})
	}

	/// Wait for and accept an incoming WebSocket connection, returning a WebSocketRequest
	pub fn accept(&mut self) -> AcceptResult<SslStream<TcpStream>> {
		let stream = match self.inner.accept() {
			Ok(s) => s.0,
			Err(e) => return Err(InvalidConnection {
				stream: None,
				parsed: None,
				error: e.into(),
			}),
		};

		let stream = match SslStream::accept(&*self.ssl_context, stream) {
			Ok(s) => s,
			Err(err) => return Err(InvalidConnection {
				stream: None,
				parsed: None,
				error: io::Error::new(io::ErrorKind::Other, err).into(),
			}),
		};

		match stream.into_ws() {
			Ok(u) => Ok(u),
			Err((s, r, e)) => Err(InvalidConnection {
				stream: Some(s),
				parsed: r,
				error: e.into(),
			}),
		}
	}

    /// Changes whether the Server is in nonblocking mode.
    ///
    /// If it is in nonblocking mode, accept() will return an error instead of blocking when there
    /// are no incoming connections.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }
}

impl<'s> Iterator for Server<'s, SslContext> {
	type Item = WsUpgrade<SslStream<TcpStream>>;

	fn next(&mut self) -> Option<<Self as Iterator>::Item> {
		self.accept().ok()
	}
}

impl<'s> Server<'s, NoSslContext> {
	/// Bind this Server to this socket
	pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
		Ok(Server {
			inner: try!(TcpListener::bind(&addr)),
			ssl_context: Cow::Owned(NoSslContext),
		})
	}

	/// Wait for and accept an incoming WebSocket connection, returning a WebSocketRequest
	pub fn accept(&mut self) -> AcceptResult<TcpStream> {
		let stream = match self.inner.accept() {
			Ok(s) => s.0,
			Err(e) => return Err(InvalidConnection {
				stream: None,
				parsed: None,
				error: e.into(),
			}),
		};

		match stream.into_ws() {
			Ok(u) => Ok(u),
			Err((s, r, e)) => Err(InvalidConnection {
				stream: Some(s),
				parsed: r,
				error: e.into(),
			}),
		}
	}
}

impl<'s> Iterator for Server<'s, NoSslContext> {
	type Item = WsUpgrade<TcpStream>;

	fn next(&mut self) -> Option<<Self as Iterator>::Item> {
		self.accept().ok()
	}
}

