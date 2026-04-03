use std::io;

#[derive(Debug, thiserror::Error)]
pub enum GreeterError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("IPC error: {0}")]
    Ipc(String),
}

// Manual: greetd_ipc::codec::Error doesn't implement std::error::Error
impl From<greetd_ipc::codec::Error> for GreeterError {
    fn from(e: greetd_ipc::codec::Error) -> Self {
        GreeterError::Ipc(format!("{e:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_io_error() {
        let err = GreeterError::Io(io::Error::new(io::ErrorKind::NotFound, "gone"));
        assert_eq!(err.to_string(), "IO error: gone");
    }

    #[test]
    fn display_ipc_error() {
        let err = GreeterError::Ipc("socket closed".into());
        assert_eq!(err.to_string(), "IPC error: socket closed");
    }

    #[test]
    fn from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::BrokenPipe, "broken");
        let err: GreeterError = io_err.into();
        assert!(matches!(err, GreeterError::Io(_)));
    }

    #[test]
    fn from_codec_error() {
        let codec_err = greetd_ipc::codec::Error::Eof;
        let err: GreeterError = codec_err.into();
        assert!(matches!(err, GreeterError::Ipc(_)));
        assert!(err.to_string().contains("Eof"));
    }

    #[test]
    fn implements_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(GreeterError::Ipc("test".into()));
        assert_eq!(err.to_string(), "IPC error: test");
    }
}
