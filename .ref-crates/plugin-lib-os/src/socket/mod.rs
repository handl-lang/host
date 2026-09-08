mod async_socket;
mod socket;

pub use async_socket::{AsyncSocket, AsyncSocketModule};
pub use socket::{SOCKET_OPS, Socket, SocketModule, register_socket_ops};

pub mod socket_stub;

#[cfg(test)]
mod socket_tests;
