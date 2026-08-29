//! HBOP Protobuf 协议定义与生成代码。
//!
//! 由 `build.rs` 从 `proto/badou.proto` 自动生成。
//! 包含 `BaDouStorage` gRPC service trait（server）与 `BaDouStorageClient`（client）。

pub mod badou {
    pub mod v1 {
        #![allow(clippy::result_large_err)]
        tonic::include_proto!("badou.v1");
    }
}

pub use badou::v1::*;
