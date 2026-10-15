//! Wspólne typy Protobuf dla workera i połączenia sieciowego.
pub use prost::Message;
pub mod duel;
pub mod game {
    pub mod v1 {
        include!(concat!(env!("OUT_DIR"), "/game.v1.rs"));
    }
}
