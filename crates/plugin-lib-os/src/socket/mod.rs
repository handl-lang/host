mod async_socket;
mod socket;

pub use async_socket::{AsyncSocket, AsyncSocketModule};
pub use socket::{register_socket_ops, Socket, SocketModule, SOCKET_OPS};

pub mod socket_stub;

#[cfg(test)]
mod socket_tests;
